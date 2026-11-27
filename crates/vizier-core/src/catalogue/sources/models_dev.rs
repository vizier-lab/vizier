//! models.dev as one source (spec 015's catalogue, unchanged): the build-time seed, the SQLite
//! cache and the lazy refresh in `refresh.rs`. It is the source of provider connections
//! (protocol, base URL, credential variables), and a `General` source of model facts and
//! prices for every provider.

use std::sync::Arc;

use parking_lot::RwLock;

use crate::catalogue::data::CatalogueData;
use crate::catalogue::price::Price;
use crate::catalogue::source::{
    Authority, CatalogueSource, ModelFacts, ProviderConnection, SourceData, SourceId,
    SourceProvider,
};
use crate::config::provider::ProviderId;

pub struct ModelsDev {
    /// The data as models.dev gave it, and the same data as a source snapshot.
    state: RwLock<(Arc<CatalogueData>, Arc<SourceData>)>,
}

impl ModelsDev {
    pub fn new(data: CatalogueData) -> Self {
        let source = Arc::new(to_source(&data));
        Self {
            state: RwLock::new((Arc::new(data), source)),
        }
    }

    /// models.dev's own data, not the merged view: what its refresh compares and caches.
    pub fn raw(&self) -> Arc<CatalogueData> {
        self.state.read().0.clone()
    }

    pub fn set(&self, data: CatalogueData) {
        let source = Arc::new(to_source(&data));
        *self.state.write() = (Arc::new(data), source);
    }
}

fn to_source(data: &CatalogueData) -> SourceData {
    let providers = data
        .providers
        .iter()
        .map(|(id, p)| {
            let models = p
                .models
                .iter()
                .map(|(mid, m)| {
                    let facts = ModelFacts {
                        name: Some(m.name.clone()),
                        kind: m.kind,
                        caps: m.caps.clone(),
                        limit: m.limit.clone(),
                        price: m.cost.clone().map(Price::tokens),
                        status: Some(m.status),
                        release_date: m.release_date,
                        family: m.family.clone(),
                        knowledge: m.knowledge.clone(),
                        lab: m.lab.clone(),
                        route: m.route.clone(),
                    };
                    (mid.clone(), facts)
                })
                .collect();
            let provider = SourceProvider {
                connection: Some(ProviderConnection {
                    name: p.name.clone(),
                    env: p.env.clone(),
                    api: p.api.clone(),
                    npm: p.npm.clone(),
                    doc: p.doc.clone(),
                    dialect: p.dialect,
                    kind: p.kind,
                }),
                models,
                priced: Default::default(),
            };
            (id.clone(), provider)
        })
        .collect();
    SourceData {
        origin: data.source,
        fetched_at: data.fetched_at,
        checked_at: data.checked_at,
        providers,
        unmapped: 0,
    }
}

impl CatalogueSource for ModelsDev {
    fn id(&self) -> SourceId {
        SourceId::MODELS_DEV
    }

    fn name(&self) -> &'static str {
        "models.dev"
    }

    fn authority(&self, _provider: &ProviderId) -> Authority {
        Authority::General
    }

    fn data(&self) -> Arc<SourceData> {
        self.state.read().1.clone()
    }
}
