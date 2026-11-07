use super::*;

fn store() -> SqlStore {
    let store = SqlStore::sqlite_in_memory().unwrap();
    store
        .lock()
        .unwrap()
        .execute_batch("CREATE TABLE t (id INTEGER PRIMARY KEY, name TEXT UNIQUE, n INTEGER, r REAL, b BLOB, flag INTEGER)")
        .unwrap();
    store
}

#[test]
fn values_round_trip() {
    let store = store();
    let conn = store.lock().unwrap();
    conn.execute(
        "INSERT INTO t (id, name, n, r, b, flag) VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
        params![1i64, "a", Some(7u32), 1.5f64, vec![0u8, 255], true],
    )
    .unwrap();
    let (name, n, r, b, flag, missing): (String, Option<u32>, f64, Vec<u8>, bool, Option<String>) =
        conn.query_row(
            "SELECT name, n, r, b, flag, NULL FROM t WHERE id = ?1",
            [1i64],
            |row| {
                Ok((
                    row.get(0)?,
                    row.get(1)?,
                    row.get(2)?,
                    row.get(3)?,
                    row.get(4)?,
                    row.get(5)?,
                ))
            },
        )
        .unwrap();
    assert_eq!(
        (name.as_str(), n, r, b, flag, missing),
        ("a", Some(7), 1.5, vec![0, 255], true, None)
    );
}

#[test]
fn no_rows_is_optional_and_wrong_type_names_the_column() {
    let store = store();
    let conn = store.lock().unwrap();
    let none = conn
        .query_row("SELECT name FROM t WHERE id = 9", (), |row| {
            row.get::<_, String>(0)
        })
        .optional()
        .unwrap();
    assert_eq!(none, None);

    conn.execute("INSERT INTO t (id, name) VALUES (1, 'x')", ())
        .unwrap();
    let err = conn
        .query_row("SELECT name FROM t", (), |row| row.get::<_, i64>(0))
        .unwrap_err();
    assert!(
        matches!(err, Error::FromSqlConversionFailure(0, Type::Text, _)),
        "{err}"
    );
}

#[test]
fn unique_violation_is_recognised() {
    let store = store();
    let conn = store.lock().unwrap();
    conn.execute("INSERT INTO t (id, name) VALUES (1, 'x')", ())
        .unwrap();
    let err = conn
        .execute("INSERT INTO t (id, name) VALUES (2, 'x')", ())
        .unwrap_err();
    assert!(err.is_constraint_violation());
    assert!(is_constraint_violation(&anyhow::Error::new(err)));
}

#[test]
fn transaction_commits_rolls_back_and_nests() {
    let store = store();
    let conn = store.lock().unwrap();
    {
        let tx = conn.transaction().unwrap();
        tx.execute("INSERT INTO t (id) VALUES (1)", ()).unwrap();
        {
            let inner = tx.transaction().unwrap();
            inner.execute("INSERT INTO t (id) VALUES (2)", ()).unwrap();
            // dropped: the savepoint rolls back, the outer transaction keeps row 1
        }
        tx.commit().unwrap();
    }
    {
        let tx = conn.transaction().unwrap();
        tx.execute("INSERT INTO t (id) VALUES (3)", ()).unwrap();
        tx.rollback().unwrap();
    }
    let ids: Vec<i64> = conn
        .prepare("SELECT id FROM t ORDER BY id")
        .unwrap()
        .query_map((), |row| row.get(0))
        .unwrap()
        .collect::<Result<_>>()
        .unwrap();
    assert_eq!(ids, vec![1]);
}

#[test]
fn params_from_iter_and_boxed_params_bind_in_order() {
    let store = store();
    let conn = store.lock().unwrap();
    let boxed: Vec<Box<dyn ToSql>> = vec![Box::new(5i64), Box::new("five".to_string())];
    conn.execute(
        "INSERT INTO t (id, name) VALUES (?, ?)",
        params_from_iter(boxed.iter()),
    )
    .unwrap();
    let name: String = conn
        .query_row(
            "SELECT name FROM t WHERE id IN (?1, ?2)",
            params_from_iter([4i64, 5]),
            |r| r.get(0),
        )
        .unwrap();
    assert_eq!(name, "five");
}
