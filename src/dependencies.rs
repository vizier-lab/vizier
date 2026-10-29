use std::sync::Arc;

use anyhow::Result;
use parking_lot::Mutex;
use rusqlite::OptionalExtension;

use crate::storage::turn::TurnStorage;
use crate::{
    agents::background::BackgroundJobs,
    config::{VizierConfig, storage::StorageConfig},
    constant::CORE_MD,
    file_manager::FileManager,
    indexer::{VizierIndexer, noop::NoopIndexer},
    schema::{
        RevisionOrigin, RevisionTrigger, SessionHistory, SessionHistoryContent, VizierAttachment,
    },
    storage::{
        VizierStorage,
        agent::AgentStorage,
        background_job::BackgroundJobStorage,
        client_call::ClientCallStorage,
        document::LocalDocumentStore,
        dream::DreamStorage,
        dream_journal::DreamJournalStorage,
        fs::FileSystemStorage,
        global_config::GlobalConfigStorage,
        memory_bundle::BundleMemoryStore,
        scratch::ScratchStorage,
        session::SessionStorage,
        session_file::SessionFileStorage,
        sqlite::SqliteStorage,
        state::StateStorage,
        task::TaskStorage,
        task_run::TaskRunStorage,
        user::{AVAILABLE_PERMISSIONS, UserStorage},
    },
    transport::VizierTransport,
    utils::build_path,
};

const MIGRATION_MEMORY_TO_BUNDLES: &str = "migration_memory_to_bundles_v1";
const MIGRATION_FS_TO_SQLITE: &str = "migration_fs_to_sqlite_v1";
const LEGACY_GLOBAL_AGENT_ID: &str = "_global";

#[derive(Clone)]
pub struct VizierDependencies {
    pub config: Arc<VizierConfig>,
    pub storage: Arc<VizierStorage>,
    pub sqlite_conn: Arc<Mutex<rusqlite::Connection>>,
    pub transport: VizierTransport,
    pub file_manager: FileManager,
    /// Permanent artifacts (`specs/035-add-artifacts`): metadata in SQLite, content on disk,
    /// and the one resolver for artifact attachment bytes.
    pub artifacts: Arc<crate::artifacts::Artifacts>,
    /// The transient session-file tier's one clear/delete service.
    pub session_files: Arc<crate::session_files::SessionFiles>,
    pub background_jobs: BackgroundJobs,
    /// The one HTTP client every model provider request goes through (`llm::http`).
    pub http: reqwest::Client,
    /// The provider and model catalogue (spec 015).
    pub catalogue: crate::catalogue::Catalogue,
    /// Every integration kind (spec 016).
    pub integration_kinds: Arc<crate::integrations::IntegrationKinds>,
    /// Addon errors, written by addon tools and read for status.
    pub integration_health: crate::integrations::IntegrationHealth,
    /// Each channel connection's reader health, written by `ChannelConnections` and read by
    /// agent health and integration status.
    pub channel_board: crate::channels::connections::ChannelBoard,
    /// Live agent setup/build progress (model downloads included), read by the WebUI.
    pub setup: crate::setup::SetupBoard,
    /// The notification inbox (spec 029).
    pub notifications: crate::notifications::Notifications,
}

/// Old (pre-bundle) memory frontmatter shape, kept only so `migrate_memory_to_bundles` can parse
/// concept documents written by a `--storage filesystem` deployment before this feature shipped.
#[derive(Debug, Clone, serde::Deserialize)]
struct LegacyMemoryFrontMatter {
    title: String,
    timestamp: chrono::DateTime<chrono::Utc>,
    agent_id: String,
    #[serde(default)]
    tags: Vec<String>,
    #[serde(default)]
    attachments: Vec<VizierAttachment>,
    #[serde(default)]
    read_count: u64,
}

/// Old (pre-bundle) memory row shape, as it was persisted as one flat JSON blob in the legacy
/// sqlite `memory` table (data-model.md's Memory Concept Document is its bundle-aware successor).
#[derive(Debug, serde::Deserialize)]
struct LegacyMemoryRow {
    slug: String,
    title: String,
    #[serde(default)]
    content: String,
    timestamp: chrono::DateTime<chrono::Utc>,
    agent_id: String,
    #[serde(default)]
    tags: Vec<String>,
    #[serde(default)]
    attachments: Vec<VizierAttachment>,
    #[serde(default)]
    read_count: u64,
}

