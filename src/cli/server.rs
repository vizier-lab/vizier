//! The server process: the engine plus the HTTP and command servers, under the restart rule
//! of #51 (any subsystem that stops exits the process with 1, so Docker or systemd starts
//! vizier again whole; a requested exit is 0).

use anyhow::Result;

use crate::{
    channels::http,
    command::VizierCommandServer,
    config::VizierConfig,
    error::VizierError,
    runtime::{RuntimeExit, RuntimeOptions, Subsystems, VizierRuntime},
};

pub fn run_server(config: VizierConfig) -> Result<()> {
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
        match config.channels.http.clone() {
            Some(http_config) => {
                let serve = http::serve(&engine, http_config);
                servers.spawn("http channel", async move {
                    serve.await?;
                    Err::<(), _>(VizierError("the http server stopped".into()))
                });
            }
            // Without an HTTP channel configured nothing is served, as before.
            None => {}
        }

        let commands = VizierCommandServer::new(engine.deps().clone())?;
        servers.spawn("command server", async move { commands.run().await });

        tracing::info!("vizier is running!");

        // Every subsystem runs for the life of the process, so one that stops, for whatever
        // reason, leaves vizier half-working. Exit instead, and let whatever runs vizier
        // (Docker's restart policy, systemd) start it again whole (#51).
        let (name, outcome) = tokio::select! {
            exit = engine.wait() => match exit {
                RuntimeExit::Requested => {
                    servers.abort_all();
                    drop(engine);
                    std::process::exit(0);
                }
                RuntimeExit::SubsystemStopped { name, outcome } => (name, outcome),
            },
            stopped = servers.next_stopped() => stopped,
        };
        tracing::error!("{name} stopped ({outcome}); exiting so vizier can be restarted");
        std::process::exit(1);
    })
}
