//! A headless host: start the engine on a workspace you choose, create an agent, send it one
//! message, print the reply, shut down. No HTTP server, no CLI.
//!
//! ```sh
//! # the dummyplug test model, from the `dummyplug` submodule (or set DUMMYPLUG_URL)
//! (cd dummyplug && npm start) &
//! cargo run --example headless -- --workspace /tmp/vizier-headless
//! ```

use std::{path::PathBuf, time::Duration};

use vizier_core::{
    config::{VizierConfig, provider::ProviderId},
    runtime::{RuntimeOptions, VizierRuntime},
    schema::{
        AgentCommand, AgentCommandResult, AgentConfig, AgentToolsConfig, VizierChannelId,
        VizierRequest, VizierRequestContent, VizierResponse, VizierResponseContent,
        VizierSession,
    },
};

#[tokio::main]
async fn main() -> vizier_core::Result<()> {
    let workspace = workspace_arg()?;
    let config = VizierConfig::with_workspace(workspace);
    let runtime = VizierRuntime::start(RuntimeOptions { config }).await?;

    let mut events = runtime.events();
    tokio::spawn(async move {
        while let Ok(event) = events.recv().await {
            println!("event: {event:?}");
        }
    });
    let mut session_events = runtime.session_events();
    tokio::spawn(async move {
        while let Ok(event) = session_events.recv().await {
            println!("session event: {:?}", event.session);
        }
    });

    let (resp, created) = tokio::sync::oneshot::channel();
    runtime
        .transport()
        .send_agent_command(AgentCommand::Create {
            agent_id: "headless".into(),
            config: agent_config(),
            resp,
        })
        .await
        .map_err(err)?;
    match created.await.map_err(err)? {
        // An agent that already exists from a previous run is fine.
        AgentCommandResult::Ok(_) => println!("agent created"),
        AgentCommandResult::Error(e) => println!("agent not created: {e}"),
    }

    let session = VizierSession(
        "headless".into(),
        VizierChannelId::HTTP("headless".into(), "example".into()),
        Some("hello".into()),
    );
    let (tx, rx) = flume::unbounded::<VizierResponse>();
    runtime
        .transport()
        .send_request(
            session,
            VizierRequest {
                timestamp: chrono::Utc::now(),
                user: "headless".into(),
                content: VizierRequestContent::Chat("tools".into()),
                metadata: serde_json::json!({}),
                ..Default::default()
            },
            Some(tx),
        )
        .await
        .map_err(err)?;
    while let Ok(frame) = rx.recv_async().await {
        match frame.content {
            VizierResponseContent::Message { content, .. } => {
                println!("reply:\n{content}");
                break;
            }
            VizierResponseContent::Error { message, .. } => {
                println!("the turn failed: {message}");
                break;
            }
            other => println!("frame: {other:?}"),
        }
    }

    runtime.shutdown().await
}

fn workspace_arg() -> vizier_core::Result<PathBuf> {
    let mut args = std::env::args().skip(1);
    while let Some(arg) = args.next() {
        if arg == "--workspace" {
            return args
                .next()
                .map(PathBuf::from)
                .ok_or_else(|| vizier_core::error::VizierError("--workspace needs a path".into()));
        }
    }
    Err(vizier_core::error::VizierError(
        "usage: headless --workspace <path>".into(),
    ))
}

/// A `dummyplug` agent: an offline test model, so the example needs no API key.
fn agent_config() -> AgentConfig {
    AgentConfig {
        name: "Headless".into(),
        owner_id: None,
        shared_to: vec![],
        system_prompt: None,
        description: None,
        provider: ProviderId::DUMMYPLUG,
        model: "dummyplug".into(),
        thinking_depth: 1,
        checkpoint_threshold: 0.8,
        tools: AgentToolsConfig {
            timeout: Duration::from_secs(30 * 60).into(),
            ..Default::default()
        },
        silent_read_initiative_chance: 0.0,
        max_tokens: None,
        context_window: None,
        include_documents: None,
        prompt_timeout: Duration::from_secs(60 * 60).into(),
        documents: vec![],
        heartbeat_interval: Duration::from_secs(30 * 60).into(),
        core: None,
        dream_enabled: false,
        dream_schedule: None,
        dream_provider: None,
        dream_model: None,
        avatar_url: None,
        embedding: None,
        indexer: None,
        chunking: Default::default(),
        auto_context: Default::default(),
        completion_retry: Default::default(),
    }
}

fn err(e: impl std::fmt::Display) -> vizier_core::error::VizierError {
    vizier_core::error::VizierError(e.to_string())
}