impl VizierDependencies {
    pub async fn new(config: VizierConfig) -> Result<Self> {
        let conn = SqliteStorage::open_connection(&config.workspace)?;
        let conn = Arc::new(Mutex::new(conn));

        let document_store: Arc<dyn crate::storage::document::DocumentStore> = Arc::new(
            LocalDocumentStore::new(build_path(&config.workspace, &["agents"])),
        );

        // The workspace key must exist and match before anything can write a secret. Loading it
        // here also means a missing/wrong/malformed key fails before the filesystem import adds
        // any plaintext (specs/031-encrypt-agent-secrets contracts/migration.md §1).
        let cipher = {
            let c = conn.lock();
            crate::encryption::load_or_generate(
                &config.workspace,
                &c,
                &crate::encryption::KeyRequest::from_env(),
            )?
        };
        let integration_kinds = Arc::new(crate::integrations::IntegrationKinds::builtin());
        let secret_paths: Arc<dyn crate::encryption::IntegrationSecretPaths> =
            integration_kinds.clone();

        // Deployments that were previously on the `filesystem` backend keep their memories on
        // disk as flat files and every other entity as loose files too; both are folded into
        // the sole sqlite backend by the two migrations below before anything else runs
        // (contracts/migration.md). `StorageConfig::Filesystem` survives purely so an old
        // `.vizier.yaml`/env var naming it can still be parsed and detected here — it is never
        // used to select a live backend (research.md §10, FR-025).
        if matches!(config.storage, StorageConfig::Filesystem) {
            tracing::warn!(
                "the 'filesystem' storage backend is no longer supported at runtime; this deployment's data is being migrated into the embedded sqlite database. Update your config to sqlite (or drop --storage/VIZIER_STORAGE) going forward."
            );
        }

        Self::migrate_memory_to_bundles(&config, document_store.clone(), conn.clone()).await?;
        Self::migrate_filesystem_backend_to_sqlite(
            &config,
            conn.clone(),
            cipher.clone(),
            secret_paths.clone(),
        )
        .await?;

        let catalogue = crate::catalogue::Catalogue::new(crate::catalogue::seed::load_seed());
        crate::provider_migration::migrate_provider_ids_v1(
            &mut conn.lock(),
            &catalogue.snapshot(),
        )?;
        Self::warn_about_yaml_providers(&config, &catalogue);

        // One-time plaintext conversion, before the main storage exists. No agent, channel
        // reader or scheduler can read a value mid-conversion (FR-017).
        {
            let c = conn.lock();
            crate::encryption::migrate::migrate_secrets_to_encrypted(&c, &cipher, &*secret_paths)?;
        }

        let storage = VizierStorage::new(SqliteStorage::with_secrets(
            conn.clone(),
            document_store,
            cipher,
            secret_paths,
        ));

        Self::migrate_users(&storage).await?;
        Self::migrate_agent_cores(&storage).await?;
        Self::migrate_task_requester(conn.clone()).await?;
        let interrupted_runs = Self::interrupt_orphaned_task_runs(&storage).await?;
        let interrupted_jobs = Self::interrupt_orphaned_background_jobs(&storage).await;
        Self::interrupt_orphaned_client_calls(&storage).await;
        Self::clear_orphaned_scratch(&storage).await;
        crate::skill::builtin::seed_builtin_skills(&config.workspace, &storage).await;
        // Never blocks startup: it is retried on the next start until it has finished.
        if let Err(e) = Self::migrate_reply_usage(&storage, conn.clone()).await {
            tracing::error!("failed to carry reply usage over into usage accounting: {e}");
        }

        let transport = VizierTransport::durable(storage.clone());
        let file_manager = FileManager::new(config.workspace.clone());

        let fm = file_manager.clone();
        let file_transport = transport.clone();
        tokio::spawn(async move {
            fm.run(file_transport).await;
        });

        // The refreshed catalogue cached in SQLite, when it's newer than the seed. Never
        // touches the network.
        catalogue
            .swap(crate::catalogue::cache::newest(&storage, (*catalogue.snapshot()).clone()).await);
        // The cached OpenRouter media lists, also newer-only, and their stored last failures.
        // Never touches the network (spec 028 FR-019/SC-011).
        catalogue.load_media_cache(&storage).await;

        let storage = Arc::new(storage);
        // Artifacts and session files are constructed once; the file manager gets the artifact
        // resolver installed after construction because the two cannot own each other (R3).
        let artifacts = Arc::new(crate::artifacts::Artifacts::new(
            config.workspace.clone(),
            storage.clone(),
            file_manager.clone(),
        ));
        file_manager.install_artifact_resolver(
            artifacts.clone() as Arc<dyn crate::schema::AttachmentResolver>
        );
        let session_files = Arc::new(crate::session_files::SessionFiles::new(
            storage.clone(),
            file_manager.clone(),
        ));
        let notifications = crate::notifications::Notifications::new(storage.clone());
        // The interrupted runs and jobs the sweeps above closed are news: record them now that
        // the service exists. Best-effort, like every notification write.
        Self::record_interrupted(&notifications, &storage, interrupted_runs, interrupted_jobs)
            .await;
        if let Err(e) = notifications.purge_orphans().await {
            tracing::warn!("failed to clear orphaned notifications: {e}");
        }
        match notifications
            .purge_before(
                chrono::Utc::now()
                    - chrono::Duration::days(crate::storage::notification::RETENTION_DAYS),
            )
            .await
        {
            Ok(0) => {}
            Ok(purged) => tracing::info!("purged {} notification(s) past retention", purged),
            Err(e) => tracing::warn!("failed to purge old notifications: {e}"),
        }
        let background_jobs =
            BackgroundJobs::new(storage.clone(), transport.clone(), notifications.clone());

        // Files saved to a session (by any tool or attachment) reach its open sockets as
        // `session_file` frames.
        if let Some(mut files) =
            crate::storage::session_file::SessionFileStorage::subscribe_session_files(&*storage)
        {
            let transport = transport.clone();
            tokio::spawn(async move {
                loop {
                    match files.recv().await {
                        Ok((session, file)) => {
                            transport.publish_session_event(crate::transport::SessionEvent {
                                session,
                                frame: crate::transport::SessionFrame::File(file),
                            })
                        }
                        Err(tokio::sync::broadcast::error::RecvError::Lagged(_)) => {}
                        Err(tokio::sync::broadcast::error::RecvError::Closed) => break,
                    }
                }
            });
        }

        Ok(Self {
            config: Arc::new(config.clone()),
            storage,
            sqlite_conn: conn,
            transport,
            file_manager,
            artifacts,
            session_files,
            background_jobs,
            http: crate::llm::http::client(),
            catalogue,
            integration_kinds,
            integration_health: Default::default(),
            channel_board: Default::default(),
            setup: Default::default(),
            notifications,
        })
    }

    /// When complete usage accounting started, set once [`Self::migrate_reply_usage`] has finished.
    pub const USAGE_ACCOUNTING_SINCE: &'static str = "usage.accounting_since";

    /// Spec 018 (research R6): copy the usage every saved reply recorded into `usage_event`, so
    /// totals from before the upgrade are unchanged. Runs once; the state key it sets is also
    /// what tells the usage page where complete accounting starts.
    async fn migrate_reply_usage(
        storage: &VizierStorage,
        conn: Arc<Mutex<rusqlite::Connection>>,
    ) -> Result<()> {
        use crate::storage::{state::StateStorage, usage::UsageStorage};

        if storage
            .get_state(Self::USAGE_ACCOUNTING_SINCE.to_string())
            .await?
            .is_some()
        {
            return Ok(());
        }

        // A run that failed partway left some batches in: start over, so none count twice.
        conn.lock()
            .execute("DELETE FROM usage_event WHERE legacy = 1", [])?;

        let mut after: i64 = 0;
        let mut carried = 0usize;
        loop {
            let batch: Vec<(i64, String)> = {
                let conn = conn.lock();
                let mut stmt = conn.prepare(
                    "SELECT rowid, data FROM session_history \
                     WHERE content_type = 'Response' AND rowid > ?1 ORDER BY rowid LIMIT 1000",
                )?;
                stmt.query_map([after], |row| Ok((row.get(0)?, row.get(1)?)))?
                    .collect::<rusqlite::Result<_>>()?
            };
            let Some((last, _)) = batch.last() else {
                break;
            };
            after = *last;
            let events: Vec<_> = batch
                .iter()
                .filter_map(|(_, data)| serde_json::from_str::<SessionHistory>(data).ok())
                .filter_map(|history| reply_usage_event(&history))
                .collect();
            carried += events.len();
            storage.insert_legacy_usage(events).await?;
        }

        storage
            .save_state(
                Self::USAGE_ACCOUNTING_SINCE.to_string(),
                serde_json::Value::String(chrono::Utc::now().to_rfc3339()),
            )
            .await?;
        if carried > 0 {
            tracing::info!(
                replies = carried,
                "carried reply usage over into usage accounting"
            );
        }
        Ok(())
    }

