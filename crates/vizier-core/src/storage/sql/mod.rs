//! The backend-neutral SQL layer (spec 045, contracts/sql-layer.md).
//!
//! Every storage trait has one implementation, written against [`Connection`]. Its surface is the
//! subset of `rusqlite` the storage code already used (`execute`, `query_row`, `prepare` +
//! `query_map`, `params!`, `params_from_iter`, `OptionalExtension`, `Row::get`), so method bodies
//! stay synchronous and read the same on every backend. A backend implements [`RawConn`]:
//! SQLite in [`sqlite`], PostgreSQL in `postgres`. Statements that cannot be written portably
//! ask the [`Dialect`] for the differing fragment instead of branching on the backend.

use std::cell::{Cell, RefCell};
use std::fmt;
use std::sync::Arc;

pub mod postgres;
pub mod sqlite;

pub use postgres::{PgDialect, PgOptions, PgPool};
pub use sqlite::{SqliteDialect, SqlitePool};

/// One SQL value as it crosses the backend boundary.
#[derive(Debug, Clone, PartialEq)]
pub enum SqlValue {
    Null,
    Integer(i64),
    Real(f64),
    Text(String),
    Blob(Vec<u8>),
}

pub mod types {
    pub use super::{FromSql, SqlValue, ToSql};

    /// The storage class of a value, for [`super::Error::FromSqlConversionFailure`].
    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub enum Type {
        Null,
        Integer,
        Real,
        Text,
        Blob,
    }

    impl std::fmt::Display for Type {
        fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            std::fmt::Debug::fmt(self, f)
        }
    }
}

use types::Type;

impl SqlValue {
    pub fn data_type(&self) -> Type {
        match self {
            SqlValue::Null => Type::Null,
            SqlValue::Integer(_) => Type::Integer,
            SqlValue::Real(_) => Type::Real,
            SqlValue::Text(_) => Type::Text,
            SqlValue::Blob(_) => Type::Blob,
        }
    }
}

/// Errors from the SQL layer, mirroring the `rusqlite::Error` variants storage code matches on.
#[derive(Debug)]
pub enum Error {
    /// `query_row` found no row; `OptionalExtension::optional` turns it into `None`.
    QueryReturnedNoRows,
    /// A column could not be read as the requested type.
    FromSqlConversionFailure(
        usize,
        Type,
        Box<dyn std::error::Error + Send + Sync + 'static>,
    ),
    /// A column index past the end of the row.
    InvalidColumnIndex(usize),
    /// A UNIQUE, PRIMARY KEY, FOREIGN KEY, CHECK or NOT NULL constraint refused the write.
    ConstraintViolation(String),
    /// Anything else the backend reported.
    Backend(anyhow::Error),
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Error::QueryReturnedNoRows => f.write_str("query returned no rows"),
            Error::FromSqlConversionFailure(i, t, e) => {
                write!(f, "conversion error in column {i} ({t}): {e}")
            }
            Error::InvalidColumnIndex(i) => write!(f, "invalid column index {i}"),
            Error::ConstraintViolation(m) => write!(f, "constraint violation: {m}"),
            Error::Backend(e) => write!(f, "{e:#}"),
        }
    }
}

impl std::error::Error for Error {}

impl Error {
    pub fn backend(e: impl Into<anyhow::Error>) -> Self {
        Error::Backend(e.into())
    }

    pub fn is_constraint_violation(&self) -> bool {
        matches!(self, Error::ConstraintViolation(_))
    }
}

/// True when `err` (anywhere in its chain) is a constraint violation from either backend.
pub fn is_constraint_violation(err: &anyhow::Error) -> bool {
    err.chain().any(|e| {
        e.downcast_ref::<Error>()
            .is_some_and(Error::is_constraint_violation)
    })
}

pub type Result<T, E = Error> = std::result::Result<T, E>;

// ---- conversions ---------------------------------------------------------------------------

pub trait ToSql {
    fn to_sql(&self) -> Result<SqlValue>;
}

pub trait FromSql: Sized {
    fn from_sql(value: &SqlValue) -> std::result::Result<Self, FromSqlError>;
}

