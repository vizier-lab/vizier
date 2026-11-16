//! The PostgreSQL backend (spec 045, research R2/R3): a pooled synchronous `postgres::Client`
//! behind the same [`RawConn`] seam as SQLite. Calls made from a multi-thread tokio worker run
//! inside `block_in_place`, so the client's own runtime can block without starving the server.
//!
//! Statements are written for SQLite's `?N`/`?` placeholders and rewritten to `$N` here. Each
//! parameter is converted to the Rust type PostgreSQL inferred for it, so the shared code can
//! keep binding plain integers, strings and NULLs.

use std::collections::HashMap;
use std::mem::ManuallyDrop;
use std::sync::Arc;
use std::time::Duration;

use parking_lot::Mutex;
use postgres::types::{
    FromSql as PgFromSql, IsNull, Kind, ToSql as PgToSql, Type as PgType, to_sql_checked,
};
use postgres::{Client, Statement};
use r2d2::PooledConnection;
use r2d2_postgres::PostgresConnectionManager;
use tokio_postgres_rustls::MakeRustlsConnect;

use super::{Backend, Dialect, Error, Pool, RawConn, Result, Row, SqlValue};

mod migrate;
pub use migrate::migrate;

type Manager = PostgresConnectionManager<MakeRustlsConnect>;

/// Pool settings resolved from configuration.
#[derive(Clone, Debug)]
pub struct PgOptions {
    pub max_size: u32,
    pub connect_timeout: Duration,
    /// Extra PEM roots for the server certificate (`ca_file`); platform roots otherwise.
    pub ca_pem: Option<Vec<u8>>,
}

impl Default for PgOptions {
    fn default() -> Self {
        Self {
            max_size: 8,
            connect_timeout: Duration::from_secs(10),
            ca_pem: None,
        }
    }
}

pub struct PgPool {
    /// Dropped in [`Drop`] through [`blocking`]: closing a client blocks on its own runtime.
    pool: ManuallyDrop<r2d2::Pool<Manager>>,
    /// `?N` → `$N` rewrites, keyed by the original statement text.
    rewrites: Mutex<HashMap<String, Arc<str>>>,
}

impl Drop for PgPool {
    fn drop(&mut self) {
        // SAFETY: `pool` is taken exactly once, here, and never touched again.
        let pool = unsafe { ManuallyDrop::take(&mut self.pool) };
        blocking(move || drop(pool));
    }
}

impl PgPool {
    /// Connect to `url` (`postgres://…`). The error never contains the URL or its password.
    pub fn connect(url: &str, options: &PgOptions) -> anyhow::Result<Self> {
        let config: postgres::Config = url.parse().map_err(|_| {
            anyhow::anyhow!("storage: VIZIER_DATABASE_URL is not a valid PostgreSQL URL")
        })?;
        // r2d2 reports every failed initial connection as a timeout, discarding PostgreSQL's
        // SQLSTATE. Probe once directly so startup can name an authentication or database error
        // without exposing the URL, username or password.
        let probe_config = config.clone();
        let probe_tls = MakeRustlsConnect::new(tls_config(options.ca_pem.as_deref())?);
        let probe = blocking(move || probe_config.connect(probe_tls)).map_err(|e| {
            anyhow::anyhow!(
                "storage: cannot connect to PostgreSQL ({})",
                describe_postgres_error(&e)
            )
        })?;
        blocking(move || drop(probe));
        let tls = MakeRustlsConnect::new(tls_config(options.ca_pem.as_deref())?);
        let manager = PostgresConnectionManager::new(config, tls);
        let pool = blocking(|| {
            r2d2::Pool::builder()
                .max_size(options.max_size)
                .connection_timeout(options.connect_timeout)
                .build(manager)
        })
        .map_err(|e| {
            anyhow::anyhow!(
                "storage: cannot connect to PostgreSQL ({})",
                describe_pool_error(&e)
            )
        })?;
        Ok(Self {
            pool: ManuallyDrop::new(pool),
            rewrites: Mutex::new(HashMap::new()),
        })
    }

    fn rewrite(&self, sql: &str) -> Arc<str> {
        if let Some(hit) = self.rewrites.lock().get(sql) {
            return hit.clone();
        }
        let out: Arc<str> = rewrite_placeholders(sql).into();
        let mut cache = self.rewrites.lock();
        if cache.len() > 4096 {
            cache.clear();
        }
        cache.insert(sql.to_owned(), out.clone());
        out
    }
}

