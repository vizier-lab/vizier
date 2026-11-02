//! The server process: the engine plus the HTTP and command servers, under the restart rule
//! of #51 (any subsystem that stops exits the process with 1, so Docker or systemd starts
//! vizier again whole; a requested exit is 0).

use anyhow::Result;

use crate::command::VizierCommandServer;
use vizier_core::{
    config::VizierConfig,
    error::VizierError,
    runtime::{RuntimeExit, RuntimeOptions, Subsystems, VizierRuntime},
    server_role::{ServerRole, ServerRoleSet},
};

pub fn run_server(config: VizierConfig, roles: ServerRoleSet) -> Result<()> {
    let mut builder = tokio::runtime::Builder::new_multi_thread();
    builder.worker_threads(config.worker_threads);

    let runtime = builder.enable_all().build()?;

    runtime.block_on(async move {
        let mut engine = VizierRuntime::start(RuntimeOptions {
            config: config.clone(),
            roles,
            ..Default::default()
        })
        .await?;

        // The binary's own subsystems, next to the engine's.
        let mut servers = Subsystems::default();
        if roles.contains(ServerRole::GatewayHttp) {
            match config.channels.http.clone() {
                Some(http_config) => {
                    let serve = vizier_http::serve(&engine, http_config);
                    servers.spawn("http channel", async move {
                        serve.await?;
                        Err::<(), _>(VizierError("the http server stopped".into()))
                    });
                }
                // Without an HTTP channel configured nothing is served, as before.
                None => {}
            }
        } else {
            // A process without the gateway answers nobody over HTTP; the WebUI and the API
            // are served by whichever node runs it (#169).
            tracing::info!("the gateway-http role is not in this process: nothing is served");
        }

        let commands = VizierCommandServer::new(engine.deps().clone(), roles)?;
        servers.spawn("command server", async move { commands.run().await });

        tracing::info!(roles = %roles, "vizier is running!");

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