/// Why a value could not be converted; [`Row::get`] adds the column index.
#[derive(Debug)]
pub enum FromSqlError {
    InvalidType,
    OutOfRange(i64),
    Other(Box<dyn std::error::Error + Send + Sync + 'static>),
}

impl fmt::Display for FromSqlError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            FromSqlError::InvalidType => f.write_str("invalid type"),
            FromSqlError::OutOfRange(v) => write!(f, "value {v} out of range"),
            FromSqlError::Other(e) => write!(f, "{e}"),
        }
    }
}

impl std::error::Error for FromSqlError {}

impl ToSql for SqlValue {
    fn to_sql(&self) -> Result<SqlValue> {
        Ok(self.clone())
    }
}

impl<T: ToSql + ?Sized> ToSql for &T {
    fn to_sql(&self) -> Result<SqlValue> {
        (**self).to_sql()
    }
}

impl<T: ToSql + ?Sized> ToSql for Box<T> {
    fn to_sql(&self) -> Result<SqlValue> {
        (**self).to_sql()
    }
}

impl<T: ToSql + ?Sized> ToSql for Arc<T> {
    fn to_sql(&self) -> Result<SqlValue> {
        (**self).to_sql()
    }
}

impl<T: ToSql> ToSql for Option<T> {
    fn to_sql(&self) -> Result<SqlValue> {
        match self {
            Some(v) => v.to_sql(),
            None => Ok(SqlValue::Null),
        }
    }
}

impl ToSql for str {
    fn to_sql(&self) -> Result<SqlValue> {
        Ok(SqlValue::Text(self.to_owned()))
    }
}

impl ToSql for String {
    fn to_sql(&self) -> Result<SqlValue> {
        Ok(SqlValue::Text(self.clone()))
    }
}

impl ToSql for [u8] {
    fn to_sql(&self) -> Result<SqlValue> {
        Ok(SqlValue::Blob(self.to_vec()))
    }
}

impl ToSql for Vec<u8> {
    fn to_sql(&self) -> Result<SqlValue> {
        Ok(SqlValue::Blob(self.clone()))
    }
}

impl<const N: usize> ToSql for [u8; N] {
    fn to_sql(&self) -> Result<SqlValue> {
        Ok(SqlValue::Blob(self.to_vec()))
    }
}

impl ToSql for bool {
    fn to_sql(&self) -> Result<SqlValue> {
        Ok(SqlValue::Integer(i64::from(*self)))
    }
}

impl ToSql for f64 {
    fn to_sql(&self) -> Result<SqlValue> {
        Ok(SqlValue::Real(*self))
    }
}

impl ToSql for f32 {
    fn to_sql(&self) -> Result<SqlValue> {
        Ok(SqlValue::Real(f64::from(*self)))
    }
}

macro_rules! int_to_sql {
    ($($t:ty),*) => {$(
        impl ToSql for $t {
            fn to_sql(&self) -> Result<SqlValue> {
                Ok(SqlValue::Integer(i64::from(*self)))
            }
        }
    )*};
}
int_to_sql!(i8, i16, i32, i64, u8, u16, u32);

macro_rules! wide_int_to_sql {
    ($($t:ty),*) => {$(
        impl ToSql for $t {
            fn to_sql(&self) -> Result<SqlValue> {
                i64::try_from(*self)
                    .map(SqlValue::Integer)
                    .map_err(|e| Error::backend(anyhow::anyhow!("integer {} out of range: {e}", self)))
            }
        }
    )*};
}
wide_int_to_sql!(u64, usize, isize);

impl FromSql for SqlValue {
    fn from_sql(value: &SqlValue) -> std::result::Result<Self, FromSqlError> {
        Ok(value.clone())
    }
}

impl<T: FromSql> FromSql for Option<T> {
    fn from_sql(value: &SqlValue) -> std::result::Result<Self, FromSqlError> {
        match value {
            SqlValue::Null => Ok(None),
            v => T::from_sql(v).map(Some),
        }
    }
}

impl FromSql for String {
    fn from_sql(value: &SqlValue) -> std::result::Result<Self, FromSqlError> {
        match value {
            SqlValue::Text(s) => Ok(s.clone()),
            SqlValue::Blob(b) => {
                String::from_utf8(b.clone()).map_err(|e| FromSqlError::Other(Box::new(e)))
            }
            _ => Err(FromSqlError::InvalidType),
        }
    }
}

