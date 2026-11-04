//! `vizier storage migrate`: copy a stopped workspace into PostgreSQL (and its files into the
//! configured file store). Exit codes: 0 done, 1 failed (rerun to resume), 2 refused.

use std::path::PathBuf;

use anyhow::Result;
use clap::{Args, Subcommand};
use vizier_core::config::RunOverrides;
use vizier_core::storage::migrate::{MigrateOptions, MigrateOutcome, migrate};

use super::run::{ObjectsKind, StorageKind, env_value};

#[derive(Debug, Args, Clone)]
pub struct StorageArgs {
    #[command(subcommand)]
    pub command: StorageCommand,
}

#[derive(Debug, Subcommand, Clone)]
pub enum StorageCommand {
    /// Copy a stopped workspace's database and files to PostgreSQL and the configured file
    /// store. The destination comes from the config file and environment, like `vizier run`.
    Migrate {
        /// The workspace to copy from (it must not be running)
        #[arg(long, value_name = "DIR")]
        workspace: PathBuf,
        /// Config file for the destination deployment
        #[arg(short, long, value_name = "FILE")]
        config: Option<PathBuf>,
        /// Destination database (must be postgres); env VIZIER_STORAGE
        #[arg(long, value_name = "STORAGE")]
        storage: Option<StorageKind>,
        /// Destination file store; env VIZIER_OBJECTS
        #[arg(long, value_name = "OBJECTS")]
        objects: Option<ObjectsKind>,
        /// Empty the destination's tables first, even after a completed copy
        #[arg(long)]
        reset_destination: bool,
    },
}

pub fn storage(args: StorageArgs) -> Result<()> {
    match args.command {
        StorageCommand::Migrate {
            workspace,
            config,
            storage,
            objects,
            reset_destination,
        } => {
            let mut dest = super::config_resolve::resolve(config)?;
            let storage = match storage {
                Some(kind) => Some(kind),
                None => env_value("VIZIER_STORAGE")?,
            };
            let objects = match objects {
                Some(kind) => Some(kind),
                None => env_value("VIZIER_OBJECTS")?,
            };
            let overrides = RunOverrides {
                storage: storage.map(|kind| kind.resolve(&dest.storage)),
                objects: objects.map(|kind| kind.resolve(&dest.objects)),
                ..Default::default()
            };
            dest.apply_overrides(&overrides);
            let options = MigrateOptions {
                source_workspace: workspace,
                reset_destination,
            };

            let runtime = tokio::runtime::Builder::new_multi_thread()
                .enable_all()
                .build()?;
            match runtime.block_on(migrate(&dest, &options)) {
                Ok(MigrateOutcome::Done(report)) => {
                    println!("{}", serde_json::to_string_pretty(&report)?);
                    Ok(())
                }
                Ok(MigrateOutcome::Refused(why)) => {
                    eprintln!("refused: {why}");
                    std::process::exit(2);
                }
                Err(e) => {
                    eprintln!("migration failed (rerun to resume): {e:#}");
                    std::process::exit(1);
                }
            }
        }
    }
}
