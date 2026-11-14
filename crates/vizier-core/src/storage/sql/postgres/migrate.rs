//! PostgreSQL schema migrations (spec 045, data-model.md `vizier_schema_migrations`).
//!
//! Every file under `migrations/` is embedded and applied once, in version order, inside one
//! transaction that holds a database-wide advisory lock, so two servers starting against the same
//! database cannot both apply a file. A database already migrated past what this binary knows is
//! refused rather than written with an older schema's assumptions.

use anyhow::Context;

use crate::storage::sql::{Backend, Connection, OptionalExtension, params};

/// One embedded migration file: `NNNN_<name>.sql`.
struct Migration {
    version: i64,
    name: &'static str,
    sql: &'static str,
}

/// In version order. A new file is appended here; an applied file is never edited.
const MIGRATIONS: &[Migration] = &[
    Migration {
        version: 1,
        name: "0001_initial",
        sql: include_str!("migrations/0001_initial.sql"),
    },
    Migration {
        version: 2,
        name: "0002_client_call_claim",
        sql: include_str!("migrations/0002_client_call_claim.sql"),
    },
    Migration {
        version: 3,
        name: "0003_notification_source",
        sql: include_str!("migrations/0003_notification_source.sql"),
    },
    Migration {
        version: 4,
        name: "0004_workspaces",
        sql: include_str!("migrations/0004_workspaces.sql"),
    },
    Migration {
        version: 5,
        name: "0005_project_lsp",
        sql: include_str!("migrations/0005_project_lsp.sql"),
    },
    Migration {
        version: 6,
        name: "0006_workspace_shown",
        sql: include_str!("migrations/0006_workspace_shown.sql"),
    },
    Migration {
        version: 7,
        name: "0007_row_ownership",
        sql: include_str!("migrations/0007_row_ownership.sql"),
    },
    Migration {
        version: 8,
        name: "0008_detached_scripts",
        sql: include_str!("migrations/0008_detached_scripts.sql"),
    },
];

/// Bring the schema `conn` sees (its `search_path`) up to the newest embedded migration.
///
/// The `vector` extension is an operator prerequisite (creating it needs more privilege than the
/// application should hold), so it is only checked for here.
pub fn migrate(conn: &Connection<'_>) -> anyhow::Result<()> {
    anyhow::ensure!(
        conn.backend() == Backend::Postgres,
        "storage: PostgreSQL migrations run on a PostgreSQL connection only"
    );

    let has_vector = conn
        .query_row(
            "SELECT 1 FROM pg_extension WHERE extname = 'vector'",
            [],
            |row| row.get::<_, i64>(0),
        )
        .optional()
        .context("storage: cannot read PostgreSQL extensions")?
        .is_some();
    anyhow::ensure!(
        has_vector,
        "storage: PostgreSQL is missing the \"vector\" extension (CREATE EXTENSION vector)"
    );

    let tx = conn.transaction()?;
    tx.execute_batch("SELECT pg_advisory_xact_lock(hashtext('vizier_schema'))")
        .context("storage: cannot take the schema migration lock")?;
    tx.execute_batch(
        "CREATE TABLE IF NOT EXISTS vizier_schema_migrations (
            version    BIGINT PRIMARY KEY,
            name       TEXT   NOT NULL,
            applied_at BIGINT NOT NULL
        )",
    )
    .context("storage: cannot create vizier_schema_migrations")?;

    let applied: Vec<i64> = tx
        .prepare("SELECT version FROM vizier_schema_migrations ORDER BY version")?
        .query_map([], |row| row.get(0))?
        .collect::<crate::storage::sql::Result<_>>()?;
    let known = MIGRATIONS.last().map_or(0, |m| m.version);
    if let Some(&newest) = applied.last()
        && newest > known
    {
        anyhow::bail!(
            "storage: database schema version {newest} is newer than this binary ({known})"
        );
    }

    for migration in MIGRATIONS {
        if applied.contains(&migration.version) {
            continue;
        }
        tx.execute_batch(migration.sql)
            .with_context(|| format!("storage: migration {} failed", migration.name))?;
        tx.execute(
            "INSERT INTO vizier_schema_migrations (version, name, applied_at) VALUES (?1, ?2, ?3)",
            params![
                migration.version,
                migration.name,
                crate::time::now().timestamp_millis()
            ],
        )?;
        tracing::info!(
            version = migration.version,
            name = migration.name,
            "applied PostgreSQL migration"
        );
    }
    tx.commit()?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A file saved with a UTF-8 BOM starts with U+FEFF, which PostgreSQL takes as part of the
    /// first token and refuses (`syntax error at or near "\u{feff}"`). It needs no database to
    /// catch, and it is invisible in an editor, so it is checked here for every embedded file.
    #[test]
    fn no_migration_carries_a_byte_order_mark() {
        for migration in MIGRATIONS {
            assert!(
                !migration.sql.contains('\u{feff}'),
                "{} carries a UTF-8 BOM: save it as plain UTF-8",
                migration.name
            );
        }
    }

    #[test]
    fn migrating_twice_is_a_no_op_and_a_newer_schema_is_refused() {
        let Some(store) = crate::tests_support::postgres_store() else {
            return;
        };
        let conn = store.lock().unwrap();
        migrate(&conn).unwrap();
        let applied: i64 = conn
            .query_row("SELECT COUNT(*) FROM vizier_schema_migrations", [], |r| {
                r.get(0)
            })
            .unwrap();
        assert_eq!(applied, MIGRATIONS.len() as i64);

        conn.execute(
            "INSERT INTO vizier_schema_migrations (version, name, applied_at) VALUES (999, 'future', 0)",
            [],
        )
        .unwrap();
        let err = migrate(&conn).unwrap_err().to_string();
        assert_eq!(
            err,
            format!(
                "storage: database schema version 999 is newer than this binary ({})",
                MIGRATIONS.len()
            )
        );
    }
}
