use std::collections::HashMap;
use std::sync::Arc;

use anyhow::Result;
use tokio::sync::RwLock;
use tokio::task::JoinSet;

use crate::schema::{
    AgentCommand, AgentId, ChannelCommand, CommandRequest, CommandResponse, FileCommand,
    VizierAttachment, VizierRequest, VizierResponse, VizierSession,
};

#[derive(Debug, Clone)]
pub struct DreamCommand {
    pub agent_id: AgentId,
    pub cycle_id: Option<String>,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct VizierRequestEnvelope {
    pub session: VizierSession,
    pub request: VizierRequest,
    #[serde(skip)]
    pub response_tx: Option<flume::Sender<VizierResponse>>,
}

#[derive(Debug, Clone)]
pub struct VizierTransport {
    agent_channels: Arc<RwLock<HashMap<AgentId, flume::Sender<VizierRequestEnvelope>>>>,

    command_request_channel: Arc<(
        flume::Sender<CommandRequest>,
        flume::Receiver<CommandRequest>,
    )>,

    command_response_channel: Arc<(
        flume::Sender<CommandResponse>,
        flume::Receiver<CommandResponse>,
    )>,

    agent_command_channel: Arc<(flume::Sender<AgentCommand>, flume::Receiver<AgentCommand>)>,

    channel_command_channel: Arc<(
        flume::Sender<ChannelCommand>,
        flume::Receiver<ChannelCommand>,
    )>,

    exit_channel: Arc<(flume::Sender<bool>, flume::Receiver<bool>)>,

    dream_command_channel: Arc<(flume::Sender<DreamCommand>, flume::Receiver<DreamCommand>)>,

    file_command_channel: Arc<(flume::Sender<FileCommand>, flume::Receiver<FileCommand>)>,
}

impl VizierTransport {
    pub fn new() -> Self {
        let command_request_channel = Arc::new(flume::unbounded());
        let command_response_channel = Arc::new(flume::unbounded());
        let agent_command_channel = Arc::new(flume::unbounded());
        let channel_command_channel = Arc::new(flume::unbounded());
        let exit_channel = Arc::new(flume::unbounded());
        let dream_command_channel = Arc::new(flume::unbounded());
        let file_command_channel = Arc::new(flume::unbounded());

        Self {
            agent_channels: Arc::new(RwLock::new(HashMap::new())),
            command_request_channel,
            command_response_channel,
            agent_command_channel,
            channel_command_channel,
            exit_channel,
            dream_command_channel,
            file_command_channel,
        }
    }

    pub async fn register_agent(
        &self,
        agent_id: AgentId,
    ) -> flume::Receiver<VizierRequestEnvelope> {
        let (tx, rx) = flume::unbounded();
        let mut channels = self.agent_channels.write().await;
        channels.insert(agent_id, tx);
        rx
    }

    pub async fn unregister_agent(&self, agent_id: &AgentId) {
        let mut channels = self.agent_channels.write().await;
        channels.remove(agent_id);
    }

    pub async fn send_request(
        &self,
        session: VizierSession,
        req: VizierRequest,
        response_tx: Option<flume::Sender<VizierResponse>>,
    ) -> Result<()> {
        let agent_id = &session.0;
        let channels = self.agent_channels.read().await;
        let tx = channels
            .get(agent_id)
            .ok_or_else(|| anyhow::anyhow!("agent '{}' not registered", agent_id))?;
        tx.send_async(VizierRequestEnvelope {
            session,
            request: req,
            response_tx,
        })
        .await?;
        Ok(())
    }

    pub async fn send_command_request(&self, req: CommandRequest) -> Result<()> {
        Ok(self.command_request_channel.0.send_async(req).await?)
    }

    pub async fn recv_command_request(&self) -> Result<CommandRequest> {
        Ok(self.command_request_channel.1.recv_async().await?)
    }

    pub async fn send_command_response(&self, req: CommandResponse) -> Result<()> {
        Ok(self.command_response_channel.0.send_async(req).await?)
    }

    pub async fn recv_command_response(&self) -> Result<CommandResponse> {
        Ok(self.command_response_channel.1.recv_async().await?)
    }

    pub async fn send_agent_command(&self, cmd: AgentCommand) -> Result<()> {
        Ok(self.agent_command_channel.0.send_async(cmd).await?)
    }

    pub async fn recv_agent_command(&self) -> Result<AgentCommand> {
        Ok(self.agent_command_channel.1.recv_async().await?)
    }

    pub async fn send_channel_command(&self, cmd: ChannelCommand) -> Result<()> {
        Ok(self.channel_command_channel.0.send_async(cmd).await?)
    }

    pub async fn recv_channel_command(&self) -> Result<ChannelCommand> {
        Ok(self.channel_command_channel.1.recv_async().await?)
    }

    pub async fn send_dream_command(&self, cmd: DreamCommand) -> Result<()> {
        Ok(self.dream_command_channel.0.send_async(cmd).await?)
    }

    pub async fn recv_dream_command(&self) -> Result<DreamCommand> {
        Ok(self.dream_command_channel.1.recv_async().await?)
    }

    pub async fn send_file_command(&self, cmd: FileCommand) -> Result<()> {
        Ok(self.file_command_channel.0.send_async(cmd).await?)
    }

    pub async fn recv_file_command(&self) -> Result<FileCommand> {
        Ok(self.file_command_channel.1.recv_async().await?)
    }

    pub async fn send_file_upload(
        &self,
        filename: String,
        content: Vec<u8>,
    ) -> Result<crate::schema::FileRecord> {
        let (tx, rx) = tokio::sync::oneshot::channel();
        self.send_file_command(FileCommand::Upload {
            filename,
            content,
            response: tx,
        })
        .await?;
        rx.await?
    }

    pub async fn send_file_resolve(
        &self,
        attachment: VizierAttachment,
    ) -> Result<Vec<u8>> {
        let (tx, rx) = tokio::sync::oneshot::channel();
        self.send_file_command(FileCommand::Resolve {
            attachment,
            response: tx,
        })
        .await?;
        rx.await?
    }

    pub async fn exit_signal(&self) -> Result<bool> {
        Ok(self.exit_channel.1.recv_async().await?)
    }

    pub async fn run(&self) -> Result<()> {
        let mut set = JoinSet::new();

        let req_rx = self.command_request_channel.clone().1.clone();
        let res = self.command_response_channel.clone().0.clone();
        let exit = self.exit_channel.clone().0.clone();
        let agent_cmd_tx = self.agent_command_channel.clone().0.clone();
        set.spawn(async move {
            while let Ok(req) = req_rx.recv_async().await {
                match req {
                    CommandRequest::Exit => {
                        let _ = res
                            .send_async(CommandResponse::Ok("vizier is stopping".into()))
                            .await;
                        let _ = exit.send_async(true).await;
                    }
                    CommandRequest::HealthCheck => {
                        let (tx, rx) = tokio::sync::oneshot::channel();
                        let _ = agent_cmd_tx
                            .send_async(AgentCommand::HealthCheck { resp: tx })
                            .await;
                        match rx.await {
                            Ok(statuses) => {
                                let json = serde_json::to_string(&statuses).unwrap_or_default();
                                let _ = res.send_async(CommandResponse::Ok(json)).await;
                            }
                            Err(_) => {
                                let _ = res
                                    .send_async(CommandResponse::Error(
                                        "health check failed".into(),
                                    ))
                                    .await;
                            }
                        }
                    }
                    _ => unimplemented!(),
                }
            }
        });

        set.join_all().await;
        Ok(())
    }
}
