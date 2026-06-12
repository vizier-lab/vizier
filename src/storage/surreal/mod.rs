use std::fmt::Display;
use std::sync::Arc;

use anyhow::Result;
use surrealdb::Surreal;
use surrealdb::engine::local::{Db, RocksDb};

use crate::storage::VizierStorageProvider;
use crate::utils::build_path;

pub mod agent;
pub mod dream_journal;
pub mod global_config;
pub mod history;
pub mod memory;
pub mod provider;
pub mod query;
pub mod session;
pub mod session_file;
pub mod state;
pub mod task;
pub mod user;

#[derive(Clone)]
pub struct SurrealStorage {
    pub conn: Arc<Surreal<Db>>,
}

impl SurrealStorage {
    pub async fn open_connection(workspace: &str) -> Result<Arc<Surreal<Db>>> {
        let db_path = build_path(workspace, &[".runtime", "surreal"]);
        let db = Surreal::new::<RocksDb>(db_path).await?;
        db.use_ns("vizier").use_db("v1").await?;

        db.query("DEFINE TABLE memory SCHEMALESS;").await?;
        db.query("DEFINE TABLE task SCHEMALESS;").await?;
        db.query("DEFINE TABLE session_history SCHEMALESS;").await?;
        db.query("DEFINE TABLE document_index SCHEMALESS;").await?;
        db.query("DEFINE TABLE session_detail SCHEMALESS;").await?;
        db.query("DEFINE TABLE state SCHEMALESS;").await?;
        db.query("DEFINE TABLE user SCHEMALESS;").await?;
        db.query("DEFINE TABLE user_profile SCHEMALESS;").await?;
        db.query("DEFINE TABLE role SCHEMALESS;").await?;
        db.query("DEFINE TABLE api_key SCHEMALESS;").await?;
        db.query("DEFINE TABLE agent_config SCHEMALESS;").await?;
        db.query("DEFINE TABLE provider_config SCHEMALESS;").await?;
        db.query("DEFINE TABLE global_config SCHEMALESS;").await?;
        db.query("DEFINE TABLE dream_journal SCHEMALESS;").await?;
        db.query("DEFINE TABLE session_file SCHEMALESS;").await?;

        Ok(Arc::new(db))
    }

    pub fn from_conn(conn: Arc<Surreal<Db>>) -> Self {
        Self { conn }
    }
}

impl VizierStorageProvider for SurrealStorage {}

#[allow(unused)]
pub enum DistanceFunction {
    Knn,
    Hamming,
    Euclidean,
    Cosine,
    Jaccard,
}

impl Display for DistanceFunction {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        match self {
            DistanceFunction::Cosine => write!(f, "vector::similarity::cosine"),
            DistanceFunction::Knn => write!(f, "vector::distance::knn"),
            DistanceFunction::Euclidean => write!(f, "vector::distance::euclidean"),
            DistanceFunction::Hamming => write!(f, "vector::distance::hamming"),
            DistanceFunction::Jaccard => write!(f, "vector::similarity::jaccard"),
        }
    }
}
