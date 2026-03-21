use std::{env::current_dir, path::PathBuf, str::FromStr, sync::Arc, time::Duration};

use anyhow::Result;
use chrono::Utc;
use clap::Args;
use indicatif::{ProgressBar, ProgressStyle};
use tokio::{io, net::UnixStream};

use crate::{
    config::VizierConfig,
    schema::{SessionId, VizierRequest, VizierResponse, VizierSession},
    utils::format_thinking,
};

#[derive(Debug, Args, Clone)]
pub struct TuiArgs {
    agent_id: String,
    session_id: Option<String>,

    #[arg(
        short,
        long,
        value_name = "PATH",
        value_hint = clap::ValueHint::DirPath,
        help = "path to .vizier.yaml config file",
    )]
    config: Option<std::path::PathBuf>,
}

pub async fn tui(args: TuiArgs) -> Result<()> {
    let config = VizierConfig::load(args.config)?;

    let bind_path = PathBuf::from_str("/tmp/vizier.sock")?;
    let stream = Arc::new(UnixStream::connect(bind_path).await?);

    let prompt = format!("{}: ", config.primary_user.name.clone());

    // generate random session if not provided
    let session_id = args
        .session_id
        .unwrap_or_else(|| uuid::Uuid::new_v4().to_string());

    log::info!("connected to socket session {}", session_id.clone());

    let session = VizierSession(args.agent_id.clone(), SessionId::Socket(session_id.clone()));
    let agent_name = &config.agents.get(&args.agent_id.clone()).unwrap().name;
    while let Ok(text) = inquire::Text::new(&prompt)
        .with_placeholder("ask anything here...")
        .prompt()
    {
        if text == "/exit".to_string() {
            break;
        }

        let metadata = serde_json::json!({
            "sent_at": Utc::now().to_string(),
            "directory": current_dir().unwrap().to_str().unwrap(),
        });

        // try write
        let req = (
            session.clone(),
            VizierRequest {
                user: config.primary_user.name.clone(),
                content: text.clone(),
                metadata,
                ..Default::default()
            },
        );

        let msg = serde_json::to_vec(&req)?;
        while let Ok(_) = stream.writable().await {
            if let Err(err) = stream.try_write(&msg) {
                if err.kind() == io::ErrorKind::WouldBlock {
                    continue;
                }

                log::error!("{:?}", err);
                break;
            } else {
                break;
            }
        }

        let pb = ProgressBar::new_spinner();
        pb.enable_steady_tick(Duration::from_millis(120));
        pb.set_style(
            ProgressStyle::with_template("{spinner:.blue} {msg}")
                .unwrap()
                .tick_strings(&["⠋", "⠙", "⠹", "⠸", "⠼", "⠴", "⠦", "⠧", "⠇", "⠏"]),
        );
        pb.set_message("thinking...");

        // wait for response
        while let Ok(_) = stream.readable().await {
            let mut msg = vec![0; 8092];
            match stream.try_read(&mut msg) {
                Ok(n) => {
                    let mut msg = msg.clone();
                    msg.truncate(n);
                    let str = String::from_utf8(msg)?;
                    let (res_session, response): (VizierSession, VizierResponse) =
                        serde_json::from_str(&str)?;

                    if res_session != session {
                        continue;
                    }

                    match response {
                        VizierResponse::Message { content, stats: _ } => {
                            pb.finish_and_clear();
                            termimad::print_text(&format!("\n---\n{content}\n---\n",));
                            break;
                        }
                        VizierResponse::Thinking { name, args } => {
                            pb.finish_and_clear();
                            let thinking = format_thinking(&name, &args);
                            termimad::print_text(&format!("\n{} *{thinking}*\n", agent_name));
                            pb.reset();
                        }
                        _ => {}
                    }
                }

                Err(err) => {
                    if err.kind() == io::ErrorKind::WouldBlock {
                        continue;
                    }

                    log::error!("{:?}", err);
                    break;
                }
            }
        }
    }

    Ok(())
}
