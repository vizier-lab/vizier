use std::path::PathBuf;
use std::sync::Arc;

use anyhow::Result;
use serde::{Deserialize, Serialize};

use crate::agents::tools::VizierTool;
use crate::dependencies::VizierDependencies;
use crate::error::VizierError;
use crate::schema::AgentId;
use crate::storage::VizierStorage;
use crate::storage::indexer::DocumentIndexer;

pub fn init_document_tools(agent_id: String, deps: VizierDependencies) -> Result<DocumentRead> {
    Ok(DocumentRead::new(agent_id.clone(), deps.storage.clone()))
}

pub type DocumentRead = ReadDocument;
pub struct ReadDocument(AgentId, Arc<VizierStorage>);

impl DocumentRead {
    fn new(agent_id: AgentId, store: Arc<VizierStorage>) -> Self {
        Self(agent_id, store)
    }
}

#[derive(Debug, Deserialize, Serialize, schemars::JsonSchema)]
pub struct DocumentReadArgs {
    #[schemars(description = "Terms, keywords, or prompt to search")]
    pub query: String,
}

#[async_trait::async_trait]
impl VizierTool for DocumentRead {
    type Input = DocumentReadArgs;
    type Output = Vec<String>;

    fn name() -> String {
        "documents_search".to_string()
    }

    fn description(&self) -> String {
        "Search any included documents for informations".into()
    }

    async fn call(&self, args: Self::Input) -> anyhow::Result<Self::Output, VizierError> {
        let res = self
            .1
            .search_document_index(format!("document/{}", self.0.clone()), args.query, 10, 0.1)
            .await
            .unwrap();

        let mut docs = vec![];
        for index in res {
            let content = crate::utils::markdown::read_content(PathBuf::from(&index.path))?;
            docs.push(content);
        }

        Ok(docs)
    }
}