    fn migration_done(conn: &rusqlite::Connection, marker: &str) -> Result<bool> {
        let value: Option<String> = conn
            .query_row(
                "SELECT value FROM state WHERE key = ?1",
                rusqlite::params![marker],
                |row| row.get(0),
            )
            .optional()?;
        Ok(value.is_some())
    }

    fn mark_migration_done(conn: &rusqlite::Connection, marker: &str) -> Result<()> {
        conn.execute(
            "INSERT OR REPLACE INTO state (key, value) VALUES (?1, ?2)",
            rusqlite::params![marker, "true"],
        )?;
        Ok(())
    }

    async fn unique_migration_path(
        bundle_store: &BundleMemoryStore,
        agent_id: &str,
        bundle: &str,
        base_path: &str,
    ) -> String {
        let mut candidate = base_path.to_string();
        let mut n = 2;
        loop {
            match bundle_store
                .get_memory_detail(
                    agent_id.to_string(),
                    Some(bundle.to_string()),
                    candidate.clone(),
                )
                .await
            {
                Ok(None) | Err(_) => return candidate,
                Ok(Some(_)) => {
                    candidate = format!("{base_path}-{n}");
                    n += 1;
                }
            }
        }
    }

    /// Part A (contracts/migration.md): every existing memory, regardless of source backend,
    /// becomes a concept document in its owning agent's default bundle. Runs for every
    /// deployment (a fresh sqlite install with no legacy `memory` rows, or one already
    /// migrated, is a no-op).
    async fn migrate_memory_to_bundles(
        config: &VizierConfig,
        document_store: Arc<dyn crate::storage::document::DocumentStore>,
        conn: Arc<Mutex<rusqlite::Connection>>,
    ) -> Result<()> {
        {
            let c = conn.lock();
            if Self::migration_done(&c, MIGRATION_MEMORY_TO_BUNDLES)? {
                return Ok(());
            }
        }

        let bundle_store = BundleMemoryStore::new(document_store, conn.clone());

        // A cheap, storage-agnostic embedder placeholder for the migration loop itself — real
        // per-agent indexers are resolved lazily below once the sqlite-backed VizierStorage
        // exists (agents must already be readable via AgentStorage to look up embedding config).
        let mut touched: std::collections::HashSet<(String, String)> =
            std::collections::HashSet::new();

        match &config.storage {
            StorageConfig::Sqlite => {
                let legacy_rows: Vec<String> = {
                    let c = conn.lock();
                    match c.prepare("SELECT data FROM memory") {
                        Ok(mut stmt) => stmt
                            .query_map([], |row| row.get::<_, String>(0))?
                            .filter_map(|r| r.ok())
                            .collect(),
                        Err(_) => vec![],
                    }
                };

                if legacy_rows.is_empty() {
                    let c = conn.lock();
                    Self::mark_migration_done(&c, MIGRATION_MEMORY_TO_BUNDLES)?;
                    return Ok(());
                }

                tracing::info!(
                    "migrating {} legacy memory row(s) into bundle storage",
                    legacy_rows.len()
                );

                let designated_global_agent = {
                    let c = conn.lock();
                    c.query_row(
                        "SELECT agent_id FROM agent_config ORDER BY agent_id LIMIT 1",
                        [],
                        |r| r.get::<_, String>(0),
                    )
                    .optional()?
                };

                for raw in legacy_rows {
                    let parsed: LegacyMemoryRow = match serde_json::from_str(&raw) {
                        Ok(v) => v,
                        Err(e) => {
                            tracing::error!("skipping unreadable legacy memory row: {}", e);
                            continue;
                        }
                    };

                    let target_agent = if parsed.agent_id == LEGACY_GLOBAL_AGENT_ID {
                        match &designated_global_agent {
                            Some(a) => {
                                tracing::warn!(
                                    "legacy global memory '{}' migrated to agent '{}' as private (no more global visibility)",
                                    parsed.title,
                                    a
                                );
                                a.clone()
                            }
                            None => {
                                tracing::warn!(
                                    "legacy global memory '{}' has no agent to migrate to; skipping",
                                    parsed.title
                                );
                                continue;
                            }
                        }
                    } else {
                        parsed.agent_id.clone()
                    };

                    let base_path = parsed.slug.clone();
                    let path = Self::unique_migration_path(
                        &bundle_store,
                        &target_agent,
                        "default",
                        &base_path,
                    )
                    .await;

                    if let Err(e) = bundle_store
                        .write_migrated_memory(
                            target_agent.clone(),
                            "default".to_string(),
                            path,
                            parsed.title.clone(),
                            parsed.content.clone(),
                            parsed.tags.clone(),
                            parsed.attachments.clone(),
                            parsed.timestamp,
                            parsed.timestamp,
                            parsed.read_count,
                            &VizierIndexer::build(NoopIndexer),
                        )
                        .await
                    {
                        tracing::error!(
                            "failed to migrate legacy memory '{}': {}",
                            parsed.title,
                            e
                        );
                        continue;
                    }
                    touched.insert((target_agent, "default".to_string()));
                }
            }
            StorageConfig::Filesystem => {
                let pattern = crate::utils::build_glob_path(
                    &config.workspace,
                    &["agents", "*", "memory", "*.md"],
                );
                let entries: Vec<std::path::PathBuf> = match glob::glob(&pattern) {
                    Ok(g) => g.filter_map(|e| e.ok()).collect(),
                    Err(_) => vec![],
                };

                if entries.is_empty() {
                    let c = conn.lock();
                    Self::mark_migration_done(&c, MIGRATION_MEMORY_TO_BUNDLES)?;
                    return Ok(());
                }

                tracing::info!(
                    "migrating {} legacy memory file(s) into bundle storage",
                    entries.len()
                );

                let agents_pattern =
                    crate::utils::build_glob_path(&config.workspace, &["agents", "*"]);
                let designated_global_agent = glob::glob(&agents_pattern)
                    .ok()
                    .into_iter()
                    .flatten()
                    .filter_map(|e| e.ok())
                    .filter(|p| p.is_dir())
                    .filter_map(|p| p.file_name().map(|n| n.to_string_lossy().to_string()))
                    .find(|n| n != LEGACY_GLOBAL_AGENT_ID);

                for entry in entries {
                    let (frontmatter, content) = match crate::utils::markdown::read_markdown::<
                        LegacyMemoryFrontMatter,
                    >(entry.clone())
                    {
                        Ok(v) => v,
                        Err(e) => {
                            tracing::error!(
                                "skipping unreadable legacy memory file {:?}: {}",
                                entry,
                                e
                            );
                            continue;
                        }
                    };

                    let target_agent = if frontmatter.agent_id == LEGACY_GLOBAL_AGENT_ID {
                        match &designated_global_agent {
                            Some(a) => {
                                tracing::warn!(
                                    "legacy global memory '{}' migrated to agent '{}' as private (no more global visibility)",
                                    frontmatter.title,
                                    a
                                );
                                a.clone()
                            }
                            None => {
                                tracing::warn!(
                                    "legacy global memory '{}' has no agent to migrate to; skipping",
                                    frontmatter.title
                                );
                                continue;
                            }
                        }
                    } else {
                        frontmatter.agent_id.clone()
                    };

                    let base_path = entry
                        .file_stem()
                        .map(|s| s.to_string_lossy().to_string())
                        .unwrap_or_else(|| "memory".to_string());
                    let path = Self::unique_migration_path(
                        &bundle_store,
                        &target_agent,
                        "default",
                        &base_path,
                    )
                    .await;

                    if let Err(e) = bundle_store
                        .write_migrated_memory(
                            target_agent.clone(),
                            "default".to_string(),
                            path,
                            frontmatter.title.clone(),
                            content,
                            frontmatter.tags.clone(),
                            frontmatter.attachments.clone(),
                            frontmatter.timestamp,
                            frontmatter.timestamp,
                            frontmatter.read_count,
                            &VizierIndexer::build(NoopIndexer),
                        )
                        .await
                    {
                        tracing::error!("failed to migrate legacy memory file {:?}: {}", entry, e);
                        continue;
                    }
                    touched.insert((target_agent, "default".to_string()));
                }
            }
        }

        for (agent_id, bundle) in &touched {
            if let Err(e) = bundle_store.finalize_bundle(agent_id, bundle).await {
                tracing::error!(
                    "failed to finalize migrated bundle '{}/{}': {}",
                    agent_id,
                    bundle,
                    e
                );
            }
        }

        {
            let c = conn.lock();
            Self::mark_migration_done(&c, MIGRATION_MEMORY_TO_BUNDLES)?;
        }

        Ok(())
    }

