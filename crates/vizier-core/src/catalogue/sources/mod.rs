//! The shipped catalogue sources (spec 068). Each is one file; [`standard`] is the one place
//! their registration order is written (`contracts/catalogue-source.md` §2).

use std::sync::Arc;

use super::source::CatalogueSource;

pub mod curated;
pub mod models_dev;
pub mod openrouter;
pub mod provider_list;
pub mod venice;

/// Every shipped source except models.dev, which `Catalogue` appends last. Authority decides
/// precedence; this order only breaks ties within one level.
pub fn standard() -> Vec<Arc<dyn CatalogueSource>> {
    vec![
        Arc::new(openrouter::source()),
        Arc::new(venice::source()),
        Arc::new(curated::Curated),
    ]
}
