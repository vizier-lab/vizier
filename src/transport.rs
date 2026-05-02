use std::sync::Arc;

use anyhow::Result;
use async_broadcast::{Receiver, Sender, broadcast};
use tokio::task::JoinSet;

use crate::schema::{
    CommandRequest, CommandResponse, VizierRequest, VizierResponse, VizierSession,
};

#[derive(Debug, Clone)]
pub struct VizierTransport {
    request_channel: Arc<(
        Sender<(VizierSession, VizierRequest)>,
        Receiver<(VizierSession, VizierRequest)>,
    )>,

    response_channel: Arc<(
        Sender<(VizierSession, VizierResponse)>,
        Receiver<(VizierSession, VizierResponse)>,
    )>,

    command_request_channel: Arc<(
        flume::Sender<CommandRequest>,
        flume::Receiver<CommandRequest>,
    )>,

    command_response_channel: Arc<(
        flume::Sender<CommandResponse>,
        flume::Receiver<CommandResponse>,
    )>,

    exit_channel: Arc<(flume::Sender<bool>, flume::Receiver<bool>)>,
}

impl VizierTransport {
    pub fn new() -> Self {
        let mut request_channel = broadcast(1000);
        request_channel.0.set_overflow(true);
        request_channel.1.set_overflow(true);

        let mut response_channel = broadcast(1000);
        response_channel.0.set_overflow(true);
        response_channel.1.set_overflow(true);

        let command_request_channel = Arc::new(flume::unbounded());
        let command_response_channel = Arc::new(flume::unbounded());

        let exit_channel = Arc::new(flume::unbounded());

        Self {
            request_channel: Arc::new(request_channel),
            response_channel: Arc::new(response_channel),

            command_request_channel,
            command_response_channel,

            exit_channel,
        }
    }

    pub async fn send_request(&self, session: VizierSession, req: VizierRequest) -> Result<()> {
        self.request_channel.0.broadcast((session, req)).await?;
        Ok(())
    }

    pub async fn subscribe_request(&self) -> Result<Receiver<(VizierSession, VizierRequest)>> {
        Ok(self.request_channel.1.clone())
    }

    pub async fn send_response(&self, session: VizierSession, res: VizierResponse) -> Result<()> {
        self.response_channel.0.broadcast((session, res)).await?;
        Ok(())
    }

    pub async fn subscribe_response(&self) -> Result<Receiver<(VizierSession, VizierResponse)>> {
        Ok(self.response_channel.1.clone())
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

    pub async fn exit_signal(&self) -> Result<bool> {
        Ok(self.exit_channel.1.recv_async().await?)
    }

    pub async fn run(&self) -> Result<()> {
        let mut set = JoinSet::new();

        // handle internal commands
        let req_rx = self.command_request_channel.clone().1.clone();
        let res = self.command_response_channel.clone().0.clone();
        let exit = self.exit_channel.clone().0.clone();
        set.spawn(async move {
            while let Ok(req) = req_rx.recv_async().await {
                match req {
                    CommandRequest::Exit => {
                        let _ = res
                            .send_async(CommandResponse::Ok("vizier is stopping".into()))
                            .await;
                        let _ = exit.send_async(true).await;
                    }
                    _ => unimplemented!(),
                }
            }
        });

        // log all request
        let mut req_rx = self.request_channel.1.clone();
        set.spawn(async move {
            while let Ok((session, req)) = req_rx.recv().await {
                tracing::info!("[Request]: {:?} {:?}", session, req);
            }
        });

        let mut res_rx = self.response_channel.1.clone();
        set.spawn(async move {
            while let Ok((session, res)) = res_rx.recv().await {
                tracing::info!("[Response]: {:?} {:?}", session, res);
            }
        });

        set.join_all().await;
        Ok(())
    }
}