/// The TLS configuration: platform roots, plus `ca_pem` when given.
fn tls_config(ca_pem: Option<&[u8]>) -> anyhow::Result<rustls::ClientConfig> {
    let provider = Arc::new(rustls::crypto::aws_lc_rs::default_provider());
    let builder = rustls::ClientConfig::builder_with_provider(provider.clone())
        .with_safe_default_protocol_versions()
        .map_err(|e| anyhow::anyhow!("storage: TLS setup failed: {e}"))?;
    let config = match ca_pem {
        None => {
            use rustls_platform_verifier::BuilderVerifierExt;
            builder
                .with_platform_verifier()
                .map_err(|e| anyhow::anyhow!("storage: TLS setup failed: {e}"))?
                .with_no_client_auth()
        }
        Some(pem) => {
            let mut roots = rustls::RootCertStore::empty();
            let mut reader = std::io::BufReader::new(pem);
            for cert in rustls_pemfile_certs(&mut reader)? {
                roots
                    .add(cert)
                    .map_err(|e| anyhow::anyhow!("storage: invalid certificate in ca_file: {e}"))?;
            }
            builder.with_root_certificates(roots).with_no_client_auth()
        }
    };
    Ok(config)
}

/// Parse every `CERTIFICATE` block of a PEM file.
fn rustls_pemfile_certs(
    reader: &mut dyn std::io::BufRead,
) -> anyhow::Result<Vec<rustls::pki_types::CertificateDer<'static>>> {
    use rustls::pki_types::pem::PemObject;
    let mut text = Vec::new();
    reader.read_to_end(&mut text)?;
    rustls::pki_types::CertificateDer::pem_slice_iter(&text)
        .collect::<std::result::Result<Vec<_>, _>>()
        .map_err(|e| anyhow::anyhow!("storage: ca_file is not a PEM certificate bundle: {e}"))
}

/// The SQLSTATE or I/O kind of a direct connection failure, never its message.
fn describe_postgres_error(error: &postgres::Error) -> String {
    if let Some(db) = error.as_db_error() {
        return db.code().code().to_string();
    }
    let mut source = std::error::Error::source(error);
    while let Some(error) = source {
        if let Some(io) = error.downcast_ref::<std::io::Error>() {
            return match io.kind() {
                std::io::ErrorKind::ConnectionRefused => "connection refused".into(),
                std::io::ErrorKind::TimedOut => "timed out".into(),
                kind => format!("{kind:?}").to_ascii_lowercase(),
            };
        }
        source = error.source();
    }
    "connection failed".into()
}

/// The SQLSTATE or I/O kind of a connection failure, never the message (which can echo the
/// host and user).
fn describe_pool_error(e: &r2d2::Error) -> String {
    let text = e.to_string();
    for code in ["28P01", "28000", "3D000", "57P03", "08001", "08006"] {
        if text.contains(code) {
            return code.to_string();
        }
    }
    if text.contains("timed out") {
        "timed out".into()
    } else if text.contains("refused") {
        "connection refused".into()
    } else if text.contains("password authentication failed") {
        "28P01".into()
    } else {
        "connection failed".into()
    }
}

/// Run `f` where blocking is allowed: inside `block_in_place` on a multi-thread runtime
/// worker, on a scoped thread under a current-thread runtime, directly elsewhere.
pub(crate) fn blocking<T: Send>(f: impl FnOnce() -> T + Send) -> T {
    match tokio::runtime::Handle::try_current() {
        Ok(h) if h.runtime_flavor() == tokio::runtime::RuntimeFlavor::MultiThread => {
            tokio::task::block_in_place(f)
        }
        // The client blocks on its own runtime, which tokio refuses to start on a thread that is
        // already driving one (a current-thread runtime cannot `block_in_place`), so the call
        // runs on a short-lived thread with no runtime context.
        Ok(_) => std::thread::scope(|s| match s.spawn(f).join() {
            Ok(out) => out,
            Err(panic) => std::panic::resume_unwind(panic),
        }),
        Err(_) => f(),
    }
}

impl Pool for PgPool {
    fn backend(&self) -> Backend {
        Backend::Postgres
    }

    fn connect(&self) -> anyhow::Result<Box<dyn RawConn + '_>> {
        let client = blocking(|| self.pool.get()).map_err(|e| {
            anyhow::anyhow!(
                "storage: no PostgreSQL connection available ({})",
                describe_pool_error(&e)
            )
        })?;
        Ok(Box::new(PgConn {
            pool: self,
            client: ManuallyDrop::new(client),
            in_tx: false,
        }))
    }
}