impl FromSql for Vec<u8> {
    fn from_sql(value: &SqlValue) -> std::result::Result<Self, FromSqlError> {
        match value {
            SqlValue::Blob(b) => Ok(b.clone()),
            SqlValue::Text(s) => Ok(s.as_bytes().to_vec()),
            _ => Err(FromSqlError::InvalidType),
        }
    }
}

impl FromSql for f64 {
    fn from_sql(value: &SqlValue) -> std::result::Result<Self, FromSqlError> {
        match value {
            SqlValue::Real(r) => Ok(*r),
            SqlValue::Integer(i) => Ok(*i as f64),
            _ => Err(FromSqlError::InvalidType),
        }
    }
}

impl FromSql for f32 {
    fn from_sql(value: &SqlValue) -> std::result::Result<Self, FromSqlError> {
        f64::from_sql(value).map(|v| v as f32)
    }
}

impl FromSql for bool {
    fn from_sql(value: &SqlValue) -> std::result::Result<Self, FromSqlError> {
        match value {
            SqlValue::Integer(i) => Ok(*i != 0),
            _ => Err(FromSqlError::InvalidType),
        }
    }
}

macro_rules! int_from_sql {
    ($($t:ty),*) => {$(
        impl FromSql for $t {
            fn from_sql(value: &SqlValue) -> std::result::Result<Self, FromSqlError> {
                match value {
                    SqlValue::Integer(i) => <$t>::try_from(*i).map_err(|_| FromSqlError::OutOfRange(*i)),
                    _ => Err(FromSqlError::InvalidType),
                }
            }
        }
    )*};
}
int_from_sql!(i8, i16, i32, i64, u8, u16, u32, u64, usize, isize);

// ---- parameters ----------------------------------------------------------------------------

/// Anything that can be bound as a statement's parameters, like `rusqlite::Params`.
pub trait Params {
    fn into_values(self) -> Result<Vec<SqlValue>>;
}

impl Params for () {
    fn into_values(self) -> Result<Vec<SqlValue>> {
        Ok(Vec::new())
    }
}

impl Params for &[&dyn ToSql] {
    fn into_values(self) -> Result<Vec<SqlValue>> {
        self.iter().map(|p| p.to_sql()).collect()
    }
}

/// `[]` and `&[]`: no parameters. Concrete impls so the empty array's element type is inferred.
impl Params for [&dyn ToSql; 0] {
    fn into_values(self) -> Result<Vec<SqlValue>> {
        Ok(Vec::new())
    }
}

impl Params for &[&dyn ToSql; 0] {
    fn into_values(self) -> Result<Vec<SqlValue>> {
        Ok(Vec::new())
    }
}

macro_rules! array_params {
    ($($n:literal)*) => {$(
        impl<T: ToSql> Params for [T; $n] {
            fn into_values(self) -> Result<Vec<SqlValue>> {
                self.iter().map(ToSql::to_sql).collect()
            }
        }

        impl<T: ToSql> Params for &[T; $n] {
            fn into_values(self) -> Result<Vec<SqlValue>> {
                self.iter().map(ToSql::to_sql).collect()
            }
        }
    )*};
}
array_params!(1 2 3 4 5 6 7 8 9 10 11 12 13 14 15 16);

impl Params for Vec<SqlValue> {
    fn into_values(self) -> Result<Vec<SqlValue>> {
        Ok(self)
    }
}

/// `params_from_iter(iter)`: bind every item of an iterator in order.
pub struct ParamsFromIter<I>(I);

pub fn params_from_iter<I>(iter: I) -> ParamsFromIter<I>
where
    I: IntoIterator,
    I::Item: ToSql,
{
    ParamsFromIter(iter)
}

impl<I> Params for ParamsFromIter<I>
where
    I: IntoIterator,
    I::Item: ToSql,
{
    fn into_values(self) -> Result<Vec<SqlValue>> {
        self.0.into_iter().map(|p| p.to_sql()).collect()
    }
}

/// `params![a, b, …]`, like `rusqlite::params!`.
#[macro_export]
macro_rules! sql_params {
    () => { &[] as &[&dyn $crate::storage::sql::ToSql] };
    ($($p:expr),+ $(,)?) => { &[$(&$p as &dyn $crate::storage::sql::ToSql),+] as &[&dyn $crate::storage::sql::ToSql] };
}
pub use crate::sql_params as params;

// ---- rows ----------------------------------------------------------------------------------