    /// Part B (contracts/migration.md, FR-025): for a deployment that was on `--storage
    /// filesystem`, copy every non-memory entity it holds into `SqliteStorage` via each side's
    /// already-existing trait methods. A no-op for anything else.
    async fn migrate_filesystem_backend_to_sqlite(
        config: &VizierConfig,
        conn: Arc<Mutex<rusqlite::Connection>>,
        cipher: crate::encryption::SecretCipher,
        secret_paths: Arc<dyn crate::encryption::IntegrationSecretPaths>,
    ) -> Result<()> {
        if !matches!(config.storage, StorageConfig::Filesystem) {
            return Ok(());
        }
        {
            let c = conn.lock();
            if Self::migration_done(&c, MIGRATION_FS_TO_SQLITE)? {
                return Ok(());
            }
        }

        let fs = FileSystemStorage::new(config.workspace.clone()).await?;
        let document_store: Arc<dyn crate::storage::document::DocumentStore> = Arc::new(
            LocalDocumentStore::new(build_path(&config.workspace, &["agents"])),
        );
        let sql = SqliteStorage::with_secrets(conn.clone(), document_store, cipher, secret_paths);

        tracing::info!("migrating filesystem-backend entities into sqlite");

        // Agents (id-preserving)
        let agents = fs.list_agents().await.unwrap_or_default();
        for (agent_id, agent_config) in &agents {
            if let Err(e) = sql.create_agent(agent_id, agent_config).await {
                tracing::error!("failed to migrate agent '{}': {}", agent_id, e);
            }
            if let Ok(Some(core)) = fs.get_agent_core(agent_id).await {
                let _ = sql
                    .set_agent_core(
                        agent_id,
                        &core,
                        &RevisionOrigin::system(RevisionTrigger::Baseline),
                    )
                    .await;
            }
        }

        // Providers: copied as they are; `migrate_provider_ids_v1` rewrites them next.
        if let Ok(rows) = fs.legacy_provider_rows() {
            let c = conn.lock();
            for (variant, data) in rows {
                if let Err(e) = c.execute(
                    "INSERT OR REPLACE INTO provider_config (variant, data) VALUES (?1, ?2)",
                    rusqlite::params![variant, data],
                ) {
                    tracing::error!("failed to migrate provider {variant}: {e}");
                }
            }
        }

        // Global config
        if let Ok(entries) = fs.list_global_configs().await {
            for entry in entries {
                if let Err(e) = sql.upsert_global_config(&entry).await {
                    tracing::error!("failed to migrate global config '{}': {}", entry.key, e);
                }
            }
        }

        // Tasks
        if let Ok(tasks) = fs.get_task_list(None, None).await {
            for task in tasks {
                if let Err(e) = sql.save_task(task.clone()).await {
                    tracing::error!("failed to migrate task '{}': {}", task.slug, e);
                }
            }
        }

        // Users, roles, API keys (best-effort: role/user ids are regenerated by SqliteStorage's
        // create_user/create_role, so cross-references to the *old* ids among migrated data are
        // not preserved — usernames, password hashes, and permissions are).
        let mut role_id_map: std::collections::HashMap<String, String> =
            std::collections::HashMap::new();
        if let Ok(roles) = fs.list_roles().await {
            for role in roles {
                match sql
                    .create_role(&role.name, role.permissions.clone(), role.is_system)
                    .await
                {
                    Ok(new_role) => {
                        role_id_map.insert(role.role_id.clone(), new_role.role_id);
                    }
                    Err(e) => tracing::error!("failed to migrate role '{}': {}", role.name, e),
                }
            }
        }

        if let Ok(users) = fs.list_users().await {
            for user in users {
                let new_role_id = role_id_map
                    .get(&user.role_id)
                    .cloned()
                    .unwrap_or(user.role_id.clone());
                match sql.create_user(&user.username, "", &new_role_id).await {
                    Ok(new_user) => {
                        tracing::warn!(
                            "migrated user '{}' with a regenerated id; existing sessions/API keys tied to the old id are not carried forward automatically",
                            user.username
                        );
                        if let Ok(Some(profile)) = fs.get_user_profile(&user.user_id).await {
                            let _ = sql.upsert_user_profile(&new_user.user_id, &profile).await;
                        }
                    }
                    Err(e) => tracing::error!("failed to migrate user '{}': {}", user.username, e),
                }
            }
        }

        // Sessions, history, dream data, session files: best-effort, per agent.
        for (agent_id, _) in &agents {
            if let Ok(sessions) = fs.get_session_list(agent_id.clone(), None).await {
                for session in sessions {
                    if let Err(e) = sql.save_session_detail(session.clone()).await {
                        tracing::error!(
                            "failed to migrate session for agent '{}': {}",
                            agent_id,
                            e
                        );
                    }
                }
            }

            if let Ok(Some(status)) = fs.get_dream_status(agent_id).await {
                let _ = sql.set_dream_status(agent_id, status).await;
            }
            if let Ok(Some(time)) = fs.get_last_dream_time(agent_id).await {
                let _ = sql.set_last_dream_time(agent_id, time).await;
            }
            if let Ok(entries) = fs.list_dream_entries(agent_id.clone(), None, None).await {
                for entry in entries {
                    if let Err(e) = sql.save_dream_entry(entry).await {
                        tracing::error!(
                            "failed to migrate dream journal entry for agent '{}': {}",
                            agent_id,
                            e
                        );
                    }
                }
            }
        }

        {
            let c = conn.lock();
            Self::mark_migration_done(&c, MIGRATION_FS_TO_SQLITE)?;
        }

        tracing::warn!(
            "filesystem-backend migration complete; 'filesystem' is no longer a valid --storage/VIZIER_STORAGE value going forward"
        );

        Ok(())
    }

