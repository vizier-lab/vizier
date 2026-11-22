//! The gateway side, tested against a fake seam and with **no agent host running**
//! (`specs/048-agent-service-seam`, FR-016).
//!
//! The point of the seam is that the gateway side never needs a running agent host to be
//! tested: it reaches an agent through [`AgentsClient`], and a fake answers. Nothing here
//! starts an agent, a supervisor or a bus subscriber for one.

use std::sync::Arc;

use vizier_core::agents_client::{AgentUnavailable, AgentUnavailableReason, FakeAgentsClient};
use vizier_core::auth::{AuthMethod, Principal};
use vizier_core::config::VizierConfig;
use vizier_core::dependencies::VizierDependencies;
use vizier_core::schema::{AgentHealthStatus, AgentRunState, AgentSummary};
use vizier_core::service::{Services, agents as agents_service, error::ServiceError};
use vizier_core::storage::{agent::AgentStorage, user::Role};

/// An agent's configuration, built the way the engine's own tests build one.
fn config() -> vizier_core::schema::AgentConfig {
    serde_json::from_value(serde_json::json!({
        "name": "alpha", "owner_id": null, "system_prompt": null, "description": null,
        "provider": "dummyplug", "model": "dummyplug", "thinking_depth": 1,
        "tools": {"timeout": "30s"}, "silent_read_initiative_chance": 0.0,
        "prompt_timeout": "1m", "heartbeat_interval": "1m"
    }))
    .unwrap()
}

/// The gateway's dependencies, with the seam replaced by a fake.
async fn deps_with(fake: &FakeAgentsClient) -> (VizierDependencies, tempfile::TempDir) {
    let dir = tempfile::tempdir().unwrap();
    let config = VizierConfig {
        workspace: dir.path().to_string_lossy().to_string(),
        ..Default::default()
    };
    let mut deps = VizierDependencies::new(config).await.unwrap();
    deps.agents = Arc::new(fake.clone());
    (deps, dir)
}

/// The person asking.
fn who() -> Principal {
    Principal {
        user_id: "u1".into(),
        username: "tester".into(),
        role: Role {
            role_id: "r1".into(),
            name: "tester".into(),
            permissions: vec!["all_agents:view".into()],
            is_system: true,
            created_at: vizier_core::time::now(),
        },
        permissions: vec!["all_agents:view".into()],
        method: AuthMethod::Jwt,
    }
}

fn health(agent_id: &str, alive: bool) -> AgentHealthStatus {
    AgentHealthStatus {
        agent_id: agent_id.into(),
        alive,
        live: false,
        state: if alive { AgentRunState::Running } else { AgentRunState::Stopped },
        last_error: None,
        failures: 0,
        next_retry_at: None,
        channels: Vec::new(),
        model: None,
        warnings: Vec::new(),
        session_failure: None,
    }
}

fn summary(agent_id: &str) -> AgentSummary {
    AgentSummary {
        agent_id: agent_id.into(),
        name: agent_id.into(),
        description: None,
        avatar_url: None,
        owner_username: None,
        owner_id: None,
        shared_to: Vec::new(),
    }
}

/// The gateway reaches the agent through the seam, and the answer reaches the caller.
#[tokio::test]
async fn the_gateway_asks_the_seam_and_the_answer_reaches_the_caller() {
    let fake = FakeAgentsClient::new();
    fake.set_health("alpha", health("alpha", true));
    let (deps, _dir) = deps_with(&fake).await;
    deps.storage.create_agent("alpha", &config()).await.unwrap();

    let services = Services::new(deps);
    let reply = agents_service::ping(&services, &who(), "alpha").await.unwrap();
    assert!(reply.alive, "the fake's answer reached the caller");

    // The fake received the agent the gateway named.
    let calls = fake.calls();
    assert_eq!(calls.len(), 1);
    assert_eq!(calls[0].agent_id(), "alpha");
}

/// An agent that cannot be reached reaches the person as the error it has always been.
#[tokio::test]
async fn an_unavailable_answer_reaches_the_person_unchanged() {
    let fake = FakeAgentsClient::new();
    fake.fail_with(
        AgentUnavailable::new("alpha", AgentUnavailableReason::NoOwner).message(),
    );
    let (deps, _dir) = deps_with(&fake).await;
    deps.storage.create_agent("alpha", &config()).await.unwrap();

    let services = Services::new(deps);
    let error = agents_service::ping(&services, &who(), "alpha")
        .await
        .expect_err("an unavailable agent is an error");

    // The status and the kind are the ones the person reads today.
    assert_eq!(error.status(), 503);
    assert!(matches!(error, ServiceError::Unavailable(_)), "{error:?}");
    // The reason survives to the caller, so an operator can tell the two apart.
    let message = error.message().to_string();
    let failure = AgentUnavailable::parse(&message).expect("the reason survived");
    assert_eq!(failure.reason, AgentUnavailableReason::NoOwner);
    assert_eq!(failure.agent_id, "alpha");
}

