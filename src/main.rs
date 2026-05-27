#[allow(unused)]
#[macro_use]
extern crate tracing;

use std::process;

use crate::error::VizierError;
use tracing_subscriber::{EnvFilter, fmt};

pub type Result<T> = std::result::Result<T, VizierError>;

mod agents;
mod channels;
mod cli;
mod command;
mod config;
mod constant;
mod dependencies;
mod embedding;
mod error;
mod mcp;
mod scheduler;
mod schema;
mod shell;
mod storage;
mod transport;
mod utils;

fn main() -> Result<()> {
    dotenvy::dotenv().ok();

    if std::env::var("RUST_LOG").is_err() {
        let filter = EnvFilter::try_from_default_env()
            .unwrap_or_else(|_| EnvFilter::new("vizier=debug"))
            .add_directive("rig=error".parse().unwrap())
            .add_directive("serenity=error".parse().unwrap())
            .add_directive("sqlx=error".parse().unwrap())
            .add_directive("reqwest=error".parse().unwrap())
            .add_directive("hyper=error".parse().unwrap())
            .add_directive("tungstenite=error".parse().unwrap())
            .add_directive("h2=error".parse().unwrap())
            .add_directive("tracing=off".parse().unwrap())
            .add_directive("rustls=off".parse().unwrap())
            .add_directive("surrealdb=off".parse().unwrap())
            .add_directive("ort=off".parse().unwrap())
            .add_directive("ureq=off".parse().unwrap())
            .add_directive("bollard=off".parse().unwrap())
            .add_directive("rmcp=off".parse().unwrap())
            .add_directive("rustpython=off".parse().unwrap());

        fmt().with_env_filter(filter).compact().init();
    } else {
        fmt().compact().init();
    }

    if let Err(err) = cli::start() {
        tracing::error!("{}", err)
    }
    process::exit(0);
}