    async fn migrate_users(storage: &VizierStorage) -> Result<()> {
        let system_role = match storage.get_system_role().await? {
            Some(role) => role,
            None => {
                tracing::info!("Creating system role (superadmin)");
                storage
                    .create_role(
                        "superadmin",
                        AVAILABLE_PERMISSIONS
                            .to_vec()
                            .into_iter()
                            .map(String::from)
                            .collect(),
                        true,
                    )
                    .await?
            }
        };

        // The system role holds every permission, including ones added since it was seeded
        // (the integration permissions, spec 016). No other role is rewritten.
        let missing: Vec<String> = AVAILABLE_PERMISSIONS
            .iter()
            .filter(|p| !system_role.permissions.iter().any(|have| have == *p))
            .map(|p| p.to_string())
            .collect();
        if !missing.is_empty() {
            let mut permissions = system_role.permissions.clone();
            permissions.extend(missing);
            storage
                .update_role(&system_role.role_id, &system_role.name, permissions)
                .await?;
        }

        if storage.user_exists().await? {
            let users = storage.list_users().await?;
            for user in users {
                if storage.get_role(&user.role_id).await?.is_none() {
                    tracing::info!("Migrating user '{}' to superadmin role", user.username);
                    storage
                        .update_user(&user.user_id, None, Some(&system_role.role_id))
                        .await?;
                }
            }
        }

        Ok(())
    }

    /// FR-028: `providers` in the config file is no longer read. Each leftover key gets one
    /// warning naming the env var that replaces it.
    fn warn_about_yaml_providers(config: &VizierConfig, catalogue: &crate::catalogue::Catalogue) {
        let Some(serde_yaml::Value::Mapping(map)) = &config.providers else {
            return;
        };
        let data = catalogue.snapshot();
        for key in map.keys().filter_map(|k| k.as_str()) {
            let env = crate::config::provider::ProviderId::parse(key)
                .ok()
                .and_then(|id| {
                    crate::catalogue::builtin::builtin(&id)
                        .and_then(|b| b.env.first().map(|e| e.to_string()))
                        .or_else(|| {
                            data.providers.get(&id).and_then(|p| {
                                p.env
                                    .iter()
                                    .find(|e| e.contains("KEY") || e.contains("TOKEN"))
                                    .cloned()
                            })
                        })
                });
            match env {
                Some(var) => tracing::warn!(
                    "providers.{key} is ignored; set {var} or add it in Settings › Providers"
                ),
                None => {
                    tracing::warn!("providers.{key} is ignored; add it in Settings › Providers")
                }
            }
        }
    }

    pub async fn run(&self) -> Result<()> {
        self.transport.run().await?;

        Ok(())
    }

