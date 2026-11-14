use std::{
    fs,
    path::{Path, PathBuf},
    str::FromStr,
};

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};

pub mod embedding;
pub mod provider;
pub mod shell;
pub mod storage;
pub mod tools;

use crate::{
    config::{
        storage::{ObjectsConfig, StorageConfig},
    },
    constant,
};

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct ChannelsConfig {
    pub http: Option<HTTPChannelConfig>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct HTTPChannelConfig {
    pub port: u32,
    pub jwt_secret: String,
    #[serde(default = "default_jwt_expiry")]
    pub jwt_expiry_hours: u64,
    #[serde(default = "default_ws_idle_timeout_secs")]
    pub ws_idle_timeout_secs: u64,
    /// Allowed browser origins; `None` allows any (the server's behaviour).
    #[serde(default)]
    pub cors_origins: Option<Vec<String>>,
    /// Address to bind; `None` binds every interface (the server's behaviour).
    #[serde(default)]
    pub host: Option<String>,
}

fn default_jwt_expiry() -> u64 {
    720 // 30 days
}

fn default_ws_idle_timeout_secs() -> u64 {
    300 // 5 minutes
}

/// Passage sizing bounds for the memory chunker (`src/storage/chunk.rs`). Byte-denominated
/// rather than token-denominated so the chunker stays synchronous and provider-agnostic — no
/// tokenizer dependency. `target_size` is what the packer aims for, `min_size` is a constraint
/// that overrides boundary preference, and `max_size` is a hard ceiling except where an
/// indivisible block is permitted to overflow it (FR-002, FR-003, FR-004, FR-041).
#[derive(Debug, Serialize, Deserialize, Clone, PartialEq)]
pub struct ChunkLimits {
    #[serde(default = "default_chunk_target_size")]
    pub target_size: usize,
    #[serde(default = "default_chunk_min_size")]
    pub min_size: usize,
    #[serde(default = "default_chunk_max_size")]
    pub max_size: usize,
}

fn default_chunk_target_size() -> usize {
    1200
}

fn default_chunk_min_size() -> usize {
    400
}

fn default_chunk_max_size() -> usize {
    2400
}

impl Default for ChunkLimits {
    fn default() -> Self {
        Self {
            target_size: default_chunk_target_size(),
            min_size: default_chunk_min_size(),
            max_size: default_chunk_max_size(),
        }
    }
}

/// Size bounds for one memory document's **content** (specs/043-unify-core-and-memory,
/// FR-016/FR-019/FR-022/FR-023). Both values are denominated in bytes of the markdown body
/// only — never the serialised file, whose frontmatter carries titles, tags, links and
/// timestamps the system rewrites (research Decision 2).
///
/// `content_cap` refuses a write; `warn_at` only warns, so a document may legitimately sit
/// between them. Sits beside [`ChunkLimits`] so it rides the same per-agent config plumbing a
/// memory write already receives.
#[derive(Debug, Serialize, Deserialize, Clone, PartialEq)]
pub struct MemoryLimits {
    /// Hard ceiling: content larger than this is refused, never truncated (FR-017).
    #[serde(default = "default_content_cap")]
    pub content_cap: usize,
    /// Soft limit: content larger than this still writes, and the result carries a warning
    /// (FR-019). Values above `content_cap` are treated as `content_cap` (no warning band).
    #[serde(default = "default_warn_at")]
    pub warn_at: usize,
}

fn default_content_cap() -> usize {
    8192
}

fn default_warn_at() -> usize {
    4096
}

impl Default for MemoryLimits {
    fn default() -> Self {
        Self {
            content_cap: default_content_cap(),
            warn_at: default_warn_at(),
        }
    }
}

impl MemoryLimits {
    /// The tighter pair the identity document is held to, because it is paid for on every
    /// turn (FR-023). Selected by address, not by a second config field (data-model rule 8).
    pub fn identity() -> Self {
        Self {
            content_cap: 4096,
            warn_at: 1536,
        }
    }

    /// The soft limit actually applied. A pair where `warn_at > content_cap` is a config
    /// mistake, and it degrades to "no warning band" rather than erroring — a bad value must
    /// never break writes (data-model rule 7).
    pub fn warn_threshold(&self) -> usize {
        self.warn_at.min(self.content_cap)
    }

    /// True when `len` exceeds the ceiling.
    pub fn exceeds_cap(&self, len: usize) -> bool {
        len > self.content_cap
    }

    /// True when `len` is over the soft limit but still at or under the ceiling, so the write
    /// succeeds and carries a warning.
    pub fn warns_at(&self, len: usize) -> bool {
        len > self.warn_threshold() && !self.exceeds_cap(len)
    }
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct VizierConfig {
    #[serde(skip)]
    pub workspace: String,
    /// Removed in spec 015 (FR-028): providers are configured in Settings › Providers or
    /// through their environment variables. A leftover section is read only to warn about it.
    #[serde(default, skip_serializing)]
    pub providers: Option<serde_yaml::Value>,
    pub storage: StorageConfig,
    /// Where file bytes live (spec 045); local workspace files unless configured.
    #[serde(default)]
    pub objects: ObjectsConfig,
    pub channels: ChannelsConfig,
    #[serde(default = "default_worker_threads")]
    pub worker_threads: usize,
    /// The model catalogue's lazy refresh (spec 015, data-model §10).
    #[serde(default)]
    pub catalogue: CatalogueConfig,
    /// Language servers for project chats (spec 054).
    #[serde(default)]
    pub lsp: LspConfig,
    /// Subagents as temporary workers (spec 059).
    #[serde(default)]
    pub subagents: SubagentsConfig,
}

/// Subagents (`specs/059-subagent-workers`).
#[derive(Debug, Serialize, Deserialize, Clone, PartialEq, Eq)]
pub struct SubagentsConfig {
    /// How deep workers may hire workers. `1`: only the main agent spawns (a worker is at depth
    /// 1 and is offered no management tool). Raise it to let workers hire their own.
    #[serde(default = "default_subagents_max_depth")]
    pub max_depth: u8,
}

fn default_subagents_max_depth() -> u8 {
    1
}

impl Default for SubagentsConfig {
    fn default() -> Self {
        Self {
            max_depth: default_subagents_max_depth(),
        }
    }
}

/// Language servers in project chats (`specs/054-project-lsp`).
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct LspConfig {
    /// Most servers running at once, instance-wide. The least recently used one stops first.
    #[serde(default = "default_lsp_max_servers")]
    pub max_servers: usize,
}

fn default_lsp_max_servers() -> usize {
    6
}

impl Default for LspConfig {
    fn default() -> Self {
        Self {
            max_servers: default_lsp_max_servers(),
        }
    }
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct CatalogueConfig {
    /// Refresh from `url` when someone uses the catalogue and the copy is older than `ttl`.
    /// Off for air-gapped installs; the bundled snapshot is always complete.
    #[serde(default = "default_catalogue_refresh")]
    pub refresh: bool,
    #[serde(default = "default_catalogue_ttl")]
    pub ttl: duration_string::DurationString,
    /// Logos come from the same origin's `/logos/`.
    #[serde(default = "default_catalogue_url")]
    pub url: String,
}

fn default_catalogue_refresh() -> bool {
    true
}

fn default_catalogue_ttl() -> duration_string::DurationString {
    std::time::Duration::from_secs(24 * 3600).into()
}

fn default_catalogue_url() -> String {
    "https://models.dev/api.json".into()
}

impl Default for CatalogueConfig {
    fn default() -> Self {
        Self {
            refresh: default_catalogue_refresh(),
            ttl: default_catalogue_ttl(),
            url: default_catalogue_url(),
        }
    }
}

fn default_worker_threads() -> usize {
    4
}

#[derive(Debug, Serialize, Deserialize, Clone)]
struct AllConfig {
    vizier: VizierConfig,
}

#[derive(Debug, Clone, Default)]
pub struct RunOverrides {
    pub workspace: Option<PathBuf>,
    pub port: Option<u32>,
    pub storage: Option<StorageConfig>,
    pub objects: Option<ObjectsConfig>,
    pub workers: Option<usize>,
    pub ws_idle_timeout: Option<u64>,
    pub catalogue_refresh: Option<bool>,
    pub catalogue_ttl: Option<duration_string::DurationString>,
}

impl VizierConfig {
    /// Reads a config file. `${ENV}` references in its text are expanded, and its workspace is
    /// `.vizier` next to the file, created if missing.
    pub fn from_path(path: &Path) -> Result<Self> {
        let raw_string = fs::read_to_string(path)
            .with_context(|| format!("failed to read config file: {}", path.display()))?;
        let parent_path = if path.parent().map(|p| p.as_os_str().is_empty()).unwrap_or(true) {
            PathBuf::from_str("./").unwrap()
        } else {
            path.parent().unwrap().to_path_buf()
        };

        let config_string = shellexpand::env(&raw_string)?;
        let mut config = serde_yaml::from_str::<AllConfig>(&config_string)?;

        let mut ws = parent_path;
        ws.push(".vizier");
        let _ = fs::create_dir_all(&ws)?;
        config.vizier.workspace = ws.to_string_lossy().to_string();

        Ok(config.vizier)
    }

    /// The built-in defaults on an explicit workspace. Reads nothing from the environment.
    pub fn with_workspace(workspace: PathBuf) -> Self {
        Self {
            workspace: workspace.to_string_lossy().to_string(),
            ..Default::default()
        }
    }

    /// This config with `${ENV}` references expanded in every value, as
    /// [`VizierConfig::from_path`] expands a file's text.
    pub fn expand_env(&self) -> Result<Self> {
        let yaml = serde_yaml::to_string(&AllConfig {
            vizier: self.clone(),
        })?;
        let expanded = shellexpand::env(&yaml)?;
        Ok(serde_yaml::from_str::<AllConfig>(&expanded)?.vizier)
    }

    pub fn save(&self, path: std::path::PathBuf, addition: String) -> Result<()> {
        if let Some(parent_dir) = path.parent() {
            let _ = std::fs::create_dir_all(parent_dir)?;
        }

        let _ = fs::write(
            &path,
            format!(
                "{}\n\n{addition}",
                serde_yaml::to_string(&AllConfig {
                    vizier: self.clone(),
                })?
            ),
        )?;

        Ok(())
    }

    pub fn apply_overrides(&mut self, overrides: &RunOverrides) {
        if let Some(ref w) = overrides.workspace {
            self.workspace = w.to_string_lossy().to_string();
            let _ = fs::create_dir_all(w);
        }
        if let Some(p) = overrides.port {
            let http = self.channels.http.get_or_insert_with(default_http_channel);
            http.port = p;
        }
        if let Some(ref s) = overrides.storage {
            self.storage = s.clone();
        }
        if let Some(o) = &overrides.objects {
            self.objects = o.clone();
        }
        if let Some(w) = overrides.workers {
            self.worker_threads = w;
        }
        if let Some(t) = overrides.ws_idle_timeout {
            let http = self.channels.http.get_or_insert_with(default_http_channel);
            http.ws_idle_timeout_secs = t;
        }
        if let Some(r) = overrides.catalogue_refresh {
            self.catalogue.refresh = r;
        }
        if let Some(t) = overrides.catalogue_ttl {
            self.catalogue.ttl = t;
        }
    }
}

fn default_http_channel() -> HTTPChannelConfig {
    HTTPChannelConfig {
        port: 9999,
        jwt_secret: "${VIZIER_JWT_SECRET}".into(),
        jwt_expiry_hours: 720,
        ws_idle_timeout_secs: 300,
        cors_origins: None,
        host: None,
    }
}

impl Default for VizierConfig {
    fn default() -> Self {
        VizierConfig {
            workspace: String::new(),
            storage: StorageConfig::Sqlite,
            objects: ObjectsConfig::Local,
            providers: None,
            channels: ChannelsConfig {
                http: Some(default_http_channel()),
            },
            worker_threads: 4,
            catalogue: CatalogueConfig::default(),
            lsp: LspConfig::default(),
            subagents: SubagentsConfig::default(),
        }
    }
}

#[cfg(test)]
mod memory_limits_tests {
    use super::MemoryLimits;

    #[test]
    fn a_bad_pairing_behaves_as_no_warning_band_instead_of_failing() {
        // `warn_at` above the ceiling is a configuration mistake: degrade, never error.
        let bad = MemoryLimits {
            content_cap: 4096,
            warn_at: 8192,
        };
        assert_eq!(bad.warn_threshold(), 4096);
        // Nothing can be "over the soft limit but under the ceiling" any more.
        assert!(!bad.warns_at(4096));
        assert!(!bad.warns_at(1000));
        // The ceiling itself still refuses.
        assert!(bad.exceeds_cap(4097));
    }

    #[test]
    fn the_warning_band_is_exclusive_at_the_soft_limit_and_inclusive_at_the_ceiling() {
        let limits = MemoryLimits {
            content_cap: 8192,
            warn_at: 4096,
        };
        assert!(!limits.warns_at(4096), "exactly at the soft limit is clean");
        assert!(limits.warns_at(4097), "one byte over warns");
        assert!(limits.warns_at(8192), "exactly at the ceiling still wins");
        assert!(!limits.warns_at(8193), "over the ceiling is a refusal, not a warning");
        assert!(limits.exceeds_cap(8193));
        assert!(!limits.exceeds_cap(8192), "equality is accepted");
    }

    #[test]
    fn the_identity_pair_is_tighter_than_the_ordinary_pair() {
        let ordinary = MemoryLimits::default();
        let identity = MemoryLimits::identity();
        assert_eq!((ordinary.content_cap, ordinary.warn_at), (8192, 4096));
        assert_eq!((identity.content_cap, identity.warn_at), (4096, 1536));
        assert!(identity.content_cap < ordinary.content_cap);
        assert!(identity.warn_at < ordinary.warn_at);
    }

    #[test]
    fn an_absent_config_deserialises_to_the_defaults() {
        // An agent config saved before this existed must load unchanged.
        let limits: MemoryLimits = serde_yaml::from_str("{}").unwrap();
        assert_eq!(limits, MemoryLimits::default());
    }
}

#[cfg(test)]
mod lsp_config_tests {
    use super::*;

    #[test]
    fn a_config_without_the_section_gets_the_default() {
        let lsp: LspConfig = serde_yaml::from_str("{}").unwrap();
        assert_eq!(lsp.max_servers, 6);
        let set: LspConfig = serde_yaml::from_str("max_servers: 2").unwrap();
        assert_eq!(set.max_servers, 2);
    }

    #[test]
    fn subagents_default_to_main_agent_only() {
        let config: SubagentsConfig = serde_yaml::from_str("{}").unwrap();
        assert_eq!(config.max_depth, 1);
        let set: SubagentsConfig = serde_yaml::from_str("max_depth: 2").unwrap();
        assert_eq!(set.max_depth, 2);
    }
}