/// One result row: every column already read into a [`SqlValue`].
#[derive(Debug, Clone)]
pub struct Row {
    values: Vec<SqlValue>,
}

impl Row {
    pub fn new(values: Vec<SqlValue>) -> Self {
        Self { values }
    }

    /// Column `idx` as `T`, like `rusqlite::Row::get`.
    pub fn get<I: RowIndex, T: FromSql>(&self, idx: I) -> Result<T> {
        let idx = idx.index();
        let value = self.values.get(idx).ok_or(Error::InvalidColumnIndex(idx))?;
        T::from_sql(value).map_err(|e| match e {
            FromSqlError::Other(inner) => {
                Error::FromSqlConversionFailure(idx, value.data_type(), inner)
            }
            other => Error::FromSqlConversionFailure(idx, value.data_type(), Box::new(other)),
        })
    }

    pub fn get_value(&self, idx: usize) -> Result<&SqlValue> {
        self.values.get(idx).ok_or(Error::InvalidColumnIndex(idx))
    }

    pub fn len(&self) -> usize {
        self.values.len()
    }

    pub fn is_empty(&self) -> bool {
        self.values.is_empty()
    }

    pub fn into_values(self) -> Vec<SqlValue> {
        self.values
    }
}

pub trait RowIndex {
    fn index(self) -> usize;
}

impl RowIndex for usize {
    fn index(self) -> usize {
        self
    }
}

impl RowIndex for i32 {
    fn index(self) -> usize {
        self as usize
    }
}

// ---- dialect -------------------------------------------------------------------------------

/// The fragments of SQL that differ between backends. Add a method only when a statement
/// cannot be written portably.
pub trait Dialect: Send + Sync {
    /// A TEXT-valued expression reading `path` (object keys) out of the JSON in `column`.
    fn json_text(&self, column: &str, path: &[&str]) -> String;
    /// The number of elements of the JSON array in `column`.
    fn json_array_len(&self, column: &str) -> String;
    /// The UTC calendar day (`YYYY-MM-DD`) of a millisecond timestamp column.
    fn day_of_millis(&self, column: &str) -> String;
    /// A statement taking an exclusive lock on the text key `?1` until the transaction ends, so
    /// read-then-insert sequences (revision numbers) cannot interleave. `None` where the
    /// transaction itself already excludes other writers (SQLite's `BEGIN IMMEDIATE`).
    fn key_lock_sql(&self) -> Option<&'static str>;
    /// The value an `INSERT` writes into `column` of `table` so it numbers rows in insertion
    /// order: `MAX + 1` on SQLite (safe under its single writer), `DEFAULT` on PostgreSQL, where
    /// the column is an identity.
    fn next_sequence_value(&self, table: &str, column: &str) -> String;
}

/// Which backend a store runs on, for diagnostics and backend-specific setup only.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Backend {
    Sqlite,
    Postgres,
}

// ---- backend seam --------------------------------------------------------------------------

/// What a backend connection implements. Object-safe and minimal; [`Connection`] builds the
/// `rusqlite`-shaped API on top.
pub trait RawConn {
    fn backend(&self) -> Backend;
    fn dialect(&self) -> &'static dyn Dialect;
    fn execute(&mut self, sql: &str, params: Vec<SqlValue>) -> Result<usize>;
    fn query(&mut self, sql: &str, params: Vec<SqlValue>) -> Result<Vec<Row>>;
    /// Several statements without parameters (schema, `BEGIN`, `SAVEPOINT`, …).
    fn execute_batch(&mut self, sql: &str) -> Result<()>;
    /// The underlying SQLite connection, for SQLite-only schema setup and legacy migrations.
    fn as_rusqlite(&self) -> Option<&rusqlite::Connection> {
        None
    }
}

/// A connection checked out of a [`SqlStore`]. Methods take `&self` like `rusqlite`, so the
/// helpers that pass `&Connection` around keep their shape.
pub struct Connection<'c> {
    raw: RefCell<Box<dyn RawConn + 'c>>,
    depth: Cell<u32>,
}

impl<'c> Connection<'c> {
    pub fn new(raw: Box<dyn RawConn + 'c>) -> Self {
        Self {
            raw: RefCell::new(raw),
            depth: Cell::new(0),
        }
    }

