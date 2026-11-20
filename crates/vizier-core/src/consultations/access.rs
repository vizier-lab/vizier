//! Who may be consulted (`specs/064-consult-agents` research D5, FR-010 to FR-013, FR-011a).
//!
//! A consultation uses the consultee's tools, integrations, memory and provider budget, and
//! shows it what the conversation is about. So an agent may consult only agents the person it is
//! working for may chat with (`access::can_view_agent`): the person in a WebUI or project chat,
//! the consulting agent's owner in a scheduled run, and, for a consultation that consults
//! further, the person the first consultation was made for. Discord and Telegram identities are
//! self-declared until vizier-lab/vizier#116, so nothing is offered there.
//!
//! An unknown agent and one the person may not use are refused with the same answer, so the
//! refusal never says whether an agent exists.

use crate::{
    access::can_view_agent,
    agents::subagents::OnBehalf,
    agents::tools::ToolContext,
    auth::{AuthMethod, Principal},
    schema::{AgentConfig, AgentId, VizierChannelId},
    storage::VizierStorage,
};

/// Whether the consultation tools are offered in this turn (FR-011, FR-011a, FR-015, FR-016):
/// below the configured depth, not inside a subagent worker, not in a dream, and not in a
/// Discord or Telegram conversation or anything started from one (a detached script runs in its
/// conversation's session, so the same test covers it).
pub fn may_consult(ctx: &ToolContext, max_depth: u8) -> bool {
    ctx.consultation_chain.len() < max_depth as usize
        && ctx.subagent_depth == 0
        && !matches!(
            ctx.session.1,
            VizierChannelId::Dream(..) | VizierChannelId::Subagent | VizierChannelId::Workspace(_)
        )
        && ctx.session.1.platform().is_none()
}

/// Why a consultation was refused, as data the agent can act on (FR-009).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Refusal {
    /// No such agent, or the person may not use it: one answer for both.
    UnknownAgent(String),
    SelfConsult,
    /// The agent is already consulting, above this turn.
    InChain(String),
    /// No person to act for: an agent without an owner, outside a person's chat.
    NoOwner,
    /// The agent exists and is allowed, but cannot take requests now.
    Unavailable { agent: String, reason: String },
}

impl Refusal {
    pub fn to_json(&self) -> serde_json::Value {
        let (status, message) = match self {
            Self::UnknownAgent(agent) => (
                "unknown_agent",
                format!("there is no agent {agent} you may consult; list_consultable_agents lists them"),
            ),
            Self::SelfConsult => (
                "self",
                "you cannot consult yourself; for help with your own work, hire a subagent".into(),
            ),
            Self::InChain(agent) => (
                "in_chain",
                format!("{agent} is already consulting, above this conversation; it cannot be consulted from here"),
            ),
            Self::NoOwner => (
                "no_owner",
                "this agent has no owner, and no person is in this conversation, so there is no one to consult on behalf of".into(),
            ),
            Self::Unavailable { agent, reason } => {
                ("unavailable", format!("{agent} cannot take requests now: {reason}"))
            }
        };
        serde_json::json!({ "status": status, "message": message })
    }
}

/// The person a turn consults for, resolved to their current role (so a revoked permission is
/// felt at once).
#[derive(Debug, Clone)]
pub struct Consulter {
    pub principal: Principal,
}

impl Consulter {
    /// Who the consultee is told the consultation is for: the person, unless the turn has none
    /// of its own (a scheduled run is on the owner's behalf, but no person is involved).
    pub fn on_behalf(&self, ctx: &ToolContext) -> OnBehalf {
        let person_present = ctx.session.1.web_user().is_some()
            || matches!(ctx.session.1, VizierChannelId::Consultation(_));
        OnBehalf {
            user_id: self.principal.user_id.clone(),
            display: person_present.then(|| self.principal.username.clone()),
        }
    }
}

/// Whose access decides in this turn (D5): the person in a web or project chat; for a
/// consultation consulting further, the person the first one was made for; otherwise the
/// consulting agent's owner.
pub async fn consulter(
    storage: &VizierStorage,
    ctx: &ToolContext,
) -> crate::Result<Result<Consulter, Refusal>> {
    use crate::storage::{agent::AgentStorage, subagent::SubagentStorage, user::UserStorage};
    let err = |what: &str, e: anyhow::Error| crate::error::VizierError(format!("could not read {what}: {e}"));
    let user = if let Some(username) = ctx.session.1.web_user() {
        storage.get_user(username).await.map_err(|e| err("the person", e))?
    } else if let (VizierChannelId::Consultation(_), Some(topic)) =
        (&ctx.session.1, ctx.session.2.as_deref())
    {
        let row = storage
            .subagent_by_worker(topic)
            .await
            .map_err(|e| err("the consultation", e))?;
        match row.and_then(|row| row.principal_user_id) {
            Some(id) => storage.get_user_by_id(&id).await.map_err(|e| err("the person", e))?,
            None => None,
        }
    } else {
        let owner = storage
            .get_agent(&ctx.session.0)
            .await
            .map_err(|e| err("the agent", e))?
            .and_then(|config| config.owner_id);
        match owner {
            Some(id) => storage.get_user_by_id(&id).await.map_err(|e| err("the owner", e))?,
            None => None,
        }
    };
    let Some(user) = user else {
        return Ok(Err(Refusal::NoOwner));
    };
    let Some(role) = storage.get_role(&user.role_id).await.map_err(|e| err("the role", e))? else {
        return Ok(Err(Refusal::NoOwner));
    };
    Ok(Ok(Consulter {
        principal: Principal {
            user_id: user.user_id,
            username: user.username,
            permissions: role.permissions.clone(),
            role,
            // Not read by the view rule; the person is acting through an agent, not signing in.
            method: AuthMethod::Jwt,
        },
    }))
}

