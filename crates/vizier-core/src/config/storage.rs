use std::path::PathBuf;

use serde::{Deserialize, Serialize};

/// Where relational data and vectors live (spec 045). The connection URL itself is never part of
/// the configuration: it is read from the environment variable `url_env` names, so it cannot end
/// up in a config file, a `Debug` print or a CLI argument.
#[derive(Debug, Serialize, Deserialize, Clone, Default)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum StorageConfig {
    /// No longer a selectable runtime backend (research.md §10, FR-025) — this variant survives
    /// only so an existing `.vizier.yaml`/`VIZIER_STORAGE=filesystem` from before this feature
    /// still parses, so `VizierDependencies::new` can detect it and run the one-time migration
    /// into sqlite. It is never used to construct a live storage backend; `--storage filesystem`
    /// on the CLI is rejected outright (see `cli::run::StorageKind`).
    Filesystem,
    /// The embedded database under `<workspace>/.runtime/vizier.db` (the default).
    #[default]
    Sqlite,
    /// PostgreSQL with the `vector` extension (opt-in).
    Postgres(PostgresConfig),
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct PostgresConfig {
    /// The environment variable holding the `postgres://` URL.
    #[serde(default = "default_url_env")]
    pub url_env: String,
    #[serde(default = "default_pool_size")]
    pub pool_size: u32,
    /// Extra PEM roots for the server certificate; the platform's roots otherwise.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub ca_file: Option<PathBuf>,
}

impl Default for PostgresConfig {
    fn default() -> Self {
        Self { url_env: default_url_env(), pool_size: default_pool_size(), ca_file: None }
    }
}

fn default_url_env() -> String {
    "VIZIER_DATABASE_URL".into()
}

fn default_pool_size() -> u32 {
    8
}

/// Where file bytes live: uploads, artifacts and memory documents (spec 045). Credentials are
/// never part of the configuration (environment only, see `storage::blob::s3::S3Credentials`).
#[derive(Debug, Serialize, Deserialize, Clone, Default, PartialEq)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ObjectsConfig {
    /// The workspace directory (the default).
    #[default]
    Local,
    /// An S3-compatible bucket.
    S3(S3Config),
}

#[derive(Debug, Serialize, Deserialize, Clone, Default, PartialEq)]
pub struct S3Config {
    #[serde(default)]
    pub bucket: String,
    #[serde(default)]
    pub region: String,
    /// `https://minio.internal:9000`; AWS (`https://s3.<region>.amazonaws.com`) when absent.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub endpoint: Option<String>,
    /// Path-style addressing; defaults to true when `endpoint` is set.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub path_style: Option<bool>,
    /// A key prefix inside the bucket, so several deployments can share one.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub prefix: Option<String>,
}

impl S3Config {
    /// Apply `VIZIER_S3_BUCKET`, `VIZIER_S3_REGION`, `VIZIER_S3_ENDPOINT`,
    /// `VIZIER_S3_PATH_STYLE` and `VIZIER_S3_PREFIX` over the YAML values.
    pub fn with_env_overrides(mut self) -> Self {
        self.apply_overrides(|k| std::env::var(k).ok());
        self
    }

    fn apply_overrides(&mut self, var: impl Fn(&str) -> Option<String>) {
        let set = |v: Option<String>| v.map(|s| s.trim().to_string()).filter(|s| !s.is_empty());
        if let Some(v) = set(var("VIZIER_S3_BUCKET")) {
            self.bucket = v;
        }
        if let Some(v) = set(var("VIZIER_S3_REGION")) {
            self.region = v;
        }
        if let Some(v) = set(var("VIZIER_S3_ENDPOINT")) {
            self.endpoint = Some(v);
        }
        if let Some(v) = set(var("VIZIER_S3_PATH_STYLE")) {
            self.path_style = v.to_ascii_lowercase().parse().ok();
        }
        if let Some(v) = set(var("VIZIER_S3_PREFIX")) {
            self.prefix = Some(v);
        }
    }

    pub fn path_style(&self) -> bool {
        self.path_style.unwrap_or(self.endpoint.is_some())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn existing_configs_parse_and_new_ones_round_trip() {
        let sqlite: StorageConfig = serde_yaml::from_str("type: sqlite").unwrap();
        assert!(matches!(sqlite, StorageConfig::Sqlite));

        let pg: StorageConfig = serde_yaml::from_str("type: postgres").unwrap();
        let StorageConfig::Postgres(pg) = pg else { panic!("postgres") };
        assert_eq!((pg.url_env.as_str(), pg.pool_size), ("VIZIER_DATABASE_URL", 8));

        let objects: ObjectsConfig =
            serde_yaml::from_str("type: s3\nbucket: b\nregion: r\nendpoint: http://m:9000").unwrap();
        let ObjectsConfig::S3(s3) = objects else { panic!("s3") };
        assert!(s3.path_style(), "an explicit endpoint defaults to path-style");
    }

    #[test]
    fn env_overrides_replace_yaml_values_and_ignore_blanks() {
        let mut s3 = S3Config { bucket: "yaml".into(), region: "r".into(), ..Default::default() };
        s3.apply_overrides(|k| match k {
            "VIZIER_S3_BUCKET" => Some("env".into()),
            "VIZIER_S3_REGION" => Some("  ".into()),
            "VIZIER_S3_PATH_STYLE" => Some("false".into()),
            _ => None,
        });
        assert_eq!((s3.bucket.as_str(), s3.region.as_str(), s3.path_style()), ("env", "r", false));
    }
}
