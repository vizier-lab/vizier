//! The server process: the engine plus the HTTP and command servers, under the restart rule
//! of #51 (any subsystem that stops exits the process with 1, so Docker or systemd starts
//! vizier again whole; a requested exit or OS termination signal is 0).

use std::time::Duration;

use anyhow::Result;

use crate::command::VizierCommandServer;
use vizier_core::{
    config::VizierConfig,
    runtime::{RuntimeExit, RuntimeOptions, Subsystems, VizierRuntime},
};
use vizier_http::health::HealthHandle;

enum Stop {
    Graceful(&'static str),
    Failed { name: &'static str, outcome: String },
}

pub fn run_server(config: VizierConfig, shutdown_grace: Duration) -> Result<()> {
    let mut builder = tokio::runtime::Builder::new_multi_thread();
    builder.worker_threads(config.worker_threads);

    let runtime = builder.enable_all().build()?;

    runtime.block_on(async move {
        let mut engine = VizierRuntime::start(RuntimeOptions {
            config: config.clone(),
        })
        .await?;

        // The binary's own subsystems, next to the engine's.
        let mut servers = Subsystems::default();
        let health = HealthHandle::new();
        let mut http_shutdown = None;
        if let Some(http_config) = config.channels.http.clone() {
            let (shutdown_tx, shutdown_rx) = tokio::sync::oneshot::channel();
            http_shutdown = Some(shutdown_tx);
            let serve = vizier_http::serve_with_shutdown(
                &engine,
                http_config,
                health.clone(),
                async move {
                    let _ = shutdown_rx.await;
                },
            );
            servers.spawn("http channel", serve);
        }

        let commands = VizierCommandServer::new(engine.deps().clone())?;
        servers.spawn("command server", async move { commands.run().await });

        tracing::info!("vizier is running!");

        // Every subsystem runs for the life of the process, so one that stops, for whatever
        // reason, leaves vizier half-working. Exit instead, and let the workload manager start
        // it again whole (#51).
        let stop = tokio::select! {
            exit = engine.wait() => match exit {
                RuntimeExit::Requested => Stop::Graceful("shutdown command"),
                RuntimeExit::SubsystemStopped { name, outcome } => Stop::Failed { name, outcome },
            },
            (name, outcome) = servers.next_stopped() => Stop::Failed { name, outcome },
            signal = shutdown_signal() => match signal {
                Ok(signal) => Stop::Graceful(signal),
                Err(error) => Stop::Failed {
                    name: "signal handler",
                    outcome: error.to_string(),
                },
            },
        };

        match stop {
            Stop::Failed { name, outcome } => {
                tracing::error!("{name} stopped ({outcome}); exiting so vizier can be restarted");
                std::process::exit(1);
            }
            Stop::Graceful(reason) => {
                health.begin_shutdown();
                tracing::info!(
                    reason,
                    grace_seconds = shutdown_grace.as_secs(),
                    "graceful shutdown started"
                );
                if let Some(shutdown) = http_shutdown {
                    let _ = shutdown.send(());
                }

                let shutdown = async move {
                    // The HTTP subsystem returns only after accepted requests and WebSockets
                    // close. A deadline still bounds clients that refuse to disconnect.
                    if config.channels.http.is_some() {
                        let _ = servers.next_stopped().await;
                    }
                    servers.abort_all();
                    engine.shutdown().await
                };

                match tokio::time::timeout(shutdown_grace, shutdown).await {
                    Ok(result) => result?,
                    Err(_) => tracing::warn!(
                        grace_seconds = shutdown_grace.as_secs(),
                        "graceful shutdown deadline reached; remaining work was stopped"
                    ),
                }
                tracing::info!("vizier stopped");
                Ok(())
            }
        }
    })
}

async fn shutdown_signal() -> std::io::Result<&'static str> {
    #[cfg(unix)]
    {
        let mut terminate =
            tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate())?;
        tokio::select! {
            result = tokio::signal::ctrl_c() => {
                result?;
                Ok("SIGINT")
            }
            _ = terminate.recv() => Ok("SIGTERM"),
        }
    }

    #[cfg(not(unix))]
    {
        tokio::signal::ctrl_c().await?;
        Ok("interrupt signal")
    }
}
