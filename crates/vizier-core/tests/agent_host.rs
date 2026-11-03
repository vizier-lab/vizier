//! The agent host, tested on its own (`specs/163-agent-service-seam` FR-016).
//!
//! The host is built from a bus handle, a storage handle and the list of agents it is told
//! to hold. No gateway is running here, and no other role: the host answers for the agents
//! it holds and refuses the ones it does not.

use vizier_core::agents::AgentHost;
use vizier_core::agents_client::{
    AgentUnavailable, AgentUnavailableReason, AgentsClient, LocalAgentsClient, availability,
};
use vizier_core::config::VizierConfig;
use vizier_core::dependencies::VizierDependencies;
use vizier_core::schema::{AgentCommand, AgentHealthStatus, AgentRunState};

/// An agent's configuration, built the way the engine's own tests build one.
fn config(name: &str) -> vizier_core::schema::AgentConfig {
    serde_json::from_value(serde_json::json!({
        "name": name, "owner_id": null, "system_prompt": null, "description": null,
        "provider": "dummyplug", "model": "dummyplug", "thinking_depth": 1,
        "tools": {"timeout": "30s"}, "silent_read_initiative_chance": 0.0,
        "prompt_timeout": "1m", "heartbeat_interval": "1m"
    }))
    .unwrap()
}

/// The dependencies, with one stored agent.
async fn deps_with(agent_id: &str) -> (VizierDependencies, tempfile::TempDir) {
    use vizier_core::storage::agent::AgentStorage;
    let dir = tempfile::tempdir().unwrap();
    let config = VizierConfig {
        workspace: dir.path().to_string_lossy().to_string(),
        ..Default::default()
    };
    let deps = VizierDependencies::new(config).await.unwrap();
    deps.storage.create_agent(agent_id, &config_for(agent_id)).await.unwrap();
    (deps, dir)
}

/// The configuration for `agent_id`.
fn config_for(agent_id: &str) -> vizier_core::schema::AgentConfig {
    config(agent_id)
}

/// The host holds the agent it was told to hold, and answers for it.
#[tokio::test]
async fn the_host_answers_for_the_agent_it_holds() {
    let (deps, _dir) = deps_with("alpha").await;
    let mut host = AgentHost::new(deps.clone(), vec!["alpha".into()]).await.unwrap();
    let running = tokio::spawn(async move { host.run().await });

    // The seam reaches the agent over the bus, exactly as it would in the server.
    let client = LocalAgentsClient::new(deps.transport.clone());
    let health = client.health("alpha").await.expect("the host answers");
    // The answer is about the agent the host holds. Its run state is the supervisor's:
    // this test has no provider, so the agent is still building.
    assert_eq!(health.agent_id, "alpha");
    assert_ne!(health.state, AgentRunState::Stopped, "{health:?}");

    running.abort();
}

/// The host refuses an agent it does not hold: it never builds the agent on the spot.
#[tokio::test]
async fn the_host_refuses_an_agent_it_does_not_hold() {
    use vizier_core::storage::agent::AgentStorage;
    let (deps, _dir) = deps_with("alpha").await;
    // "ghost" was never stored, and the host was never told to hold it.
    let mut host = AgentHost::new(deps.clone(), vec!["alpha".into()]).await.unwrap();
    let running = tokio::spawn(async move { host.run().await });

    let client = LocalAgentsClient::new(deps.transport.clone());
    let error = client
        .health("ghost")
        .await
        .expect_err("an agent the host does not hold is unavailable");
    let failure = AgentUnavailable::parse(&error.0).expect("an availability failure");
    assert_eq!(failure.reason, AgentUnavailableReason::NoOwner);
    assert_eq!(failure.agent_id, "ghost");

    // And it was not built on the spot: nothing was stored for it.
    assert!(
        deps.storage.get_agent("ghost").await.unwrap().is_none(),
        "the host never built the agent it does not hold"
    );

    running.abort();
}

/// The host publishes the health view of the agents it holds, so a gateway can read it.
#[tokio::test]
async fn the_host_publishes_the_health_view_of_the_agent_it_holds() {
    let (deps, _dir) = deps_with("alpha").await;
    let mut host = AgentHost::new(deps.clone(), vec!["alpha".into()]).await.unwrap();
    let running = tokio::spawn(async move { host.run().await });

    // The view is published on take; give the write its turn.
    tokio::time::sleep(std::time::Duration::from_millis(100)).await;
    let view = vizier_core::agents::views::read_health(&deps, "alpha")
        .await
        .expect("the host published the health view");
    assert_eq!(view.agent_id, "alpha");

    running.abort();
}

/// The two availability reasons are told apart by the bus's own failures.
#[test]
fn the_two_reasons_are_told_apart() {
    let no_owner = availability("alpha", vizier_schema::BusError::NoResponders);
    let owner_lost = availability("alpha", vizier_schema::BusError::Timeout);
    let no_owner = AgentUnavailable::parse(&no_owner.0).expect("a reason");
    let owner_lost = AgentUnavailable::parse(&owner_lost.0).expect("a reason");
    assert_eq!(no_owner.reason, AgentUnavailableReason::NoOwner);
    assert_eq!(owner_lost.reason, AgentUnavailableReason::OwnerLost);
}

/// A command about an agent the host does not hold is not answered at all.
#[tokio::test]
async fn a_command_for_an_agent_the_host_does_not_hold_is_not_answered() {
    let (deps, _dir) = deps_with("alpha").await;
    let mut host = AgentHost::new(deps.clone(), vec!["alpha".into()]).await.unwrap();
    let running = tokio::spawn(async move { host.run().await });

    // The bus finds nobody subscribed to the ghost's subject: the host subscribed only for
    // the agent it holds.
    let (tx, rx) = tokio::sync::oneshot::channel();
    let error = deps
        .transport
        .send_agent_command_to(
            "ghost",
            AgentCommand::AgentHealth { agent_id: "ghost".into(), resp: tx },
            std::time::Duration::from_secs(5),
        )
        .await
        .expect_err("nobody is subscribed to a ghost's command subject");
    assert!(
        error.to_string().contains("no responders"),
        "{error}"
    );
    // The slot was never filled.
    assert!(rx.await.is_err());

    running.abort();
}

/// The health the host answers with is the agent's own.
#[tokio::test]
async fn the_health_the_host_answers_carries_that_agents_id() {
    let (deps, _dir) = deps_with("alpha").await;
    let mut host = AgentHost::new(deps.clone(), vec!["alpha".into()]).await.unwrap();
    let running = tokio::spawn(async move { host.run().await });

    let client = LocalAgentsClient::new(deps.transport.clone());
    let health: AgentHealthStatus = client.health("alpha").await.unwrap();
    assert_eq!(health.agent_id, "alpha");
    // The host holds the agent, so it answers with a state for it rather than refusing.
    assert!(matches!(
        health.state,
        AgentRunState::Starting | AgentRunState::Running | AgentRunState::Failed
    ));

    running.abort();
}
