use anyhow::Result;
use futures::lock::Mutex;
use std::{path::PathBuf, str::FromStr, sync::Arc};
use tokio::{
    io::{AsyncReadExt, AsyncWriteExt, Interest},
    net::UnixListener,
    task::JoinSet,
};

use crate::{
    channels::VizierChannel,
    schema::{SessionId, VizierRequest, VizierSession},
    transport::VizierTransport,
};

pub struct SocketChannel(pub VizierTransport);

impl VizierChannel for SocketChannel {
    async fn run(&mut self) -> Result<()> {
        let transport = self.0.clone();
        let recv = Arc::new(Mutex::new(transport.subscribe_response().await?));
        let bind_path = PathBuf::from_str("/tmp/vizier.sock")?;
        if bind_path.exists() {
            std::fs::remove_file(&bind_path)?;
        }

        let listener = Arc::new(UnixListener::bind(bind_path.clone())?);
        let mut js = JoinSet::new();

        while let Ok((socket, _)) = listener.accept().await {
            js.abort_all();
            let (mut reader, mut writer) = socket.into_split();

            let read_transport = transport.clone();
            js.spawn(async move {
                let mut msg = vec![0; 8092];
                while let Ok(_) = reader.readable().await {
                    match reader.read(&mut msg).await {
                        Ok(n) => {
                            if n == 0 {
                                break;
                            }

                            let mut msg = msg.clone();
                            msg.truncate(n);

                            let str = String::from_utf8(msg.clone()).unwrap();
                            let (session, request): (VizierSession, VizierRequest) =
                                serde_json::from_str(&str).unwrap();

                            if let Err(err) = read_transport.send_request(session, request).await {
                                log::error!("{:?}", err);
                                return;
                            }
                        }
                        Err(e) => {
                            log::error!("{:?}", e);
                            return;
                        }
                    }
                }
            });

            let recv = recv.clone();
            js.spawn(async move {
                while let Ok(_) = writer.writable().await {
                    if let Ok((session, response)) = recv.lock().await.recv().await {
                        let mut is_socket = false;
                        if let SessionId::Socket(_) = session.1 {
                            is_socket = true;
                        }

                        if !is_socket {
                            continue;
                        }

                        let msg = serde_json::to_vec(&(session.clone(), response.clone())).unwrap();

                        if let Err(err) = writer.write_all(&msg).await {
                            log::error!("{:?}", err);
                            return;
                        }
                    }
                }
            });
        }

        js.join_all().await;
        Ok(())
    }
}
