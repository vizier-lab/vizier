use std::fmt::Display;
use std::sync::Arc;

use anyhow::Result;
use surrealdb::Surreal;
use surrealdb::engine::local::{Db, RocksDb};

use crate::embedding::VizierEmbedder;
use crate::storage::VizierStorageProvider;

pub mod history;
pub mod memory;
pub mod query;
pub mod task;

#[derive(Clone)]
pub struct SurrealStorage {
    pub conn: Arc<Surreal<Db>>,
    pub embedder: Option<Arc<VizierEmbedder>>,
}

impl SurrealStorage {
    pub async fn new(workspace: String, embedder: Option<Arc<VizierEmbedder>>) -> Result<Self> {
        let db = Surreal::new::<RocksDb>(format!("{workspace}/vizier.db")).await?;
        db.use_ns("vizier").use_db("v1").await?;

        db.query("DEFINE TABLE memory SCHEMALESS;").await?;
        db.query("DEFINE TABLE task SCHEMALESS;").await?;
        db.query("DEFINE TABLE session_history SCHEMALESS;").await?;

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