struct PgConn<'p> {
    pool: &'p PgPool,
    /// Returned to the pool in [`Drop`] through [`blocking`]: a broken client is closed there.
    client: ManuallyDrop<PooledConnection<Manager>>,
    in_tx: bool,
}

impl PgConn<'_> {
    fn prepare(&mut self, sql: &str) -> Result<Statement> {
        let sql = self.pool.rewrite(sql);
        let client: &mut Client = &mut self.client;
        blocking(|| client.prepare(&sql)).map_err(map_err)
    }
}

impl Drop for PgConn<'_> {
    fn drop(&mut self) {
        // SAFETY: `client` is taken exactly once, here, and never touched again.
        let mut client = unsafe { ManuallyDrop::take(&mut self.client) };
        let in_tx = self.in_tx;
        blocking(move || {
            // Never hand a connection with an open transaction back to the pool.
            if in_tx {
                let _ = client.batch_execute("ROLLBACK");
            }
            drop(client);
        });
    }
}

impl RawConn for PgConn<'_> {
    fn backend(&self) -> Backend {
        Backend::Postgres
    }

    fn dialect(&self) -> &'static dyn Dialect {
        &PgDialect
    }

    fn execute(&mut self, sql: &str, params: Vec<SqlValue>) -> Result<usize> {
        let stmt = self.prepare(sql)?;
        let bound = bind(&stmt, params)?;
        let refs: Vec<&(dyn PgToSql + Sync)> =
            bound.iter().map(|p| p as &(dyn PgToSql + Sync)).collect();
        let client: &mut Client = &mut self.client;
        let n = blocking(|| client.execute(&stmt, &refs)).map_err(map_err)?;
        Ok(n as usize)
    }

    fn query(&mut self, sql: &str, params: Vec<SqlValue>) -> Result<Vec<Row>> {
        let stmt = self.prepare(sql)?;
        let bound = bind(&stmt, params)?;
        let refs: Vec<&(dyn PgToSql + Sync)> =
            bound.iter().map(|p| p as &(dyn PgToSql + Sync)).collect();
        let client: &mut Client = &mut self.client;
        let rows = blocking(|| client.query(&stmt, &refs)).map_err(map_err)?;
        rows.iter().map(read_row).collect()
    }

    fn execute_batch(&mut self, sql: &str) -> Result<()> {
        let trimmed = sql.trim().to_ascii_uppercase();
        if trimmed.starts_with("BEGIN") {
            self.in_tx = true;
        } else if trimmed == "COMMIT" || trimmed == "ROLLBACK" {
            self.in_tx = false;
        }
        let client: &mut Client = &mut self.client;
        blocking(|| client.batch_execute(sql)).map_err(map_err)
    }
}

// ---- placeholders ---------------------------------------------------------------------------

/// Rewrite SQLite placeholders to PostgreSQL's: `?N` → `$N`, and a bare `?` → `$k` where `k` is
/// one more than the largest index used so far (SQLite's rule). Quoted literals, quoted
/// identifiers and comments are left alone.
pub(crate) fn rewrite_placeholders(sql: &str) -> String {
    let bytes = sql.as_bytes();
    let mut out = String::with_capacity(sql.len() + 8);
    let mut max = 0usize;
    let mut i = 0;
    while i < bytes.len() {
        let c = bytes[i];
        match c {
            b'\'' | b'"' => {
                let end = find_quote_end(bytes, i);
                out.push_str(&sql[i..end]);
                i = end;
            }
            b'-' if bytes.get(i + 1) == Some(&b'-') => {
                let end = sql[i..].find('\n').map_or(bytes.len(), |n| i + n);
                out.push_str(&sql[i..end]);
                i = end;
            }
            b'?' => {
                let start = i + 1;
                let mut j = start;
                while j < bytes.len() && bytes[j].is_ascii_digit() {
                    j += 1;
                }
                let n = if j > start {
                    sql[start..j].parse::<usize>().unwrap_or(0)
                } else {
                    max + 1
                };
                max = max.max(n);
                out.push('$');
                out.push_str(&n.to_string());
                i = j;
            }
            _ => {
                let ch = sql[i..].chars().next().unwrap_or(' ');
                out.push(ch);
                i += ch.len_utf8();
            }
        }
    }
    out
}

fn find_quote_end(bytes: &[u8], start: usize) -> usize {
    let quote = bytes[start];
    let mut i = start + 1;
    while i < bytes.len() {
        if bytes[i] == quote {
            if bytes.get(i + 1) == Some(&quote) {
                i += 2;
                continue;
            }
            return i + 1;
        }
        i += 1;
    }
    bytes.len()
}