/// A create goes through the seam too, and the summary comes back.
#[tokio::test]
async fn a_create_goes_through_the_seam() {
    let fake = FakeAgentsClient::new();
    fake.set_summary("alpha", summary("alpha"));
    let (deps, _dir) = deps_with(&fake).await;

    let services = Services::new(deps);
    let created = agents_service::create(
        &services,
        &who(),
        vizier_core::service::agents::CreateAgentRequest {
            agent_id: "alpha".into(),
            name: "alpha".into(),
            description: None,
            provider: "dummyplug".parse().unwrap(),
            model: "dummyplug".into(),
            system_prompt: None,
            thinking_depth: None,
            checkpoint_threshold: None,
            max_tokens: None,
            context_window: None,
            tools: None,
            prompt_timeout: None,
            dream_enabled: None,
            dream_schedule: None,
            dream_provider: None,
            dream_model: None,
            avatar_url: None,
            silent_read_initiative_chance: None,
            embedding: None,
            indexer: None,
        },
    )
    .await
    .unwrap();

    assert_eq!(created.summary.agent_id, "alpha");
    let calls = fake.calls();
    assert_eq!(calls.len(), 1);
    assert_eq!(calls[0].agent_id(), "alpha");
}

/// The seam is held as a trait object, so a fake can stand in for it.
#[tokio::test]
async fn the_seam_is_a_trait_object() {
    let fake = FakeAgentsClient::new();
    let (deps, _dir) = deps_with(&fake).await;
    // The dependencies hold the fake as the trait, not as the concrete type, and the fake
    // behind it is the one the test holds.
    let seam: Arc<dyn vizier_core::agents_client::AgentsClient> = deps.agents.clone();
    seam.health("alpha").await.unwrap();
    assert_eq!(fake.calls().len(), 1, "the fake behind the trait is the test's fake");
}

/// T041 — the switch that made the whole feature unreachable.
///
/// These go through the same two steps the API goes through: the request body becomes a config
/// (`CreateAgentRequest::into_config`, which is what `PUT /agents/{id}` calls), the config is
/// stored, and `detail` — what the WebUI reads to decide whether to draw the pill — is asked what
/// it sees. A fake agents client is enough because nothing here starts a session.
mod live_voice_switch {
    use super::*;
    use vizier_core::service::agents::CreateAgentRequest;

    fn request(tools: serde_json::Value) -> CreateAgentRequest {
        serde_json::from_value(serde_json::json!({
            "agent_id": "helper",
            "name": "helper",
            "provider": "dummyplug",
            "model": "dummyplug",
            "tools": tools,
        }))
        .expect("a request with tools")
    }

    /// The detail the WebUI reads. The config is written straight into the store.
    async fn detail_after(tools: serde_json::Value) -> vizier_core::service::agents::AgentDetail {
        let fake = FakeAgentsClient::default();
        let (deps, _dir) = deps_with(&fake).await;
        let config = request(tools).into_config();
        deps.storage.create_agent("helper", &config).await.unwrap();
        let services = Services::new(deps);
        agents_service::detail(&services, &who(), "helper")
            .await
            .expect("the agent is there")
    }

    // Spec 055: no switch of its own. An agent that can hear (stt on) and speak (tts on) offers a
    // call; the `live_voice` block's `enabled` flag is not read.

    #[tokio::test]
    async fn an_agent_that_can_hear_and_speak_offers_a_call() {
        let detail = detail_after(serde_json::json!({ "stt": true, "tts": true })).await;
        assert!(detail.live_voice, "both halves on is all a call needs");
        assert_eq!(detail.live_voice_missing, None);
    }

    #[tokio::test]
    async fn an_agent_with_neither_names_the_recogniser_first() {
        let detail = detail_after(serde_json::json!({ "fetch": true })).await;
        assert!(!detail.live_voice);
        assert_eq!(detail.live_voice_missing, Some("recognizer"));
    }

    #[tokio::test]
    async fn a_missing_voice_is_named() {
        let detail = detail_after(serde_json::json!({ "stt": true })).await;
        assert!(!detail.live_voice, "half a call is not a call");
        assert_eq!(detail.live_voice_missing, Some("voice"));
    }

    #[tokio::test]
    async fn the_old_switch_neither_adds_nor_hides_a_call() {
        let on = detail_after(serde_json::json!({ "live_voice": { "enabled": true } })).await;
        assert!(!on.live_voice, "a stored switch alone offers nothing");
        let off = detail_after(serde_json::json!({
            "stt": true, "tts": true, "live_voice": { "enabled": false }
        }))
        .await;
        assert!(off.live_voice, "and an old `false` does not hide a call");
    }

    #[tokio::test]
    async fn the_key_travels_as_null_rather_than_disappearing() {
        // `AgentDetail` has no `skip_serializing_if`, so the client can rely on the key existing.
        let detail = detail_after(serde_json::json!({ "stt": true, "tts": true })).await;
        let json = serde_json::to_value(&detail).expect("the detail serialises");
        assert!(json.get("live_voice_missing").is_some(), "the key is always present: {json}");
        assert_eq!(json["live_voice_missing"], serde_json::Value::Null);
    }
}
