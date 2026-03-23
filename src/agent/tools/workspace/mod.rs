use std::marker::PhantomData;

use rig::{completion::ToolDefinition, tool::Tool};
use schemars::schema_for;
use serde::{Deserialize, Serialize};

use crate::error::{VizierError, throw_vizier_error};

pub trait PrimaryDocument {
    const NAME: &'static str;
    const WRITE_NAME: &'static str;
}

pub struct AgentDocument;

impl PrimaryDocument for AgentDocument {
    const NAME: &'static str = "AGENT.md";
    const WRITE_NAME: &'static str = "WRITE_AGENT_MD_FILE";
}

pub struct IdentDocument;

impl PrimaryDocument for IdentDocument {
    const NAME: &'static str = "IDENTITY.md";
    const WRITE_NAME: &'static str = "WRITE_IDENTITY_MD_FILE";
}

pub struct WritePrimaryDocument<T: PrimaryDocument> {
    _phantom_data: PhantomData<T>,
    workspace: String,
}

impl<T: PrimaryDocument> WritePrimaryDocument<T> {
    pub fn new(workspace: String) -> Self {
        Self {
            _phantom_data: PhantomData,
            workspace,
        }
    }
}

#[derive(Debug, Deserialize, Serialize, schemars::JsonSchema)]
pub struct WritePrimaryDocumentArgs {
    #[schemars(description = "New content of the file")]
    content: String,
}

impl<T: PrimaryDocument> Tool for WritePrimaryDocument<T>
where
    Self: Sync + Send,
{
    const NAME: &'static str = T::WRITE_NAME;
    type Error = VizierError;
    type Args = WritePrimaryDocumentArgs;
    type Output = ();

    async fn definition(&self, _prompt: String) -> ToolDefinition {
        let parameters = serde_json::to_value(schema_for!(Self::Args)).unwrap();

        ToolDefinition {
            name: Self::NAME.to_string(),
            description: format!(
                "write over the content {} file, **not append**. Always tell user after updating document!",
                T::NAME
            ),
            parameters,
        }
    }

    async fn call(&self, args: Self::Args) -> Result<Self::Output, Self::Error> {
        log::info!("write {}", T::NAME);

        let path = std::path::PathBuf::from(format!("{}/{}", self.workspace, T::NAME));

        match std::fs::write(path, args.content) {
            Ok(_) => Ok(()),
            Err(err) => throw_vizier_error("write file", err),
        }
    }
}
