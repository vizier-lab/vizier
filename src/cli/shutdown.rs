use std::path::PathBuf;

use anyhow::{Result, bail};
use clap::Args;

use crate::{command::VizierCommandClient, config::VizierConfig};

#[derive(Debug, Args, Clone)]
pub struct ShutdownArgs {
    #[arg(
        short,
        long,
        value_name = "PATH",
        value_hint = clap::ValueHint::DirPath,
        help = "path to .vizier.yaml or .vizier/config.yaml (optional, uses defaults if omitted)",
    )]
    config: Option<std::path::PathBuf>,
}

pub fn shutdown(args: ShutdownArgs) -> Result<()> {
    let config = VizierConfig::load(args.config.clone())?;

    let sock_path = PathBuf::from(&config.workspace)
        .join(".runtime")
        .join(".vizier.sock");
    if !sock_path.exists() {
        bail!("no vizier daemon running at {}", sock_path.display());
    }

    tokio::runtime::Runtime::new()?.block_on(async move {
        let connection = VizierCommandClient::new(config).unwrap();
        if let Ok(res) = connection
            .send_command(crate::schema::CommandRequest::Exit)
            .await
        {
            tracing::info!("{}", res);
        }
    });

    Ok(())
}