/// Whether `agent_id` may be consulted from this turn for `consulter` (FR-010, FR-015).
pub async fn consultable(
    storage: &VizierStorage,
    ctx: &ToolContext,
    consulter: &Consulter,
    agent_id: &str,
) -> crate::Result<Result<AgentConfig, Refusal>> {
    use crate::storage::agent::AgentStorage;
    if agent_id == ctx.session.0 {
        return Ok(Err(Refusal::SelfConsult));
    }
    if ctx.consultation_chain.iter().any(|above| above == agent_id) {
        return Ok(Err(Refusal::InChain(agent_id.to_string())));
    }
    let config = storage
        .get_agent(agent_id)
        .await
        .map_err(|e| crate::error::VizierError(format!("could not read agent {agent_id}: {e}")))?;
    Ok(match config {
        Some(config) if can_view_agent(&consulter.principal, &config) => Ok(config),
        _ => Err(Refusal::UnknownAgent(agent_id.to_string())),
    })
}

/// Every agent this turn may consult for `consulter`, by id (FR-012): never the caller, never
/// one in the chain.
pub async fn consultable_agents(
    storage: &VizierStorage,
    ctx: &ToolContext,
    consulter: &Consulter,
) -> crate::Result<Vec<(AgentId, AgentConfig)>> {
    use crate::storage::agent::AgentStorage;
    let mut agents = storage
        .list_agents()
        .await
        .map_err(|e| crate::error::VizierError(format!("could not list agents: {e}")))?;
    agents.retain(|(id, config)| {
        *id != ctx.session.0
            && !ctx.consultation_chain.contains(id)
            && can_view_agent(&consulter.principal, config)
    });
    agents.sort_by(|a, b| a.0.cmp(&b.0));
    Ok(agents)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::schema::{DreamStage, VizierSession};

    fn ctx(channel: VizierChannelId, chain: Vec<&str>, subagent_depth: u8) -> ToolContext {
        let mut ctx = crate::agents::tools::tests_support::context(VizierSession(
            "alpha".into(),
            channel,
            Some("t".into()),
        ));
        ctx.consultation_chain = chain.into_iter().map(str::to_string).collect();
        ctx.subagent_depth = subagent_depth;
        ctx
    }

    fn web() -> VizierChannelId {
        VizierChannelId::HTTP("dani".into(), "chat".into())
    }

    #[test]
    fn offered_in_a_person_s_chat_and_a_scheduled_run() {
        assert!(may_consult(&ctx(web(), vec![], 0), 1));
        assert!(may_consult(&ctx(VizierChannelId::Project("dani".into(), "p".into()), vec![], 0), 1));
        assert!(may_consult(&ctx(VizierChannelId::Task("brief".into(), crate::time::now()), vec![], 0), 1));
    }

    #[test]
    fn not_offered_deeper_than_the_setting_or_in_a_worker_or_a_dream() {
        let consultee = VizierChannelId::Consultation("beta".into());
        assert!(!may_consult(&ctx(consultee.clone(), vec!["beta"], 0), 1), "a consultee, by default");
        assert!(may_consult(&ctx(consultee, vec!["beta"], 0), 2), "a raised setting");
        assert!(!may_consult(&ctx(VizierChannelId::Subagent, vec![], 1), 1));
        assert!(!may_consult(&ctx(web(), vec![], 1), 1), "a worker's detached script");
        let dream = VizierChannelId::Dream(
            Box::new(VizierSession("alpha".into(), web(), None)),
            DreamStage::Extraction,
        );
        assert!(!may_consult(&ctx(dream, vec![], 0), 1));
    }

    /// FR-011a: deferred until verified channel identities (vizier-lab/vizier#116).
    #[test]
    fn not_offered_on_discord_or_telegram() {
        assert!(!may_consult(&ctx(VizierChannelId::DiscordChanel(1), vec![], 0), 1));
        assert!(!may_consult(&ctx(VizierChannelId::TelegramChannel(-1), vec![], 0), 1));
    }

    /// An agent `owner` owns (or nobody, with `None`), on the offline provider.
    fn agent(name: &str, owner: Option<&str>) -> AgentConfig {
        let mut config: AgentConfig = serde_json::from_value(serde_json::json!({
            "name": name,
            "provider": "dummyplug",
            "model": "dummyplug",
            "thinking_depth": 1,
            "tools": { "timeout": "30s" },
            "silent_read_initiative_chance": 0.0,
            "prompt_timeout": "30s",
            "heartbeat_interval": "60s",
        }))
        .unwrap();
        config.owner_id = owner.map(str::to_string);
        config
    }

    /// FR-010, FR-011, FR-013 against real storage: whose access decides in each kind of turn,
    /// and which agents that person may consult.
    #[tokio::test]
    async fn the_person_decides_and_the_owner_stands_in_for_a_scheduled_run() {
        use crate::storage::{agent::AgentStorage, subagent::SubagentStorage, user::UserStorage};
        let dir = tempfile::tempdir().unwrap();
        let deps = crate::dependencies::VizierDependencies::new(crate::config::VizierConfig {
            workspace: dir.path().to_string_lossy().to_string(),
            ..Default::default()
        })
        .await
        .unwrap();
        let storage = &deps.storage;
        let plain = storage.create_role("plain", vec![], false).await.unwrap();
        let owner = storage.create_user("owner", "", &plain.role_id).await.unwrap();
        let viewer = storage.create_user("viewer", "", &plain.role_id).await.unwrap();
        storage.create_agent("alpha", &agent("Alpha", Some(&owner.user_id))).await.unwrap();
        storage.create_agent("beta", &agent("Beta", Some(&owner.user_id))).await.unwrap();
        storage.create_agent("orphan", &agent("Orphan", None)).await.unwrap();
        let mut shared = agent("Gamma", Some(&owner.user_id));
        shared.shared_to = vec![viewer.user_id.clone()];
        storage.create_agent("gamma", &shared).await.unwrap();

        // A person's chat: that person's view right.
        let chat = ctx(VizierChannelId::HTTP("viewer".into(), "c".into()), vec![], 0);
        let who = consulter(storage, &chat).await.unwrap().unwrap();
        assert_eq!(who.principal.user_id, viewer.user_id);
        assert_eq!(who.on_behalf(&chat).display.as_deref(), Some("viewer"));
        assert_eq!(consultable(storage, &chat, &who, "beta").await.unwrap().unwrap_err(), Refusal::UnknownAgent("beta".into()));
        assert!(consultable(storage, &chat, &who, "gamma").await.unwrap().is_ok());
        let listed: Vec<String> = consultable_agents(storage, &chat, &who).await.unwrap().into_iter().map(|(id, _)| id).collect();
        assert_eq!(listed, vec!["gamma".to_string()]);

        // A scheduled run: the consulting agent's owner, and no person named to the consultee.
        let run = ctx(VizierChannelId::Task("brief".into(), crate::time::now()), vec![], 0);
        let who = consulter(storage, &run).await.unwrap().unwrap();
        assert_eq!(who.principal.user_id, owner.user_id);
        assert_eq!(who.on_behalf(&run).display, None);
        assert!(consultable(storage, &run, &who, "beta").await.unwrap().is_ok());
        assert_eq!(consultable(storage, &run, &who, "alpha").await.unwrap().unwrap_err(), Refusal::SelfConsult);

        // An agent with no owner, outside a person's chat, has no one to consult for.
        let mut orphan = run.clone();
        orphan.session.0 = "orphan".into();
        assert_eq!(consulter(storage, &orphan).await.unwrap().unwrap_err(), Refusal::NoOwner);

        // A consultation consulting further (with a raised depth) keeps the first one's person.
        let mut row = crate::schema::subagent::tests_support::row(crate::schema::SubagentState::Running);
        row.id = "c_1".into();
        row.agent_id = "alpha".into();
        row.consultee_id = Some("beta".into());
        row.principal_user_id = Some(viewer.user_id.clone());
        row.worker_topic = "consulted".into();
        storage.open_subagents(vec![row]).await.unwrap();
        let mut inner = ctx(VizierChannelId::Consultation("alpha".into()), vec!["alpha"], 0);
        inner.session = crate::schema::VizierSession("beta".into(), VizierChannelId::Consultation("alpha".into()), Some("consulted".into()));
        let who = consulter(storage, &inner).await.unwrap().unwrap();
        assert_eq!(who.principal.user_id, viewer.user_id);
        assert_eq!(consultable(storage, &inner, &who, "alpha").await.unwrap().unwrap_err(), Refusal::InChain("alpha".into()));
    }

    #[test]
    fn unknown_and_not_allowed_read_the_same() {
        let a = Refusal::UnknownAgent("beta".into()).to_json();
        assert_eq!(a["status"], "unknown_agent");
        assert!(a["message"].as_str().unwrap().contains("list_consultable_agents"));
        assert_eq!(Refusal::SelfConsult.to_json()["status"], "self");
        assert_eq!(Refusal::InChain("beta".into()).to_json()["status"], "in_chain");
        assert_eq!(Refusal::NoOwner.to_json()["status"], "no_owner");
    }
}
