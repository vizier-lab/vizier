extern crate pretty_env_logger;
#[allow(unused)]
#[macro_use]
extern crate log;

use std::process;

use crate::error::VizierError;

pub type Result<T> = std::result::Result<T, VizierError>;

mod agent;
mod channels;
mod cli;
mod config;
mod constant;
mod dependencies;
mod embedding;
mod error;
mod scheduler;
mod schema;
mod storage;
mod transport;
mod utils;

#[tokio::main(flavor = "multi_thread")]
async fn main() -> Result<()> {
    dotenvy::dotenv().ok();

    if std::env::var("RUST_LOG").is_err() {
        pretty_env_logger::formatted_builder()
            .filter_level(log::LevelFilter::Debug)
            .filter_module("rig", log::LevelFilter::Error)
            .filter_module("serenity", log::LevelFilter::Error)
            .filter_module("sqlx", log::LevelFilter::Error)
            .filter_module("reqwest", log::LevelFilter::Error)
            .filter_module("hyper", log::LevelFilter::Error)
            .filter_module("tungstenite", log::LevelFilter::Error)
            .filter_module("sqlx", log::LevelFilter::Error)
            .filter_module("h2", log::LevelFilter::Error)
            .filter_module("tracing", log::LevelFilter::Off)
            .filter_module("rustls", log::LevelFilter::Off)
            .filter_module("surrealdb", log::LevelFilter::Off)
            .filter_module("ort", log::LevelFilter::Off)
            .filter_module("ureq", log::LevelFilter::Off)
            .init();
    } else {
        pretty_env_logger::init();
    }

    if let Err(err) = cli::start().await {
        log::error!("{}", err)
    }
    process::exit(0);
}
