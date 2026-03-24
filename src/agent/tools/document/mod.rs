use std::path::PathBuf;
use std::sync::Arc;

use anyhow::Result;
use rig::completion::ToolDefinition;
use rig::tool::Tool;
use schemars::schema_for;
use serde::{Deserialize, Serialize};

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

impl Tool for DocumentRead {
    const NAME: &'static str = "documents_search";
    type Error = VizierError;
    type Args = DocumentReadArgs;
    type Output = Vec<String>;

    async fn definition(&self, _prompt: String) -> ToolDefinition {
        let parameters = serde_json::to_value(schema_for!(Self::Args)).unwrap();

        ToolDefinition {
            name: Self::NAME.to_string(),
            description: "Search any included documents for informations".into(),
            parameters,
        }
    }

    async fn call(&self, args: Self::Args) -> Result<Self::Output, Self::Error> {
        let res = self
            .1
            // TODO: don't hardcode the threshold
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
