use std::fmt::Display;
use std::sync::Arc;

use anyhow::Result;
use surrealdb::Surreal;
use surrealdb::engine::local::{Db, RocksDb};

use crate::{embedding::VizierEmbedder, storage::VizierStorageProvider, utils::build_path};

pub mod history;
pub mod memory;
pub mod query;
pub mod session;
pub mod shared_document;
pub mod skill;
pub mod state;
pub mod task;
pub mod user;

#[derive(Clone)]
pub struct SurrealStorage {
    pub conn: Arc<Surreal<Db>>,
    pub embedder: Option<Arc<VizierEmbedder>>,
}

impl SurrealStorage {
    pub async fn new(workspace: String, embedder: Option<Arc<VizierEmbedder>>) -> Result<Self> {
        let db_path = build_path(&workspace, &[".runtime", "surreal"]);
        let db = Surreal::new::<RocksDb>(db_path).await?;
        db.use_ns("vizier").use_db("v1").await?;

        db.query("DEFINE TABLE memory SCHEMALESS;").await?;
        db.query("DEFINE TABLE task SCHEMALESS;").await?;
        db.query("DEFINE TABLE session_history SCHEMALESS;").await?;
        db.query("DEFINE TABLE document_index SCHEMALESS;").await?;
        db.query("DEFINE TABLE skill SCHEMALESS;").await?;
        db.query("DEFINE TABLE session_detail SCHEMALESS;").await?;
        db.query("DEFINE TABLE user SCHEMALESS;").await?;
        db.query("DEFINE TABLE api_key SCHEMALESS;").await?;
        db.query("DEFINE TABLE shared_document SCHEMALESS;").await?;

        let res = Self {
            conn: Arc::new(db),
            embedder,
        };

        Ok(res)
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
