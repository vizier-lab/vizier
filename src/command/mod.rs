use std::{path::PathBuf, sync::Arc, time::Duration};
use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    net::{UnixListener, UnixStream},
};

use crate::{
    config::VizierConfig,
    dependencies::VizierDependencies,
    schema::{CommandRequest, CommandResponse},
};
use anyhow::Result;

/// How long one command over the socket may take, start to finish.
const COMMAND_TIMEOUT: Duration = Duration::from_secs(15);

pub struct VizierCommandServer {
    deps: VizierDependencies,
}

impl VizierCommandServer {
    pub fn new(deps: VizierDependencies) -> Result<Self> {
        Ok(Self { deps })
    }

    pub async fn run(&self) -> Result<()> {
        let sock_path = PathBuf::from(&self.deps.config.workspace)
            .join(".runtime")
            .join(".vizier.sock");

        // remove existing socket
        let _ = std::fs::remove_file(&sock_path);

        let listener = match UnixListener::bind(&sock_path) {
            Ok(listener) => listener,
            // Everything but `vizier shutdown` and `vizier agent ps` works without the
            // socket, so this is not a reason to stop the process (#51).
            Err(e) => {
                tracing::error!(
                    "can't open the command socket at {}: {e}. `vizier shutdown` and \
                     `vizier agent ps` won't reach this process",
                    sock_path.display()
                );
                std::future::pending::<()>().await;
                return Ok(());
            }
        };

        loop {
            let stream = match listener.accept().await {
                Ok((stream, _)) => stream,
                Err(e) => {
                    tracing::warn!("command socket: accept failed: {e}");
                    tokio::time::sleep(Duration::from_millis(100)).await;
                    continue;
                }
            };
            // One connection at a time: responses come back on one shared channel, so two
            // commands in flight could be handed each other's answers.
            match tokio::time::timeout(COMMAND_TIMEOUT, self.serve(stream)).await {
                Ok(Ok(())) => {}
                Ok(Err(e)) => tracing::warn!("command socket: {e:#}"),
                Err(_) => tracing::warn!("command socket: a command timed out"),
            }
        }
    }

    /// Read one command, answer it, and close.
    async fn serve(&self, mut stream: UnixStream) -> Result<()> {
        let mut msg = vec![0; 1024 * 1024];
        let n = stream.read(&mut msg).await?;
        if n == 0 {
            return Ok(());
        }

        let response = match serde_json::from_slice::<CommandRequest>(&msg[..n]) {
            Ok(request) => {
                self.deps.transport.send_command_request(request).await?;
                self.deps.transport.recv_command_response().await?
            }
            Err(e) => CommandResponse::Error(format!("not a command: {e}")),
        };

        stream.write_all(&serde_json::to_vec(&response)?).await?;
        Ok(())
    }
}

pub struct VizierCommandClient {
    config: VizierConfig,
}

impl VizierCommandClient {
    pub fn new(config: VizierConfig) -> Result<Self> {
        Ok(Self { config })
    }

    pub async fn send_command(&self, command: CommandRequest) -> Result<CommandResponse> {
        let sock_path = PathBuf::from(&self.config.workspace)
            .join(".runtime")
            .join(".vizier.sock");

        let stream = Arc::new(UnixStream::connect(&sock_path).await?);

        let reader = stream.clone();

        // send command
        stream.writable().await?;
        let msg = serde_json::to_vec(&command).unwrap();
        stream.try_write(&msg)?;

        // wait for response
        reader.readable().await?;
        let mut msg = vec![0; 1024 * 1024];
        let n = reader.try_read(&mut msg)?;
        msg.truncate(n);

        let raw = String::from_utf8(msg.clone())?;
        let res = serde_json::from_str(&raw).unwrap();

        Ok(res)
    }
}
