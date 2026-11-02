//! The SQLite backend: one process-wide `rusqlite::Connection` behind a mutex, exactly as storage
//! used it before the SQL layer existed. Statements go through `rusqlite`'s prepared-statement
//! cache.

use std::sync::Arc;

use parking_lot::{Mutex, MutexGuard};
use rusqlite::types::{Value, ValueRef};

use super::{Backend, Dialect, Error, Pool, RawConn, Result, Row, SqlValue};

pub struct SqlitePool {
    conn: Arc<Mutex<rusqlite::Connection>>,
}

impl SqlitePool {
    pub fn new(conn: rusqlite::Connection) -> Self {
        conn.set_prepared_statement_cache_capacity(256);
        Self {
            conn: Arc::new(Mutex::new(conn)),
        }
    }
}

impl Pool for SqlitePool {
    fn backend(&self) -> Backend {
        Backend::Sqlite
    }

    fn connect(&self) -> anyhow::Result<Box<dyn RawConn + '_>> {
        Ok(Box::new(SqliteConn {
            conn: self.conn.lock(),
        }))
    }
}

struct SqliteConn<'p> {
    conn: MutexGuard<'p, rusqlite::Connection>,
}

fn to_value(v: SqlValue) -> Value {
    match v {
        SqlValue::Null => Value::Null,
        SqlValue::Integer(i) => Value::Integer(i),
        SqlValue::Real(r) => Value::Real(r),
        SqlValue::Text(s) => Value::Text(s),
        SqlValue::Blob(b) => Value::Blob(b),
    }
}

fn from_ref(v: ValueRef<'_>) -> Result<SqlValue> {
    Ok(match v {
        ValueRef::Null => SqlValue::Null,
        ValueRef::Integer(i) => SqlValue::Integer(i),
        ValueRef::Real(r) => SqlValue::Real(r),
        ValueRef::Text(t) => {
            SqlValue::Text(std::str::from_utf8(t).map_err(Error::backend)?.to_owned())
        }
        ValueRef::Blob(b) => SqlValue::Blob(b.to_vec()),
    })
}

/// Map a `rusqlite` error onto the shared variants storage code matches on.
pub(crate) fn map_err(e: rusqlite::Error) -> Error {
    match e {
        rusqlite::Error::QueryReturnedNoRows => Error::QueryReturnedNoRows,
        rusqlite::Error::SqliteFailure(f, msg)
            if f.code == rusqlite::ErrorCode::ConstraintViolation =>
        {
            Error::ConstraintViolation(msg.unwrap_or_else(|| f.to_string()))
        }
        other => Error::backend(other),
    }
}

impl RawConn for SqliteConn<'_> {
    fn backend(&self) -> Backend {
        Backend::Sqlite
    }

    fn dialect(&self) -> &'static dyn Dialect {
        &SqliteDialect
    }

    fn execute(&mut self, sql: &str, params: Vec<SqlValue>) -> Result<usize> {
        let mut stmt = self.conn.prepare_cached(sql).map_err(map_err)?;
        stmt.execute(rusqlite::params_from_iter(params.into_iter().map(to_value)))
            .map_err(map_err)
    }

    fn query(&mut self, sql: &str, params: Vec<SqlValue>) -> Result<Vec<Row>> {
        let mut stmt = self.conn.prepare_cached(sql).map_err(map_err)?;
        let width = stmt.column_count();
        let mut rows = stmt
            .query(rusqlite::params_from_iter(params.into_iter().map(to_value)))
            .map_err(map_err)?;
        let mut out = Vec::new();
        while let Some(row) = rows.next().map_err(map_err)? {
            let mut values = Vec::with_capacity(width);
            for i in 0..width {
                values.push(from_ref(row.get_ref(i).map_err(map_err)?)?);
            }
            out.push(Row::new(values));
        }
        Ok(out)
    }

    fn execute_batch(&mut self, sql: &str) -> Result<()> {
        self.conn.execute_batch(sql).map_err(map_err)
    }

    fn as_rusqlite(&self) -> Option<&rusqlite::Connection> {
        Some(&self.conn)
    }
}

pub struct SqliteDialect;

impl Dialect for SqliteDialect {
    fn json_text(&self, column: &str, path: &[&str]) -> String {
        format!("json_extract({column}, '$.{}')", path.join("."))
    }

    fn json_array_len(&self, column: &str) -> String {
        format!("json_array_length({column})")
    }

    fn day_of_millis(&self, column: &str) -> String {
        format!("date({column} / 1000, 'unixepoch')")
    }

    fn key_lock_sql(&self) -> Option<&'static str> {
        None
    }

    fn next_sequence_value(&self, table: &str, column: &str) -> String {
        format!("(SELECT IFNULL(MAX({column}), 0) + 1 FROM {table})")
    }
}