// ---- values ---------------------------------------------------------------------------------

/// A parameter converted to the type PostgreSQL inferred for its placeholder.
#[derive(Debug)]
enum Bound {
    Null(PgType),
    Bool(bool),
    I16(i16),
    I32(i32),
    I64(i64),
    F32(f32),
    F64(f64),
    Text(String),
    Bytes(Vec<u8>),
}

impl PgToSql for Bound {
    fn to_sql(
        &self,
        ty: &PgType,
        out: &mut bytes::BytesMut,
    ) -> std::result::Result<IsNull, Box<dyn std::error::Error + Sync + Send>> {
        match self {
            Bound::Null(_) => Ok(IsNull::Yes),
            Bound::Bool(v) => v.to_sql(ty, out),
            Bound::I16(v) => v.to_sql(ty, out),
            Bound::I32(v) => v.to_sql(ty, out),
            Bound::I64(v) => v.to_sql(ty, out),
            Bound::F32(v) => v.to_sql(ty, out),
            Bound::F64(v) => v.to_sql(ty, out),
            Bound::Text(v) => {
                // TEXT-like and unknown/custom types (e.g. an enum) take the UTF-8 bytes.
                out.extend_from_slice(v.as_bytes());
                Ok(IsNull::No)
            }
            Bound::Bytes(v) => v.to_sql(ty, out),
        }
    }

    fn accepts(_: &PgType) -> bool {
        true
    }

    to_sql_checked!();
}

fn bind(stmt: &Statement, params: Vec<SqlValue>) -> Result<Vec<Bound>> {
    let types = stmt.params();
    if types.len() != params.len() {
        return Err(Error::backend(anyhow::anyhow!(
            "statement expects {} parameters, got {}",
            types.len(),
            params.len()
        )));
    }
    types
        .iter()
        .zip(params)
        .enumerate()
        .map(|(i, (ty, v))| convert(i, ty, v))
        .collect()
}

fn convert(idx: usize, ty: &PgType, value: SqlValue) -> Result<Bound> {
    let mismatch = |v: &SqlValue| {
        Error::backend(anyhow::anyhow!(
            "parameter {} ({:?}) cannot be bound as {}",
            idx + 1,
            v.data_type(),
            ty.name()
        ))
    };
    Ok(match (value, ty) {
        (SqlValue::Null, ty) => Bound::Null(ty.clone()),
        (SqlValue::Integer(i), &PgType::BOOL) => Bound::Bool(i != 0),
        (SqlValue::Integer(i), &PgType::INT2) => {
            Bound::I16(i16::try_from(i).map_err(|_| mismatch(&SqlValue::Integer(i)))?)
        }
        (SqlValue::Integer(i), &PgType::INT4) => {
            Bound::I32(i32::try_from(i).map_err(|_| mismatch(&SqlValue::Integer(i)))?)
        }
        (SqlValue::Integer(i), &PgType::INT8) => Bound::I64(i),
        (SqlValue::Integer(i), &PgType::FLOAT4) => Bound::F32(i as f32),
        (SqlValue::Integer(i), &PgType::FLOAT8) => Bound::F64(i as f64),
        (SqlValue::Integer(i), ty) if is_textual(ty) => Bound::Text(i.to_string()),
        (SqlValue::Real(r), &PgType::FLOAT4) => Bound::F32(r as f32),
        (SqlValue::Real(r), &PgType::FLOAT8) => Bound::F64(r),
        (SqlValue::Real(r), &PgType::INT8) if r.fract() == 0.0 => Bound::I64(r as i64),
        (SqlValue::Real(r), ty) if is_textual(ty) => Bound::Text(r.to_string()),
        (SqlValue::Text(s), &PgType::INT8) => Bound::I64(
            s.parse()
                .map_err(|_| mismatch(&SqlValue::Text(s.clone())))?,
        ),
        (SqlValue::Text(s), &PgType::INT4) => Bound::I32(
            s.parse()
                .map_err(|_| mismatch(&SqlValue::Text(s.clone())))?,
        ),
        (SqlValue::Text(s), &PgType::BYTEA) => Bound::Bytes(s.into_bytes()),
        (SqlValue::Text(s), ty) if is_textual(ty) => Bound::Text(s),
        (SqlValue::Blob(b), &PgType::BYTEA) => Bound::Bytes(b),
        (SqlValue::Blob(b), ty) if is_textual(ty) => {
            Bound::Text(String::from_utf8(b).map_err(Error::backend)?)
        }
        (v, _) => return Err(mismatch(&v)),
    })
}

