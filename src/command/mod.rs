use std::{path::PathBuf, sync::Arc};
use tokio::{
    net::{UnixListener, UnixStream},
    task::JoinSet,
};

use crate::{
    config::VizierConfig,
    dependencies::VizierDependencies,
    schema::{CommandRequest, CommandResponse},
};
use anyhow::Result;

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

        let listener = Arc::new(UnixListener::bind(&sock_path)?);

        while let Ok((stream, _)) = listener.accept().await {
            let stream = Arc::new(stream);

            let mut js = JoinSet::new();
            let mut msg = vec![0; 1024 * 1024];

            let read_transport = self.deps.transport.clone();
            let reader = stream.clone();

            js.spawn(async move {
                while let Ok(_) = reader.readable().await {
                    match reader.try_read(&mut msg) {
                        Ok(n) => {
                            msg.truncate(n);

                            if let Ok(raw) = String::from_utf8(msg.clone()) {
                                let req = serde_json::from_str(&raw).unwrap();
                                let _ = read_transport.send_command_request(req).await;
                            }
                        }
                        Err(ref e) if e.kind() == tokio::io::ErrorKind::WouldBlock => {
                            continue;
                        }
                        Err(e) => {
                            tracing::error!("{}", e);
                            break;
                        }
                    }
                }
            });

            let write_transport = self.deps.transport.clone();
            let writer = stream.clone();
            js.spawn(async move {
                while let Ok(res) = write_transport.recv_command_response().await {
                    let _ = writer.writable().await;

                    let msg = serde_json::to_vec(&res).unwrap();
                    if let Err(e) = writer.try_write(&msg) {
                        tracing::error!("{}", e);
                    }
                }
            });

            let _ = js.join_all().await;
        }

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
