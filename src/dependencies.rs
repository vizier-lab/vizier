use std::sync::Arc;

use anyhow::Result;
use parking_lot::Mutex;
use rusqlite::OptionalExtension;

use crate::{
    config::{VizierConfig, provider::ProviderVariant, storage::StorageConfig},
    constant::CORE_MD,
    file_manager::FileManager,
    indexer::{VizierIndexer, noop::NoopIndexer},
    schema::{
        AgentToolsConfig, ProviderEntry, ProviderEntryConfig, RevisionOrigin, RevisionTrigger,
        VizierAttachment,
    },
    storage::{
        VizierStorage,
        agent::AgentStorage,
        document::LocalDocumentStore,
        dream::DreamStorage,
        dream_journal::DreamJournalStorage,
        fs::FileSystemStorage,
        global_config::GlobalConfigStorage,
        memory_bundle::BundleMemoryStore,
        provider::ProviderStorage,
        session::SessionStorage,
        session_file::SessionFileStorage,
        sqlite::SqliteStorage,
        state::StateStorage,
        task::TaskStorage,
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
        Self::migrate_filesystem_backend_to_sqlite(&config, conn.clone()).await?;

        let storage = VizierStorage::new(SqliteStorage::new(conn.clone(), document_store));

        Self::migrate_users(&storage).await?;
        Self::migrate_providers(&config, &storage).await?;
        Self::migrate_agent_tools(&storage).await?;
        Self::migrate_agent_cores(&storage).await?;

        let transport = VizierTransport::new();
        let file_manager = FileManager::new(config.workspace.clone());

        let fm = file_manager.clone();
        let file_transport = transport.clone();
        tokio::spawn(async move {
            fm.run(file_transport).await;
        });

        Ok(Self {
            config: Arc::new(config.clone()),
            storage: Arc::new(storage),
            sqlite_conn: conn,
            transport,
            file_manager,
        })
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
                .get_memory_detail(agent_id.to_string(), Some(bundle.to_string()), candidate.clone())
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
        let mut touched: std::collections::HashSet<(String, String)> = std::collections::HashSet::new();

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
                    let path = Self::unique_migration_path(&bundle_store, &target_agent, "default", &base_path).await;

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
                        tracing::error!("failed to migrate legacy memory '{}': {}", parsed.title, e);
                        continue;
                    }
                    touched.insert((target_agent, "default".to_string()));
                }
            }
            StorageConfig::Filesystem => {
                let pattern =
                    crate::utils::build_glob_path(&config.workspace, &["agents", "*", "memory", "*.md"]);
                let entries: Vec<std::path::PathBuf> = match glob::glob(&pattern) {
                    Ok(g) => g.filter_map(|e| e.ok()).collect(),
                    Err(_) => vec![],
                };

                if entries.is_empty() {
                    let c = conn.lock();
                    Self::mark_migration_done(&c, MIGRATION_MEMORY_TO_BUNDLES)?;
                    return Ok(());
                }

                tracing::info!("migrating {} legacy memory file(s) into bundle storage", entries.len());

                let agents_pattern = crate::utils::build_glob_path(&config.workspace, &["agents", "*"]);
                let designated_global_agent = glob::glob(&agents_pattern)
                    .ok()
                    .into_iter()
                    .flatten()
                    .filter_map(|e| e.ok())
                    .filter(|p| p.is_dir())
                    .filter_map(|p| p.file_name().map(|n| n.to_string_lossy().to_string()))
                    .find(|n| n != LEGACY_GLOBAL_AGENT_ID);

                for entry in entries {
                    let (frontmatter, content) =
                        match crate::utils::markdown::read_markdown::<LegacyMemoryFrontMatter>(entry.clone()) {
                            Ok(v) => v,
                            Err(e) => {
                                tracing::error!("skipping unreadable legacy memory file {:?}: {}", entry, e);
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
                    let path = Self::unique_migration_path(&bundle_store, &target_agent, "default", &base_path).await;

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
                        tracing::error!(
                            "failed to migrate legacy memory file {:?}: {}",
                            entry,
                            e
                        );
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
        let sql = SqliteStorage::new(conn.clone(), document_store);

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

        // Providers
        if let Ok(providers) = fs.list_providers().await {
            for p in providers {
                if let Err(e) = sql.upsert_provider(&p).await {
                    tracing::error!("failed to migrate provider {:?}: {}", p.variant, e);
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
        let mut role_id_map: std::collections::HashMap<String, String> = std::collections::HashMap::new();
        if let Ok(roles) = fs.list_roles().await {
            for role in roles {
                match sql.create_role(&role.name, role.permissions.clone(), role.is_system).await {
                    Ok(new_role) => {
                        role_id_map.insert(role.role_id.clone(), new_role.role_id);
                    }
                    Err(e) => tracing::error!("failed to migrate role '{}': {}", role.name, e),
                }
            }
        }

        if let Ok(users) = fs.list_users().await {
            for user in users {
                let new_role_id = role_id_map.get(&user.role_id).cloned().unwrap_or(user.role_id.clone());
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
                        AVAILABLE_PERMISSIONS.to_vec().into_iter().map(String::from).collect(),
                        true,
                    )
                    .await?
            }
        };

        if storage.user_exists().await? {
            let users = storage.list_users().await?;
            for user in users {
                if storage.get_role(&user.role_id).await?.is_none() {
                    tracing::info!(
                        "Migrating user '{}' to superadmin role",
                        user.username
                    );
                    storage
                        .update_user(&user.user_id, None, Some(&system_role.role_id))
                        .await?;
                }
            }
        }

        Ok(())
    }

    async fn migrate_providers(config: &VizierConfig, storage: &VizierStorage) -> Result<()> {
        if !storage.list_providers().await?.is_empty() {
            return Ok(());
        }

        tracing::info!("migrating providers from YAML config to storage");

        let providers = &config.providers;
        let entries: Vec<ProviderEntry> = [
            providers.ollama.as_ref().map(|c| ProviderEntry {
                variant: ProviderVariant::ollama,
                config: ProviderEntryConfig::Ollama {
                    base_url: c.base_url.clone(),
                },
            }),
            providers.openai.as_ref().map(|c| ProviderEntry {
                variant: ProviderVariant::openai,
                config: ProviderEntryConfig::Openai {
                    api_key: c.api_key.clone(),
                },
            }),
            providers.anthropic.as_ref().map(|c| ProviderEntry {
                variant: ProviderVariant::anthropic,
                config: ProviderEntryConfig::Anthropic {
                    api_key: c.api_key.clone(),
                },
            }),
            providers.deepseek.as_ref().map(|c| ProviderEntry {
                variant: ProviderVariant::deepseek,
                config: ProviderEntryConfig::Deepseek {
                    api_key: c.api_key.clone(),
                },
            }),
            providers.openrouter.as_ref().map(|c| ProviderEntry {
                variant: ProviderVariant::openrouter,
                config: ProviderEntryConfig::Openrouter {
                    api_key: c.api_key.clone(),
                },
            }),
            providers.gemini.as_ref().map(|c| ProviderEntry {
                variant: ProviderVariant::gemini,
                config: ProviderEntryConfig::Gemini {
                    api_key: c.api_key.clone(),
                },
            }),
            providers.mimo.as_ref().map(|c| ProviderEntry {
                variant: ProviderVariant::mimo,
                config: ProviderEntryConfig::Mimo {
                    api_key: c.api_key.clone(),
                },
            }),
            providers.llama_cpp.as_ref().map(|c| ProviderEntry {
                variant: ProviderVariant::llama_cpp,
                config: ProviderEntryConfig::LlamaCpp {
                    base_url: c.base_url.clone(),
                },
            }),
            providers.elevenlabs.as_ref().map(|c| ProviderEntry {
                variant: ProviderVariant::elevenlabs,
                config: ProviderEntryConfig::Elevenlabs {
                    api_key: c.api_key.clone(),
                },
            }),
            providers.custom.as_ref().map(|c| ProviderEntry {
                variant: ProviderVariant::custom,
                config: ProviderEntryConfig::Custom {
                    api_key: c.api_key.clone(),
                    base_url: c.base_url.clone(),
                },
            }),
        ]
        .into_iter()
        .flatten()
        .collect();

        for entry in entries {
            if let Err(e) = storage.upsert_provider(&entry).await {
                tracing::warn!("failed to migrate provider {:?}: {}", entry.variant, e);
            }
        }

        Ok(())
    }

    pub async fn run(&self) -> Result<()> {
        self.transport.run().await?;

        Ok(())
    }

    async fn migrate_agent_tools(storage: &VizierStorage) -> Result<()> {
        use std::collections::HashMap;

        let agents = storage.list_agents().await?;
        if agents.is_empty() {
            return Ok(());
        }

        let global_mcp = match storage.get_global_config("mcp_servers").await {
            Ok(Some(entry)) => {
                if let crate::schema::GlobalConfigValue::McpServers(servers) = entry.value {
                    Some(servers)
                } else {
                    None
                }
            }
            _ => None,
        };

        let global_shell = match storage.get_global_config("shell").await {
            Ok(Some(entry)) => {
                if let crate::schema::GlobalConfigValue::Shell(shell) = entry.value {
                    Some(shell)
                } else {
                    None
                }
            }
            _ => None,
        };

        let mut migrated = 0;
        for (agent_id, mut agent_config) in agents {
            let mut changed = false;

            if agent_config.tools.mcp_servers.is_empty() {
                if let Some(ref global_servers) = global_mcp {
                    if !global_servers.is_empty() {
                        agent_config.tools.mcp_servers = global_servers.clone();
                        changed = true;
                    }
                }
            }

            if changed {
                if let Err(e) = storage.update_agent(&agent_id, &agent_config).await {
                    tracing::warn!(
                        "failed to migrate tools for agent '{}': {}",
                        agent_id,
                        e
                    );
                } else {
                    migrated += 1;
                }
            }
        }

        if migrated > 0 {
            tracing::info!("migrated tools config for {} agents", migrated);
        }

        let _ = storage.delete_global_config("mcp_servers").await;
        let _ = storage.delete_global_config("shell").await;

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
                match storage.set_agent_core(&agent_id, &legacy_core, &baseline).await {
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
                    Err(e) => tracing::warn!(
                        "failed to move legacy CORE for agent '{}': {}",
                        agent_id,
                        e
                    ),
                }
                continue;
            }

            match storage.get_agent_core(&agent_id).await {
                Ok(Some(_)) => continue,
                Ok(None) => {}
                Err(e) => {
                    tracing::warn!(
                        "skipping CORE backfill for agent '{}': {}",
                        agent_id,
                        e
                    );
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
}