    /// Two one-time fixups per agent: (1) a CORE that still lives in the legacy
    /// `agent_config.core` JSON field is moved into the `agent_core` table (recorded as the
    /// document's `baseline` revision) so history has a single home to hook into; (2) an agent
    /// with no CORE anywhere gets the default template.
    async fn migrate_agent_cores(storage: &VizierStorage) -> Result<()> {
        let agents = storage.list_agents().await?;
        if agents.is_empty() {
            return Ok(());
        }

        let baseline = RevisionOrigin::system(RevisionTrigger::Baseline);
        let mut seeded = 0;
        let mut moved = 0;
        for (agent_id, mut config) in agents {
            if let Some(legacy_core) = config.core.take() {
                // `set_agent_core` writes the table; clearing the config field afterwards
                // makes `agent_core` the only source of truth.
                match storage
                    .set_agent_core(&agent_id, &legacy_core, &baseline)
                    .await
                {
                    Ok(()) => {
                        if let Err(e) = storage.update_agent(&agent_id, &config).await {
                            tracing::warn!(
                                "moved CORE for agent '{}' but failed to clear legacy field: {}",
                                agent_id,
                                e
                            );
                        }
                        moved += 1;
                    }
                    Err(e) => {
                        tracing::warn!("failed to move legacy CORE for agent '{}': {}", agent_id, e)
                    }
                }
                continue;
            }

            match storage.get_agent_core(&agent_id).await {
                Ok(Some(_)) => continue,
                Ok(None) => {}
                Err(e) => {
                    tracing::warn!("skipping CORE backfill for agent '{}': {}", agent_id, e);
                    continue;
                }
            }

            if let Err(e) = storage
                .set_agent_core(
                    &agent_id,
                    CORE_MD,
                    &RevisionOrigin::system(RevisionTrigger::Baseline),
                )
                .await
            {
                tracing::warn!(
                    "failed to backfill default CORE for agent '{}': {}",
                    agent_id,
                    e
                );
            } else {
                seeded += 1;
            }
        }

        if moved > 0 {
            tracing::info!("moved legacy CORE into agent_core for {} agent(s)", moved);
        }
        if seeded > 0 {
            tracing::info!("backfilled default CORE for {} agent(s)", seeded);
        }

        Ok(())
    }
    /// Every stored task's `user: "x"` becomes `requester: {"user": "x"}`
    /// (`specs/011-task-completion-reports/data-model.md`).
    ///
    /// This works on the raw `task.data` blob rather than through `TaskStorage`, because a
    /// row still carrying `user` no longer deserializes into `Task` at all — and
    /// `get_task_list` drops a row it cannot parse silently, so going through it would
    /// migrate nothing while reporting success.
    ///
    /// **Imprecise by construction, and plainly so**: nothing recorded before this change
    /// distinguishes a task the agent set up on its own initiative, so every pre-existing
    /// task becomes person-attributed. Guessing an agent from the string would be worse than
    /// being approximate.
    async fn migrate_task_requester(conn: Arc<Mutex<rusqlite::Connection>>) -> Result<()> {
        const MARKER: &str = "migration__task_requester__v1";

        let conn = conn.lock();
        if Self::migration_done(&conn, MARKER)? {
            return Ok(());
        }

        let rows: Vec<(String, String)> = {
            let mut stmt = conn.prepare("SELECT id, data FROM task")?;
            let mapped = stmt.query_map([], |row| Ok((row.get(0)?, row.get(1)?)))?;
            mapped.filter_map(|r| r.ok()).collect()
        };

        let mut migrated = 0;
        for (id, data) in rows {
            let Ok(mut value) = serde_json::from_str::<serde_json::Value>(&data) else {
                tracing::warn!(
                    "skipping requester migration for task '{}': unparseable data",
                    id
                );
                continue;
            };
            let Some(object) = value.as_object_mut() else {
                continue;
            };
            if object.contains_key("requester") {
                continue;
            }
            // A task with neither field is already broken; `"user"` is the literal the web
            // form used to default to, and it migrates as faithfully as anything else would.
            let user = match object.remove("user") {
                Some(serde_json::Value::String(user)) => user,
                _ => continue,
            };
            object.insert("requester".to_string(), serde_json::json!({ "user": user }));

            match serde_json::to_string(&value) {
                Ok(data) => {
                    conn.execute(
                        "UPDATE task SET data = ?1 WHERE id = ?2",
                        rusqlite::params![data, id],
                    )?;
                    migrated += 1;
                }
                Err(e) => tracing::warn!("failed to re-encode task '{}': {}", id, e),
            }
        }

        Self::mark_migration_done(&conn, MARKER)?;

        if migrated > 0 {
            tracing::info!("migrated {} task(s) from user to requester", migrated);
        }

        Ok(())
    }

    /// A process that stopped mid-run left a `task_run` row claiming to be in flight. Such a
    /// row would both display as perpetually running and — since the row *is* the overlap
    /// lock — permanently block the task from firing again.
    ///
    /// Running at startup is what makes this correct rather than a guess: nothing can
    /// legitimately be running before the scheduler starts.
    ///
    /// Unlike the migrations above this is not one-time, so it carries no marker.
    async fn interrupt_orphaned_task_runs(
        storage: &VizierStorage,
    ) -> Result<Vec<crate::storage::task_run::InterruptedRun>> {
        // A run whose session has a pending turn is resumed with it and closed by
        // `VizierScheduler::recover`; only the rest died with the process.
        let pending = storage
            .sessions_with_pending_turns()
            .await
            .unwrap_or_default();
        let keep: Vec<i64> = storage
            .open_task_runs()
            .await
            .unwrap_or_default()
            .iter()
            .filter(|run| pending.contains(&crate::storage::task_run::task_run_session(run)))
            .map(|run| run.id)
            .collect();
        match storage.interrupt_open_task_runs_except(&keep).await {
            Ok(swept) => {
                if !swept.is_empty() {
                    tracing::info!(
                        "marked {} task run(s) left open by a previous process as interrupted",
                        swept.len()
                    );
                }
                Ok(swept)
            }
            // A failed sweep must not stop the process from booting; the cost is a task
            // that looks busy until the next successful startup.
            Err(e) => {
                tracing::warn!("failed to sweep open task runs: {}", e);
                Ok(vec![])
            }
        }
    }

    /// Record the task runs and background jobs the startup sweeps just closed as
    /// notifications. A task's title is looked up; a deleted or unreadable task falls back to
    /// its slug.
    async fn record_interrupted(
        notifications: &crate::notifications::Notifications,
        storage: &VizierStorage,
        runs: Vec<crate::storage::task_run::InterruptedRun>,
        jobs: Vec<crate::storage::background_job::InterruptedJob>,
    ) {
        let now = chrono::Utc::now();
        for run in runs {
            let title = match storage
                .get_task(run.agent_id.clone(), run.task_slug.clone())
                .await
            {
                Ok(Some(task)) => task.title,
                _ => run.task_slug.clone(),
            };
            notifications
                .task_run_ended(
                    &run.agent_id,
                    &run.task_slug,
                    &title,
                    run.id,
                    crate::schema::TaskRunState::Interrupted,
                    now,
                )
                .await;
        }
        for job in jobs {
            notifications
                .job_ended(
                    &job.id,
                    &job.agent_id,
                    job.kind,
                    crate::schema::JobState::Interrupted,
                    None,
                    now,
                )
                .await;
        }
    }

