use std::marker::PhantomData;

use serde::{Deserialize, Serialize};

use crate::{
    agents::tools::VizierTool,
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
    const NAME: &'static str = "AGENT.md";
    const WRITE_NAME: &'static str = "WRITE_AGENT_MD_FILE";
    const READ_NAME: &'static str = "READ_AGENT_MD_FILE";
}

pub struct IdentDocument;

impl PrimaryDocument for IdentDocument {
    const NAME: &'static str = "IDENTITY.md";
    const WRITE_NAME: &'static str = "WRITE_IDENTITY_MD_FILE";
    const READ_NAME: &'static str = "READ_IDENTITY_MD_FILE";
}

pub struct HeartbeatDocument;

impl PrimaryDocument for HeartbeatDocument {
    const NAME: &'static str = "HEARTBEAT.md";
    const WRITE_NAME: &'static str = "WRITE_HEARTBEAT_MD_FILE";
    const READ_NAME: &'static str = "READ_HEARTBEAT_MD_FILE";
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
    type Output = ();

    fn name() -> String {
        T::WRITE_NAME.to_string()
    }

    fn description(&self) -> String {
        format!(
            "write over the content {} file, **not append**. Always tell user after updating document!",
            T::NAME
        )
    }

    async fn call(&self, args: Self::Input) -> Result<Self::Output, VizierError> {
        let path = build_path(&self.workspace, &[T::NAME]);

        match std::fs::write(path, args.content) {
            Ok(_) => Ok(()),
            Err(err) => throw_vizier_error("write file", err),
        }
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
        format!("read the conten of {} file", T::NAME)
    }

    async fn call(&self, _args: Self::Input) -> Result<Self::Output, VizierError> {
        let path = build_path(&self.workspace, &[T::NAME]);
        let content = std::fs::read_to_string(path).map_err(|err| VizierError(err.to_string()))?;

        Ok(content)
    }
}