/// Types that take a UTF-8 string on the wire in binary format.
fn is_textual(ty: &PgType) -> bool {
    matches!(
        *ty,
        PgType::TEXT | PgType::VARCHAR | PgType::BPCHAR | PgType::NAME | PgType::UNKNOWN
    ) || matches!(ty.kind(), Kind::Enum(_))
}

fn read_row(row: &postgres::Row) -> Result<Row> {
    let mut values = Vec::with_capacity(row.len());
    for (i, col) in row.columns().iter().enumerate() {
        let ty = col.type_();
        let get_err =
            |e: postgres::Error| Error::backend(anyhow::anyhow!("column {i} ({}): {e}", ty.name()));
        let value = match *ty {
            PgType::BOOL => row
                .try_get::<_, Option<bool>>(i)
                .map_err(get_err)?
                .map(|b| SqlValue::Integer(i64::from(b))),
            PgType::INT2 => row
                .try_get::<_, Option<i16>>(i)
                .map_err(get_err)?
                .map(|v| SqlValue::Integer(v.into())),
            PgType::INT4 => row
                .try_get::<_, Option<i32>>(i)
                .map_err(get_err)?
                .map(|v| SqlValue::Integer(v.into())),
            PgType::INT8 => row
                .try_get::<_, Option<i64>>(i)
                .map_err(get_err)?
                .map(SqlValue::Integer),
            PgType::FLOAT4 => row
                .try_get::<_, Option<f32>>(i)
                .map_err(get_err)?
                .map(|v| SqlValue::Real(v.into())),
            PgType::FLOAT8 => row
                .try_get::<_, Option<f64>>(i)
                .map_err(get_err)?
                .map(SqlValue::Real),
            PgType::NUMERIC => row
                .try_get::<_, Option<Numeric>>(i)
                .map_err(get_err)?
                .map(|n| n.0),
            PgType::BYTEA => row
                .try_get::<_, Option<Vec<u8>>>(i)
                .map_err(get_err)?
                .map(SqlValue::Blob),
            _ => row
                .try_get::<_, Option<AnyText>>(i)
                .map_err(get_err)?
                .map(|t| SqlValue::Text(t.0)),
        };
        values.push(value.unwrap_or(SqlValue::Null));
    }
    Ok(Row::new(values))
}

/// Any column whose binary form is UTF-8 text (TEXT, VARCHAR, NAME, JSON, enums, …).
struct AnyText(String);

impl<'a> PgFromSql<'a> for AnyText {
    fn from_sql(
        ty: &PgType,
        raw: &'a [u8],
    ) -> std::result::Result<Self, Box<dyn std::error::Error + Sync + Send>> {
        // jsonb's binary form is a version byte followed by the text.
        let raw = if *ty == PgType::JSONB {
            raw.get(1..).unwrap_or_default()
        } else {
            raw
        };
        Ok(AnyText(std::str::from_utf8(raw)?.to_owned()))
    }

    fn accepts(_: &PgType) -> bool {
        true
    }
}

/// `NUMERIC` (what `SUM(bigint)` and `AVG` return), decoded to an integer when it has no
/// fractional part and to a float otherwise.
struct Numeric(SqlValue);

impl<'a> PgFromSql<'a> for Numeric {
    fn from_sql(
        _: &PgType,
        raw: &'a [u8],
    ) -> std::result::Result<Self, Box<dyn std::error::Error + Sync + Send>> {
        decode_numeric(raw)
            .map(Numeric)
            .ok_or_else(|| "malformed numeric".into())
    }

    fn accepts(ty: &PgType) -> bool {
        *ty == PgType::NUMERIC
    }
}

/// Decode PostgreSQL's binary NUMERIC: ndigits, weight, sign, dscale, then base-10000 digits.
pub(crate) fn decode_numeric(raw: &[u8]) -> Option<SqlValue> {
    let word =
        |i: usize| -> Option<i16> { Some(i16::from_be_bytes([*raw.get(i)?, *raw.get(i + 1)?])) };
    let ndigits = usize::try_from(word(0)?).ok()?;
    let weight = i32::from(word(2)?);
    let sign = word(4)? as u16;
    let dscale = word(6)?;
    if sign == 0xC000 {
        return Some(SqlValue::Real(f64::NAN));
    }
    let negative = sign == 0x4000;
    let mut int: i128 = 0;
    let mut frac: f64 = 0.0;
    for d in 0..ndigits {
        let digit = i128::from(word(8 + d * 2)?);
        let power = weight - d as i32;
        if power >= 0 {
            int = int.checked_add(digit.checked_mul(10_000i128.checked_pow(power as u32)?)?)?;
        } else {
            frac += digit as f64 * 10_000f64.powi(power);
        }
    }
    let int = if negative { -int } else { int };
    if dscale == 0 || frac == 0.0 {
        i64::try_from(int).ok().map(SqlValue::Integer)
    } else {
        let f = int as f64 + if negative { -frac } else { frac };
        Some(SqlValue::Real(f))
    }
}

