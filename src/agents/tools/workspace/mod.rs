use std::marker::PhantomData;

use serde::{Deserialize, Serialize};

use crate::{
    agents::tools::{ToolContext, VizierTool},
    error::{VizierError, throw_vizier_error},
    utils::build_path,
};

pub trait PrimaryDocument {
    const NAME: &'static str;
    const WRITE_NAME: &'static str;
    const READ_NAME: &'static str;
}

pub struct AgentDocument;

impl PrimaryDocument for AgentDocument {
    const NAME: &'static str = "SOUL.md";
    const WRITE_NAME: &'static str = "WRITE_SOUL";
    const READ_NAME: &'static str = "READ_SOUL";
}

pub struct IdentDocument;

impl PrimaryDocument for IdentDocument {
    const NAME: &'static str = "IDENTITY.md";
    const WRITE_NAME: &'static str = "WRITE_IDENTITY";
    const READ_NAME: &'static str = "READ_IDENTITY";
}

pub struct HeartbeatDocument;

impl PrimaryDocument for HeartbeatDocument {
    const NAME: &'static str = "HEARTBEAT.md";
    const WRITE_NAME: &'static str = "WRITE_HEARTBEAT";
    const READ_NAME: &'static str = "READ_HEARTBEAT";
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

#[async_trait::async_trait]
impl<T: PrimaryDocument> VizierTool for WritePrimaryDocument<T>
where
    Self: Sync + Send,
{
    type Input = WritePrimaryDocumentArgs;
    type Output = String;

    fn name() -> String {
        T::WRITE_NAME.to_string()
    }

    fn description(&self) -> String {
        format!(
            "write over your {}, **not append**. Always tell user after updating!",
            T::NAME.trim_end_matches(".md")
        )
    }

    async fn call(&self, args: Self::Input, _ctx: &ToolContext) -> Result<Self::Output, VizierError> {
        let path = build_path(&self.workspace, &[T::NAME]);

        std::fs::write(path, args.content)
            .map_err(|err| VizierError(err.to_string()))?;

        Ok(format!("{} updated successfully", T::NAME))
    }
}

pub struct ReadPrimaryDocument<T: PrimaryDocument> {
    _phantom_data: PhantomData<T>,
    workspace: String,
}

impl<T: PrimaryDocument> ReadPrimaryDocument<T> {
    pub fn new(workspace: String) -> Self {
        Self {
            _phantom_data: PhantomData,
            workspace,
        }
    }
}

#[derive(Debug, Deserialize, Serialize, schemars::JsonSchema)]
pub struct ReadPrimaryDocumentArgs {}

#[async_trait::async_trait]
impl<T: PrimaryDocument> VizierTool for ReadPrimaryDocument<T>
where
    Self: Sync + Send,
{
    type Input = ReadPrimaryDocumentArgs;
    type Output = String;

    fn name() -> String {
        T::READ_NAME.to_string()
    }

    fn description(&self) -> String {
        format!("read your {}", T::NAME.trim_end_matches(".md"))
    }

    async fn call(&self, _args: Self::Input, _ctx: &ToolContext) -> Result<Self::Output, VizierError> {
        let path = build_path(&self.workspace, &[T::NAME]);
        let content = std::fs::read_to_string(path).map_err(|err| VizierError(err.to_string()))?;

        Ok(content)
    }
}
