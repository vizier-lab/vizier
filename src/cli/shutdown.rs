use anyhow::Result;
use clap::Args;

use crate::{command::VizierCommandClient, config::VizierConfig};

#[derive(Debug, Args, Clone)]
pub struct ShutdownArgs {
    #[arg(
        short,
        long,
        value_name = "PATH",
        value_hint = clap::ValueHint::DirPath,
        help = "path to .vizier.yaml or .vizier/config.yaml config file",
    )]
    config: Option<std::path::PathBuf>,
}

pub fn shutdown(args: ShutdownArgs) -> Result<()> {
    let config = VizierConfig::load(args.config.clone())?;

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