    /// Background jobs live only as long as the process that runs them: a job still
    /// `running` or `reporting` at startup died with the previous process, and will never
    /// report. Marking it `interrupted` is what lets a reloaded WebUI show it as lost instead
    /// of perpetually running (research D9).
    async fn interrupt_orphaned_background_jobs(
        storage: &VizierStorage,
    ) -> Vec<crate::storage::background_job::InterruptedJob> {
        // A job a pending turn still backs is kept open: a running one while one of its
        // pieces has a pending turn, a reporting one while its report does. `recover` picks
        // them up once the agents have re-sent their turns (`specs/023-persist-turn-activity`).
        let keep = Self::background_jobs_backed_by_turns(storage).await;
        match storage.interrupt_open_background_jobs_except(&keep).await {
            Ok(swept) => {
                if !swept.is_empty() {
                    tracing::info!(
                        "marked {} background job(s) left open by a previous process as interrupted",
                        swept.len()
                    );
                }
                swept
            }
            Err(e) => {
                tracing::warn!("failed to sweep open background jobs: {}", e);
                vec![]
            }
        }
    }

    async fn background_jobs_backed_by_turns(storage: &VizierStorage) -> Vec<String> {
        use crate::schema::{JobState, PieceState, VizierRequestContent};
        let Ok(jobs) = storage.open_background_jobs().await else {
            return vec![];
        };
        let pending_sessions = storage
            .sessions_with_pending_turns()
            .await
            .unwrap_or_default();
        let mut keep = vec![];
        for job in jobs {
            let backed = match job.state {
                JobState::Running => job
                    .pieces
                    .iter()
                    .any(|p| p.state == PieceState::Running && pending_sessions.contains(&p.session)),
                JobState::Reporting => storage
                    .pending_turns(&job.origin.0)
                    .await
                    .unwrap_or_default()
                    .iter()
                    .any(|turn| {
                        matches!(&turn.request.content, VizierRequestContent::BackgroundReport(r) if r.job_id == job.id)
                    }),
                _ => false,
            };
            if backed {
                keep.push(job.id);
            }
        }
        keep
    }

    /// A client tool call is awaited by the turn that made it, so one still `pending` at
    /// startup died with the previous process (specs/019-client-tools R16).
    async fn interrupt_orphaned_client_calls(storage: &VizierStorage) {
        match storage.interrupt_pending_client_calls().await {
            Ok(0) => {}
            Ok(swept) => tracing::info!(
                "marked {} client tool call(s) left pending by a previous process as interrupted",
                swept
            ),
            Err(e) => tracing::warn!("failed to sweep pending client tool calls: {}", e),
        }
    }

    /// Scratch files last only while their conversation has a turn running or queued, and
    /// nothing can be running before agents start, so every row left is a leftover
    /// (`specs/020-scratchpad-tools` research R5).
    async fn clear_orphaned_scratch(storage: &VizierStorage) {
        // A session with a turn waiting to resume keeps its scratch files and its thinking
        // mark; everything else is a leftover of the previous process.
        let keep = storage
            .sessions_with_pending_turns()
            .await
            .unwrap_or_default();
        match storage.reset_thinking_except(&keep).await {
            Ok(0) => {}
            Ok(reset) => tracing::info!(
                "cleared the thinking mark of {} session(s) left by a previous process",
                reset
            ),
            Err(e) => tracing::warn!("failed to reset thinking marks: {}", e),
        }
        match storage.clear_scratch_except(&keep).await {
            Ok(0) => {}
            Ok(cleared) => tracing::info!(
                "cleared {} scratch file(s) left by a previous process",
                cleared
            ),
            Err(e) => tracing::warn!("failed to clear leftover scratch files: {}", e),
        }
    }
}

/// The usage event a saved reply carried, for [`VizierDependencies::migrate_reply_usage`]: its
/// totals as one row, `legacy`, with no turn, round or served model (research R6).
fn reply_usage_event(history: &SessionHistory) -> Option<crate::schema::usage::UsageEvent> {
    use crate::schema::{VizierResponseContent, metrics::TokenCounts};

    let SessionHistoryContent::Response(response) = &history.content else {
        return None;
    };
    let stats = match &response.content {
        VizierResponseContent::Message { stats, .. } => stats.as_ref(),
        VizierResponseContent::AudioReply(_, _, stats) => stats.as_ref(),
        _ => None,
    }?;
    let session = &history.vizier_session;
    Some(crate::schema::usage::UsageEvent {
        agent_id: session.0.clone(),
        at: history.timestamp,
        activity: session.1.usage_activity(),
        channel: Some(session.1.to_slug()),
        topic: session.2.clone(),
        place: session.usage_place(),
        turn_id: None,
        round: None,
        provider: stats
            .provider
            .as_ref()
            .map(|p| p.to_string())
            .unwrap_or_default(),
        model: stats.model.clone().unwrap_or_default(),
        served_model: None,
        tokens: TokenCounts::from(stats),
        duration_ms: stats.duration.as_millis() as u64,
        legacy: true,
    })
}

#[cfg(test)]
mod usage_migration_tests {
    use super::*;
    use crate::{
        schema::{
            VizierChannelId, VizierResponse, VizierResponseContent, VizierResponseStats,
            VizierSession, usage::UsageActivity,
        },
        storage::{
            document::LocalDocumentStore,
            history::HistoryStorage,
            sqlite::SqliteStorage,
            usage::{UsageGrouping, UsageQuery, UsageStorage},
        },
    };