    pub fn backend(&self) -> Backend {
        self.raw.borrow().backend()
    }

    pub fn dialect(&self) -> &'static dyn Dialect {
        self.raw.borrow().dialect()
    }

    pub fn execute<P: Params>(&self, sql: &str, params: P) -> Result<usize> {
        let values = params.into_values()?;
        self.raw.borrow_mut().execute(sql, values)
    }

    pub fn execute_batch(&self, sql: &str) -> Result<()> {
        self.raw.borrow_mut().execute_batch(sql)
    }

    /// All rows of a query.
    pub fn query_all<P: Params>(&self, sql: &str, params: P) -> Result<Vec<Row>> {
        let values = params.into_values()?;
        self.raw.borrow_mut().query(sql, values)
    }

    /// The first row mapped by `f`; [`Error::QueryReturnedNoRows`] when there is none.
    pub fn query_row<T, P, F>(&self, sql: &str, params: P, f: F) -> Result<T>
    where
        P: Params,
        F: FnOnce(&Row) -> Result<T>,
    {
        let rows = self.query_all(sql, params)?;
        match rows.first() {
            Some(row) => f(row),
            None => Err(Error::QueryReturnedNoRows),
        }
    }

    /// A statement handle; it only remembers the SQL text (statements are cached per backend).
    pub fn prepare<'s>(&'s self, sql: &str) -> Result<Statement<'s, 'c>> {
        Ok(Statement {
            conn: self,
            sql: sql.to_owned(),
        })
    }

    /// Same as [`Self::prepare`]; kept for parity with `rusqlite`.
    pub fn prepare_cached<'s>(&'s self, sql: &str) -> Result<Statement<'s, 'c>> {
        self.prepare(sql)
    }

    /// Start a transaction (a savepoint when one is already open). It rolls back on drop unless
    /// [`Transaction::commit`] is called.
    pub fn transaction(&self) -> Result<Transaction<'_, 'c>> {
        let depth = self.depth.get();
        if depth == 0 {
            self.execute_batch(match self.backend() {
                // IMMEDIATE takes SQLite's write lock up front, so a read-then-write inside the
                // transaction cannot be overtaken by another writer.
                Backend::Sqlite => "BEGIN IMMEDIATE",
                Backend::Postgres => "BEGIN",
            })?;
        } else {
            self.execute_batch(&format!("SAVEPOINT vz_sp_{depth}"))?;
        }
        self.depth.set(depth + 1);
        Ok(Transaction {
            conn: self,
            depth,
            done: false,
        })
    }

    /// Same as [`Self::transaction`]; `rusqlite` distinguishes them only by borrow.
    pub fn unchecked_transaction(&self) -> Result<Transaction<'_, 'c>> {
        self.transaction()
    }

    /// Lock `key` until the current transaction ends (see [`Dialect::key_lock_sql`]). Call it
    /// inside a transaction.
    pub fn lock_key(&self, key: &str) -> Result<()> {
        if let Some(sql) = self.dialect().key_lock_sql() {
            self.execute(sql, [key])?;
        }
        Ok(())
    }

    /// Run `f` inside a transaction: commit on `Ok`, roll back on `Err`.
    pub fn in_transaction<T, E>(
        &self,
        f: impl FnOnce(&Connection<'c>) -> std::result::Result<T, E>,
    ) -> std::result::Result<T, E>
    where
        E: From<Error>,
    {
        let tx = self.transaction()?;
        let out = f(self)?;
        tx.commit()?;
        Ok(out)
    }

    /// The SQLite connection underneath, for SQLite-only code (schema init, legacy migrations,
    /// sqlite-vec). `None` on other backends.
    pub fn with_rusqlite<T>(&self, f: impl FnOnce(&rusqlite::Connection) -> T) -> Option<T> {
        let raw = self.raw.borrow();
        raw.as_rusqlite().map(f)
    }
}

pub struct Statement<'s, 'c> {
    conn: &'s Connection<'c>,
    sql: String,
}

impl Statement<'_, '_> {
    /// Map every row with `f`. Returns an iterator of per-row results so
    /// `.collect::<Result<Vec<_>, _>>()` call sites read as they did with `rusqlite`.
    pub fn query_map<T, P, F>(
        &mut self,
        params: P,
        mut f: F,
    ) -> Result<std::vec::IntoIter<Result<T>>>
    where
        P: Params,
        F: FnMut(&Row) -> Result<T>,
    {
        let rows = self.conn.query_all(&self.sql, params)?;
        Ok(rows.iter().map(&mut f).collect::<Vec<_>>().into_iter())
    }

