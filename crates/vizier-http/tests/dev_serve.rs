//! A development server for browser checks on machines where the `vizier` binary does not build
//! (it is Linux/macOS/Docker; on Windows only the desktop app and these crates do). Ignored: run it
//! by hand, and stop it when done.
//!
//! ```text
//! VIZIER_DEV_WORKSPACE=/tmp/ws VIZIER_DEV_PORT=9911 \
//!   cargo test -p vizier-http --test dev_serve -- --ignored --nocapture
//! ```
//!
//! It is the real runtime and the real router with the embedded WebUI, so a person signs in, sets a
//! provider and makes agents through the WebUI or the API as usual (spec 063 quickstart §7).
//!
//! With `VIZIER_DEV_DUMMYPLUG=<url>` (the dummyplug server, e.g. `http://127.0.0.1:9998/v1`) it also points the `dummyplug` provider
//! there and, once the first person has signed up, gives them an agent `plug` on it with no memory
//! embedding, so no model is downloaded. `VIZIER_DEV_LIVE_VOICE=1` also switches live voice on for it.

use vizier_core::{
    config::VizierConfig,
    runtime::{RuntimeOptions, VizierRuntime},
};

#[tokio::test(flavor = "multi_thread")]
#[ignore = "a server that runs until it is stopped"]
async fn serve_until_stopped() {
    let workspace = std::env::var("VIZIER_DEV_WORKSPACE").expect("set VIZIER_DEV_WORKSPACE to a folder");
    let port: u32 = std::env::var("VIZIER_DEV_PORT").ok().and_then(|p| p.parse().ok()).unwrap_or(9911);
    std::fs::create_dir_all(&workspace).unwrap();
    let mut config = VizierConfig::with_workspace(workspace.into());
    let http = config.channels.http.as_mut().expect("an http channel");
    http.port = port;
    if http.jwt_secret.is_empty() {
        http.jwt_secret = "dev-serve-secret".into();
    }
    let http = http.clone();
    let runtime = VizierRuntime::start(RuntimeOptions { config, ..Default::default() }).await.unwrap();
    if let Ok(url) = std::env::var("VIZIER_DEV_DUMMYPLUG") {
        seed_dummyplug(&runtime, url).await;
    }
    eprintln!("dev server on http://127.0.0.1:{port}");
    vizier_http::serve(&runtime, http).await.unwrap();
}

/// The `dummyplug` provider at `url`, and an agent `plug` for the first person once there is one.
async fn seed_dummyplug(runtime: &VizierRuntime, url: String) {
    use vizier_core::{
        config::provider::ProviderId,
        schema::{
            AgentCommand, AgentCommandResult,
            provider::{ProviderCredentials, ProviderEntry},
        },
        storage::{agent::AgentStorage, provider::ProviderStorage, user::UserStorage},
    };
    let deps = runtime.deps();
    deps.storage
        .upsert_provider(&ProviderEntry {
            id: ProviderId::DUMMYPLUG,
            credentials: ProviderCredentials { base_url: Some(url), ..Default::default() },
        })
        .await
        .unwrap();
    let storage = deps.storage.clone();
    let transport = runtime.transport().clone();
    tokio::spawn(async move {
        loop {
            if storage.get_agent("plug").await.ok().flatten().is_some() {
                return;
            }
            if let Some(owner) = storage.list_users().await.ok().and_then(|users| users.into_iter().next()) {
                let mut config: vizier_core::schema::AgentConfig = serde_json::from_value(serde_json::json!({
                    "name": "Plug", "system_prompt": null, "description": null,
                    "provider": "dummyplug", "model": "dummyplug", "thinking_depth": 1,
                    "tools": {"timeout": "30m"}, "silent_read_initiative_chance": 0.0,
                    "prompt_timeout": "1h", "heartbeat_interval": "30m"
                }))
                .unwrap();
                config.owner_id = Some(owner.user_id.clone());
                config.embedding = None;
                // `VIZIER_DEV_LIVE_VOICE=1`: the agent can hear and speak, so it offers a live voice
                // call (spec 055). Point its Speech-to-Text and Text-to-Speech at real providers in
                // the WebUI for a working call.
                if std::env::var("VIZIER_DEV_LIVE_VOICE").is_ok_and(|v| v == "1") {
                    config.tools.stt.enabled = true;
                    config.tools.tts.enabled = true;
                }
                let (resp, created) = tokio::sync::oneshot::channel();
                transport
                    .send_agent_command(AgentCommand::Create { agent_id: "plug".into(), config, resp })
                    .await
                    .unwrap();
                match created.await {
                    Ok(AgentCommandResult::Ok(_)) => eprintln!("agent plug created for {}", owner.username),
                    Ok(AgentCommandResult::Error(e)) => eprintln!("agent plug: {e}"),
                    Err(_) => eprintln!("agent plug: no answer"),
                }
                return;
            }
            tokio::time::sleep(std::time::Duration::from_millis(500)).await;
        }
    });
}