    fn setup() -> (
        VizierStorage,
        Arc<Mutex<rusqlite::Connection>>,
        tempfile::TempDir,
    ) {
        let dir = tempfile::tempdir().unwrap();
        let conn = rusqlite::Connection::open_in_memory().unwrap();
        crate::storage::sqlite::init_artifact_schema(&conn).unwrap();
        crate::storage::sqlite::init_history_schema(&conn).unwrap();
        crate::storage::sqlite::init_reaction_schema(&conn).unwrap();
        crate::storage::sqlite::init_usage_schema(&conn).unwrap();
        conn.execute_batch("CREATE TABLE state (key TEXT PRIMARY KEY, value TEXT NOT NULL);")
            .unwrap();
        let conn = Arc::new(Mutex::new(conn));
        let storage = VizierStorage::new(SqliteStorage::new(
            conn.clone(),
            Arc::new(LocalDocumentStore::new(dir.path().to_path_buf())),
        ));
        (storage, conn, dir)
    }

    fn reply(input: u64, output: u64, model: Option<&str>) -> SessionHistoryContent {
        SessionHistoryContent::Response(VizierResponse {
            timestamp: chrono::Utc::now(),
            content: VizierResponseContent::Message {
                content: "hi".into(),
                stats: Some(VizierResponseStats {
                    total_input_tokens: input,
                    total_cached_input_tokens: input / 2,
                    total_output_tokens: output,
                    total_tokens: input + output,
                    provider: model.map(|_| "anthropic".parse().unwrap()),
                    model: model.map(str::to_string),
                    ..Default::default()
                }),
            },
            ..Default::default()
        })
    }

    #[tokio::test]
    async fn carried_over_replies_keep_their_totals_and_the_carry_over_runs_once() {
        let (storage, conn, _d) = setup();
        let discord = VizierSession("a".into(), VizierChannelId::DiscordChanel(1), None);
        let task = VizierSession(
            "a".into(),
            VizierChannelId::Task("brief".into(), chrono::Utc::now()),
            None,
        );
        storage
            .save_session_history(discord.clone(), reply(1000, 100, Some("sonnet")))
            .await
            .unwrap();
        storage
            .save_session_history(discord, reply(500, 50, None))
            .await
            .unwrap();
        storage
            .save_session_history(task.clone(), reply(200, 20, Some("sonnet")))
            .await
            .unwrap();
        // Not a reply with stats: carries nothing.
        storage
            .save_session_history(task, SessionHistoryContent::Command("checkpoint".into()))
            .await
            .unwrap();

        VizierDependencies::migrate_reply_usage(&storage, conn.clone())
            .await
            .unwrap();
        VizierDependencies::migrate_reply_usage(&storage, conn.clone())
            .await
            .unwrap();

        let q = UsageQuery {
            agent_id: "a".into(),
            start: chrono::DateTime::<chrono::Utc>::MIN_UTC,
            end: chrono::Utc::now() + chrono::Duration::days(1),
            activity: None,
            conversation: None,
        };
        let groups = storage
            .usage_groups(&q, UsageGrouping::Activity)
            .await
            .unwrap();
        let sum = |a: UsageActivity| -> (u64, u64, u64) {
            groups
                .iter()
                .filter(|g| g.activity == Some(a))
                .fold((0, 0, 0), |(c, i, o), g| {
                    (c + g.calls, i + g.tokens.input, o + g.tokens.output)
                })
        };
        assert_eq!(
            sum(UsageActivity::Chat),
            (2, 1500, 150),
            "no double count on the second run"
        );
        assert_eq!(sum(UsageActivity::Scheduled), (1, 200, 20));
        assert!(
            groups
                .iter()
                .any(|g| g.provider.is_empty() && g.model.is_empty())
        );
        assert!(
            storage
                .get_state(VizierDependencies::USAGE_ACCOUNTING_SINCE.into())
                .await
                .unwrap()
                .is_some()
        );
    }
}

#[cfg(test)]
mod encryption_startup_tests {
    use super::*;
    use crate::storage::secret::SecretStorage;

    /// T022: `VizierDependencies::new` converts plaintext before constructing the main storage,
    /// so no consumer can run mid-conversion; `migrate_agent_cores` still sees `core`; and a
    /// second start is a no-op.
    #[tokio::test]
    async fn startup_migrates_before_consumers_and_is_idempotent() {
        let dir = tempfile::tempdir().unwrap();
        let workspace = dir.path().to_string_lossy().to_string();

        // A pre-encryption workspace: schema plus one plaintext secret.
        {
            let conn = SqliteStorage::open_connection(&workspace).unwrap();
            conn.execute_batch(
                "INSERT INTO agent_secret (agent_id, name, value, created_at, updated_at, updated_by)
                 VALUES ('dp-a', 'TOKEN', 'startup-secret-abc123', 'now', 'now', '\"admin\"');",
            )
            .unwrap();
        }

        let config = VizierConfig {
            workspace: workspace.clone(),
            ..Default::default()
        };

        let deps = VizierDependencies::new(config.clone()).await.unwrap();
        assert_eq!(
            deps.storage
                .get_agent_secret("dp-a", "TOKEN")
                .await
                .unwrap()
                .as_deref(),
            Some("startup-secret-abc123")
        );
        let raw: String = {
            let conn = deps.sqlite_conn.lock();
            conn.query_row(
                "SELECT value FROM agent_secret WHERE agent_id = 'dp-a' AND name = 'TOKEN'",
                [],
                |row| row.get(0),
            )
            .unwrap()
        };
        assert!(raw.starts_with("enc:v1:"), "{raw}");
        let marker: Option<String> = {
            let conn = deps.sqlite_conn.lock();
            conn.query_row(
                "SELECT value FROM state WHERE key = 'migration.secrets_encrypted_v1'",
                [],
                |row| row.get(0),
            )
            .ok()
        };
        assert_eq!(marker.as_deref(), Some("true"));
        drop(deps);

        // Second start: same value, no re-encryption.
        let deps = VizierDependencies::new(config).await.unwrap();
        assert_eq!(
            deps.storage
                .get_agent_secret("dp-a", "TOKEN")
                .await
                .unwrap()
                .as_deref(),
            Some("startup-secret-abc123")
        );
        let raw_again: String = {
            let conn = deps.sqlite_conn.lock();
            conn.query_row(
                "SELECT value FROM agent_secret WHERE agent_id = 'dp-a' AND name = 'TOKEN'",
                [],
                |row| row.get(0),
            )
            .unwrap()
        };
        assert_eq!(raw, raw_again);
    }
}
