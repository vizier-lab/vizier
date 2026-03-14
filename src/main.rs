extern crate pretty_env_logger;
#[allow(unused)]
#[macro_use]
extern crate log;

use std::process;

use anyhow::Result;

mod agent;
mod channels;
mod cli;
mod config;
mod constant;
mod database;
mod dependencies;
mod embedding;
mod error;
mod scheduler;
mod schema;
mod transport;
mod utils;

#[tokio::main(flavor = "multi_thread")]
async fn main() -> Result<()> {
    dotenvy::dotenv().ok();

    cli::start().await?;
    println!("vizier exited!");
    process::exit(0);
}
