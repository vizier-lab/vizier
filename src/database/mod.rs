use std::fmt::Display;
use std::sync::Arc;

use anyhow::Result;
use serde::{Deserialize, Serialize};
use surrealdb::Surreal;
use surrealdb::engine::local::{Db, RocksDb};

pub mod memory;
pub mod schema;

#[derive(Debug, Clone)]
pub struct VizierDatabases {
    pub conn: Arc<Surreal<Db>>,
}

impl VizierDatabases {
    pub async fn new(workspace: String) -> Result<Self> {
        let db = Surreal::new::<RocksDb>(format!("{workspace}/vizier.db")).await?;
        db.use_ns("vizier").use_db("v1").await?;

        let res = Self { conn: Arc::new(db) };

        Ok(res)
    }
}

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