/// Map a driver error onto the shared variants; constraint failures by SQLSTATE class 23.
fn map_err(e: postgres::Error) -> Error {
    if let Some(db) = e.as_db_error() {
        let code = db.code().code();
        if code.starts_with("23") {
            return Error::ConstraintViolation(format!("{code}: {}", db.message()));
        }
        return Error::backend(anyhow::anyhow!("{code}: {}", db.message()));
    }
    Error::backend(e)
}

// ---- dialect --------------------------------------------------------------------------------

pub struct PgDialect;

impl Dialect for PgDialect {
    fn json_text(&self, column: &str, path: &[&str]) -> String {
        format!("(({column})::jsonb #>> '{{{}}}')", path.join(","))
    }

    fn json_array_len(&self, column: &str) -> String {
        format!("jsonb_array_length(({column})::jsonb)")
    }

    fn day_of_millis(&self, column: &str) -> String {
        // The day in the instance's zone. The name comes from `time::zone()`, which only holds an
        // IANA name that chrono-tz knows, but is checked again here since it ends up in SQL. With
        // no name, the current offset stands in for it.
        let zone = crate::time::zone();
        let shift = match zone
            .name
            .filter(|n| n.chars().all(|c| c.is_ascii_alphanumeric() || "_/+-".contains(c)))
        {
            Some(name) => format!("AT TIME ZONE '{name}'"),
            None => {
                let seconds = chrono::Offset::fix(crate::time::now().offset()).local_minus_utc();
                format!("AT TIME ZONE 'UTC' + INTERVAL '{seconds} seconds'")
            }
        };
        format!("to_char(to_timestamp(({column}) / 1000.0) {shift}, 'YYYY-MM-DD')")
    }

    fn key_lock_sql(&self) -> Option<&'static str> {
        Some("SELECT pg_advisory_xact_lock(hashtext(?1))")
    }

    fn next_sequence_value(&self, _table: &str, _column: &str) -> String {
        "DEFAULT".into()
    }

    fn share_lock_suffix(&self) -> &'static str {
        " FOR SHARE"
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn placeholders_follow_sqlite_numbering_and_skip_literals() {
        assert_eq!(
            rewrite_placeholders("SELECT ?1, ?2, ?1"),
            "SELECT $1, $2, $1"
        );
        assert_eq!(rewrite_placeholders("IN (?, ?, ?)"), "IN ($1, $2, $3)");
        assert_eq!(rewrite_placeholders("?3 AND ?"), "$3 AND $4");
        assert_eq!(
            rewrite_placeholders("WHERE a = '?' AND \"q?\" = ?1 -- what?\n AND b = ?2"),
            "WHERE a = '?' AND \"q?\" = $1 -- what?\n AND b = $2"
        );
        assert_eq!(
            rewrite_placeholders("x = 'it''s ?' AND y = ?"),
            "x = 'it''s ?' AND y = $1"
        );
    }

    #[test]
    fn numeric_decodes_integers_and_fractions() {
        // 12345678: ndigits 2, weight 1, sign +, dscale 0, digits [1234, 5678]
        let raw = [0, 2, 0, 1, 0, 0, 0, 0, 0x04, 0xD2, 0x16, 0x2E];
        assert_eq!(decode_numeric(&raw), Some(SqlValue::Integer(12_345_678)));
        // -1.5: ndigits 2, weight 0, sign -, dscale 1, digits [1, 5000]
        let raw = [0, 2, 0, 0, 0x40, 0, 0, 1, 0, 1, 0x13, 0x88];
        assert_eq!(decode_numeric(&raw), Some(SqlValue::Real(-1.5)));
        // 0: ndigits 0
        assert_eq!(
            decode_numeric(&[0, 0, 0, 0, 0, 0, 0, 0]),
            Some(SqlValue::Integer(0))
        );
    }
}
