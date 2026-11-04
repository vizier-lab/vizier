//! Shared helpers for `#[cfg(test)]` modules. Not compiled into the binary.

use std::sync::{Mutex, MutexGuard};

use crate::schema::AgentConfig;

static ENV_LOCK: Mutex<()> = Mutex::new(());

/// Serialize tests that read or mutate process environment variables: cargo runs tests in
/// parallel threads, and one test's `set_var` is every other test's environment.
pub fn env_lock() -> MutexGuard<'static, ()> {
    ENV_LOCK.lock().unwrap_or_else(|poisoned| poisoned.into_inner())
}

/// A minimal, valid dummyplug agent config, so tests that need a stored agent can create one.
pub fn agent_config(id: &str) -> AgentConfig {
    AgentConfig {
        name: format!("Agent {id}"),
        owner_id: None,
        shared_to: vec![],
        system_prompt: None,
        description: None,
        provider: crate::config::provider::ProviderId::DUMMYPLUG,
        model: "dummyplug".into(),
        thinking_depth: 1,
        checkpoint_threshold: 0.8,
        tools: Default::default(),
        silent_read_initiative_chance: 0.0,
        max_tokens: None,
        context_window: None,
        include_documents: None,
        prompt_timeout: Default::default(),
        documents: vec![],
        heartbeat_interval: Default::default(),
        core: None,
        dream_enabled: false,
        dream_schedule: None,
        dream_provider: None,
        dream_model: None,
        avatar_url: None,
        embedding: None,
        indexer: None,
        chunking: Default::default(),
        limits: Default::default(),
        auto_context: Default::default(),
        completion_retry: Default::default(),
    }
}

// ---- PostgreSQL ---------------------------------------------------------------------------

const DATABASE_URL_VAR: &str = "VIZIER_TEST_DATABASE_URL";

/// Schemas older than this are left over from an earlier run and dropped on first use.
const STALE_SCHEMA_MS: i64 = 60 * 60 * 1000;

fn database_url() -> Option<String> {
    std::env::var(DATABASE_URL_VAR)
        .ok()
        .filter(|url| !url.is_empty())
}

fn pg_options() -> crate::storage::sql::PgOptions {
    crate::storage::sql::PgOptions {
        // Tests run in parallel, one pool each; keep them small.
        max_size: 2,
        ..Default::default()
    }
}

/// The connection that creates per-test schemas, set up once per process: it makes sure the
/// `vector` extension exists (an operator prerequisite in production) and drops schemas left
/// behind by earlier runs.
fn admin(url: &str) -> &'static crate::storage::sql::SqlStore {
    static ADMIN: std::sync::OnceLock<crate::storage::sql::SqlStore> = std::sync::OnceLock::new();
    ADMIN.get_or_init(|| {
        let pool = crate::storage::sql::PgPool::connect(url, &pg_options())
            .expect("connect to VIZIER_TEST_DATABASE_URL");
        let store = crate::storage::sql::SqlStore::new(std::sync::Arc::new(pool));
        let conn = store.lock().expect("admin connection");
        conn.execute_batch("CREATE EXTENSION IF NOT EXISTS vector SCHEMA public")
            .expect("create the vector extension");
        let cutoff = chrono::Utc::now().timestamp_millis() - STALE_SCHEMA_MS;
        let stale: Vec<String> = conn
            .prepare("SELECT nspname FROM pg_namespace WHERE nspname LIKE 't\\_%' ESCAPE '\\'")
            .and_then(|mut stmt| {
                stmt.query_map([], |row| row.get::<_, String>(0))?
                    .collect::<crate::storage::sql::Result<Vec<_>>>()
            })
            .expect("list test schemas");
        for schema in stale {
            let created = schema
                .split('_')
                .nth(1)
                .and_then(|ms| ms.parse::<i64>().ok());
            if created.is_some_and(|ms| ms < cutoff) {
                conn.execute_batch(&format!("DROP SCHEMA IF EXISTS {schema} CASCADE"))
                    .expect("drop a stale test schema");
            }
        }
        drop(conn);
        store
    })
}

/// A PostgreSQL store on a fresh, fully migrated schema, or `None` when
/// `VIZIER_TEST_DATABASE_URL` is not set. Every call gets its own schema (`t_<ms>_<uuid>`),
/// selected through the connection's `search_path`, so parallel tests never see each other's
/// rows.
pub fn postgres_store() -> Option<crate::storage::sql::SqlStore> {
    let url = database_url()?;
    let schema = format!(
        "t_{}_{}",
        chrono::Utc::now().timestamp_millis(),
        uuid::Uuid::new_v4().simple()
    );
    admin(&url)
        .lock()
        .and_then(|conn| Ok(conn.execute_batch(&format!("CREATE SCHEMA {schema}"))?))
        .expect("create a test schema");
    let separator = if url.contains('?') { '&' } else { '?' };
    let scoped = format!("{url}{separator}options=-csearch_path%3D{schema}%2Cpublic");
    let pool = crate::storage::sql::PgPool::connect(&scoped, &pg_options())
        .expect("connect to the test schema");
    let store = crate::storage::sql::SqlStore::new(std::sync::Arc::new(pool));
    crate::storage::sql::postgres::migrate(&store.lock().expect("test connection"))
        .expect("migrate the test schema");
    Some(store)
}

thread_local! {
    static ON_POSTGRES: std::cell::Cell<bool> = const { std::cell::Cell::new(false) };
}

/// Run a storage test against PostgreSQL: while `test` runs on this thread, [`test_store`] hands
/// out [`postgres_store`] instead of the test's own SQLite store. Does nothing (the test passes
/// trivially) when `VIZIER_TEST_DATABASE_URL` is not set. Used through [`crate::postgres_tests`].
pub fn on_postgres(test: impl FnOnce()) {
    struct Reset;
    impl Drop for Reset {
        fn drop(&mut self) {
            ON_POSTGRES.set(false);
        }
    }
    if database_url().is_none() {
        return;
    }
    ON_POSTGRES.set(true);
    let _reset = Reset;
    test();
}

/// The store a backend-parametrised test sets up: PostgreSQL (full schema) inside
/// [`on_postgres`], otherwise `sqlite()`, the test's own SQLite store.
pub fn test_store(
    sqlite: impl FnOnce() -> crate::storage::sql::SqlStore,
) -> crate::storage::sql::SqlStore {
    if ON_POSTGRES.get() {
        postgres_store().expect("VIZIER_TEST_DATABASE_URL is set")
    } else {
        sqlite()
    }
}

/// True inside [`on_postgres`], for the few assertions that differ by backend.
pub fn testing_postgres() -> bool {
    ON_POSTGRES.get()
}

/// Run the named tests of the enclosing module a second time on PostgreSQL, as
/// `on_postgres::<name>`. Each test must get its store from a setup that goes through
/// [`test_store`]; tests are `#[test]`/`#[tokio::test]` functions taking no arguments.
#[macro_export]
macro_rules! postgres_tests {
    ($($name:ident),+ $(,)?) => {
        mod on_postgres {
            $(
                #[test]
                fn $name() {
                    $crate::tests_support::on_postgres(super::$name);
                }
            )+
        }
    };
}