    pub fn query_row<T, P, F>(&mut self, params: P, f: F) -> Result<T>
    where
        P: Params,
        F: FnOnce(&Row) -> Result<T>,
    {
        self.conn.query_row(&self.sql, params, f)
    }

    pub fn execute<P: Params>(&mut self, params: P) -> Result<usize> {
        self.conn.execute(&self.sql, params)
    }

    pub fn exists<P: Params>(&mut self, params: P) -> Result<bool> {
        Ok(!self.conn.query_all(&self.sql, params)?.is_empty())
    }
}

/// An open transaction or savepoint. Derefs to the connection, like `rusqlite::Transaction`.
pub struct Transaction<'t, 'c> {
    conn: &'t Connection<'c>,
    depth: u32,
    done: bool,
}

impl<'c> Transaction<'_, 'c> {
    pub fn commit(mut self) -> Result<()> {
        self.finish(true)
    }

    pub fn rollback(mut self) -> Result<()> {
        self.finish(false)
    }

    fn finish(&mut self, commit: bool) -> Result<()> {
        if self.done {
            return Ok(());
        }
        self.done = true;
        self.conn.depth.set(self.depth);
        let sql = match (self.depth, commit) {
            (0, true) => "COMMIT".to_string(),
            (0, false) => "ROLLBACK".to_string(),
            (d, true) => format!("RELEASE SAVEPOINT vz_sp_{d}"),
            (d, false) => format!("ROLLBACK TO SAVEPOINT vz_sp_{d}; RELEASE SAVEPOINT vz_sp_{d}"),
        };
        self.conn.execute_batch(&sql)
    }
}

impl<'c> std::ops::Deref for Transaction<'_, 'c> {
    type Target = Connection<'c>;

    fn deref(&self) -> &Self::Target {
        self.conn
    }
}

impl Drop for Transaction<'_, '_> {
    fn drop(&mut self) {
        if !self.done
            && let Err(e) = self.finish(false)
        {
            tracing::warn!("rollback failed: {e}");
        }
    }
}

/// `.optional()` on a `query_row` result, like `rusqlite::OptionalExtension`.
pub trait OptionalExtension<T> {
    fn optional(self) -> Result<Option<T>>;
}

impl<T> OptionalExtension<T> for Result<T> {
    fn optional(self) -> Result<Option<T>> {
        match self {
            Ok(v) => Ok(Some(v)),
            Err(Error::QueryReturnedNoRows) => Ok(None),
            Err(e) => Err(e),
        }
    }
}

// ---- store ---------------------------------------------------------------------------------

/// What a backend pool implements.
pub trait Pool: Send + Sync {
    fn backend(&self) -> Backend;
    fn connect(&self) -> anyhow::Result<Box<dyn RawConn + '_>>;
}

/// The handle every storage implementation holds: cheap to clone, hands out [`Connection`]s.
#[derive(Clone)]
pub struct SqlStore {
    pool: Arc<dyn Pool>,
}

impl SqlStore {
    pub fn new(pool: Arc<dyn Pool>) -> Self {
        Self { pool }
    }

    /// A SQLite store over an existing connection (schema already initialised by the caller).
    pub fn sqlite(conn: rusqlite::Connection) -> Self {
        Self::new(Arc::new(SqlitePool::new(conn)))
    }

    /// An in-memory SQLite store with no schema, for tests.
    pub fn sqlite_in_memory() -> anyhow::Result<Self> {
        Ok(Self::sqlite(rusqlite::Connection::open_in_memory()?))
    }

    pub fn backend(&self) -> Backend {
        self.pool.backend()
    }

    /// Check out a connection. On SQLite this takes the process-wide connection lock, as before;
    /// on PostgreSQL it waits for a pooled connection and fails after the pool's timeout.
    pub fn lock(&self) -> anyhow::Result<Connection<'_>> {
        Ok(Connection::new(self.pool.connect()?))
    }
}

impl fmt::Debug for SqlStore {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("SqlStore")
            .field("backend", &self.backend())
            .finish()
    }
}

#[cfg(test)]
mod tests;
