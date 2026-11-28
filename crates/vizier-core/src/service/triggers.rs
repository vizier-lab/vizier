//! Triggers for people (`specs/071-triggers/contracts/http-api.md`): create, read, change,
//! approve, delete, and read what happened. Authorization lives here (`triggers::access`), never in
//! a handler.

use serde::{Deserialize, Serialize};
use serde_json::Value;

use super::{ServiceError, Services};
use crate::{
    access::{Caller, can_view_agent},
    agents::tools::scope,
    auth::Principal,
    dependencies::VizierDependencies,
    schema::{
        AgentConfig, OverlapPolicy, Requester, SessionHistory, TriageConfig, Trigger, TriggerEvent,
        TriggerRun,
        trigger::{DEFAULT_HOURLY_LIMIT, DEFAULT_SUMMARY_HOURS},
    },
    storage::{agent::AgentStorage, history::HistoryStorage, trigger::TriggerStorage},
    triggers::{TriggerKind, access, run_session},
};

const DEFAULT_PAGE: usize = 50;
const MAX_PAGE: usize = 200;

fn internal(e: impl std::fmt::Display) -> ServiceError {
    tracing::error!("trigger service error: {e}");
    ServiceError::Internal("Storage error".into())
}

/// A trigger as people read it: secrets masked, with what the page needs to warn.
#[derive(Debug, Serialize, Clone)]
#[cfg_attr(feature = "utoipa", derive(utoipa::ToSchema))]
pub struct TriggerResponse {
    #[serde(flatten)]
    pub trigger: Trigger,
    /// The address a webhook receives requests at, relative to the server.
    pub address_path: Option<String>,
    /// Whether its runs may send messages or wake conversations, which the run limit cannot bound
    /// (FR-016).
    pub allows_messaging: bool,
    /// A monitor's last check, last error and next check, so a quiet monitor is told apart from a
    /// broken one (US5).
    pub monitor: Option<MonitorHealth>,
    /// The newest run, if it ever ran.
    pub latest_run: Option<TriggerRun>,
    /// Runs started in the last hour, which the hourly limit bounds.
    pub runs_last_hour: u32,
    /// Whether it waits for a person: for approval, because something it needs is gone, or
    /// because its checks stopped.
    pub needs_person: bool,
}

/// An agent's triggers in numbers, for the page's heading and its sidebar badge.
#[derive(Debug, Serialize, Clone, Default, PartialEq, Eq)]
#[cfg_attr(feature = "utoipa", derive(utoipa::ToSchema))]
pub struct TriggerSummary {
    pub total: usize,
    /// Those taking events now.
    pub active: usize,
    pub needs_person: usize,
}

#[derive(Debug, Serialize, Clone)]
#[cfg_attr(feature = "utoipa", derive(utoipa::ToSchema))]
pub struct MonitorHealth {
    pub last_check_at: Option<chrono::DateTime<chrono::Local>>,
    pub next_check_at: Option<chrono::DateTime<chrono::Local>>,
    pub last_error: Option<String>,
    pub failures: u32,
    pub failed_at: Option<chrono::DateTime<chrono::Local>>,
}

/// What creating a trigger, or replacing its secret, returns: the secret, this once (FR-020).
#[derive(Debug, Serialize, Clone)]
#[cfg_attr(feature = "utoipa", derive(utoipa::ToSchema))]
pub struct CreatedTrigger {
    pub trigger: TriggerResponse,
    pub secret: Option<String>,
}

/// The fields a person sets. On update, a field left out keeps its value.
#[derive(Debug, Deserialize, Default, Clone)]
#[cfg_attr(feature = "utoipa", derive(utoipa::ToSchema))]
pub struct TriggerInput {
    pub slug: Option<String>,
    pub kind: Option<String>,
    pub title: Option<String>,
    pub instruction: Option<String>,
    #[cfg_attr(feature = "utoipa", schema(value_type = Option<Object>))]
    pub settings: Option<Value>,
    pub is_active: Option<bool>,
    /// `null` is every tool.
    #[serde(default, deserialize_with = "double_option")]
    pub tool_scope: Option<Option<Vec<String>>>,
    pub overlap: Option<OverlapPolicy>,
    /// `null` switches the limit off.
    #[serde(default, deserialize_with = "double_option")]
    pub hourly_limit: Option<Option<u32>>,
    pub cooldown_secs: Option<u32>,
    #[serde(default, deserialize_with = "double_option")]
    pub triage: Option<Option<TriageConfig>>,
    #[serde(default, deserialize_with = "double_option")]
    pub destination: Option<Option<String>>,
    #[serde(default, deserialize_with = "double_option")]
    pub summary_hours: Option<Option<u32>>,
}

/// Tells a field left out (`None`) from one sent as `null` (`Some(None)`).
fn double_option<'de, T, D>(deserializer: D) -> Result<Option<Option<T>>, D::Error>
where
    T: Deserialize<'de>,
    D: serde::Deserializer<'de>,
{
    Ok(Some(Option::deserialize(deserializer)?))
}

/// A kind as the create form reads it.
#[derive(Debug, Serialize, Clone)]
#[cfg_attr(feature = "utoipa", derive(utoipa::ToSchema))]
pub struct KindResponse {
    pub id: String,
    pub listing: crate::integrations::kind::KindListing,
    pub public: bool,
    /// What the runs of a trigger a person creates may use unless they choose; `null` is every
    /// tool.
    pub default_scope: Option<Vec<String>>,
}

pub fn kinds(deps: &VizierDependencies) -> Vec<KindResponse> {
    deps.triggers
        .kinds
        .all()
        .iter()
        .map(|kind| KindResponse {
            id: kind.id().into(),
            listing: kind.listing(),
            public: kind.is_public(),
            default_scope: kind.default_scope(),
        })
        .collect()
}

pub(crate) fn respond(deps: &VizierDependencies, trigger: Trigger) -> TriggerResponse {
    let kind = deps.triggers.kinds.get(&trigger.kind);
    let mut trigger = trigger;
    if let Some(kind) = &kind {
        trigger.settings = kind.redact(&trigger.settings);
    } else {
        trigger.settings = Value::Null;
    }
    TriggerResponse {
        address_path: kind.filter(|k| k.is_public()).map(|_| format!("/api/v1/hooks/{}", trigger.id)),
        allows_messaging: scope::allows_messaging(trigger.tool_scope.as_deref()),
        monitor: None,
        latest_run: None,
        runs_last_hour: 0,
        needs_person: trigger.awaiting_approval || trigger.needs_attention.is_some(),
        trigger,
    }
}

/// [`respond`], with what happened lately and a monitor's health read in.
pub(crate) async fn respond_full(deps: &VizierDependencies, trigger: Trigger) -> TriggerResponse {
    let is_monitor = trigger.kind == "monitor";
    let id = trigger.id.clone();
    let mut response = respond(deps, trigger);
    match deps.storage.list_trigger_runs(&id, None, 1).await {
        Ok(runs) => response.latest_run = runs.into_iter().next(),
        Err(e) => tracing::warn!(trigger = %id, "could not read a trigger's latest run: {e}"),
    }
    let hour_ago = crate::time::now() - chrono::Duration::hours(1);
    match deps.storage.trigger_runs_since(&id, hour_ago).await {
        Ok(n) => response.runs_last_hour = n,
        Err(e) => tracing::warn!(trigger = %id, "could not count a trigger's runs: {e}"),
    }
    if is_monitor && let Ok(record) = deps.storage.monitor_record(&id).await {
        // A monitor someone switched off is not waiting for anyone.
        response.needs_person |= response.trigger.is_active && record.failed_at.is_some();
        response.monitor = Some(MonitorHealth {
            last_check_at: record.last_check_at,
            next_check_at: record.next_check_at,
            last_error: record.last_error,
            failures: record.failures,
            failed_at: record.failed_at,
        });
    }
    response
}

async fn agent(s: &Services, who: &Principal, agent_id: &str) -> Result<AgentConfig, ServiceError> {
    let config = s
        .deps()
        .storage
        .get_agent(agent_id)
        .await
        .map_err(internal)?
        .ok_or_else(|| ServiceError::NotFound("Agent not found".into()))?;
    if !can_view_agent(who, &config) {
        return Err(ServiceError::NotFound("Agent not found".into()));
    }
    Ok(config)
}

async fn viewable(s: &Services, who: &Principal, agent_id: &str) -> Result<AgentConfig, ServiceError> {
    let config = agent(s, who, agent_id).await?;
    if !access::can_view(Caller::from(who), &config) {
        return Err(ServiceError::Forbidden("You may not see this agent's triggers".into()));
    }
    Ok(config)
}

async fn manageable(s: &Services, who: &Principal, agent_id: &str) -> Result<AgentConfig, ServiceError> {
    let config = agent(s, who, agent_id).await?;
    if !access::can_manage(Caller::from(who), &config) {
        return Err(ServiceError::Forbidden("You may not manage this agent's triggers".into()));
    }
    Ok(config)
}

async fn find(s: &Services, agent_id: &str, slug: &str) -> Result<Trigger, ServiceError> {
    s.deps()
        .storage
        .get_trigger(agent_id, slug)
        .await
        .map_err(internal)?
        .ok_or_else(|| ServiceError::NotFound(format!("trigger {slug} not found")))
}

/// A destination must be one of the setting person's own WebUI conversations, named
/// `webui:<topic id>` (FR-046).
async fn check_destination(deps: &VizierDependencies, trigger: &Trigger, username: &str) -> Result<(), ServiceError> {
    use crate::storage::session::SessionStorage;
    let Some(destination) = &trigger.destination else {
        return Ok(());
    };
    let Some(topic) = destination.strip_prefix("webui:") else {
        return Err(ServiceError::invalid("a destination is one of your WebUI conversations: webui:<topic id>"));
    };
    let channel = crate::schema::VizierChannelId::HTTP(username.to_string(), crate::agents::tools::webui_channel().to_string());
    let known = deps
        .storage
        .get_session_detail_by_topic(trigger.agent_id.clone(), channel, Some(topic.to_string()))
        .await
        .map_err(internal)?;
    match known {
        Some(_) => Ok(()),
        None => Err(ServiceError::invalid("that conversation is not one of yours")),
    }
}

/// A kind an addon contributed is usable only while the addon is on the agent (FR-005).
pub(crate) async fn addon_present(deps: &VizierDependencies, agent_id: &str, kind: &str) -> Result<(), ServiceError> {
    use crate::storage::integration::IntegrationStorage;
    let Some(addon) = deps.triggers.kinds.addon_of(kind) else {
        return Ok(());
    };
    let installed = deps
        .storage
        .list_agent_integrations(agent_id)
        .await
        .map_err(internal)?
        .iter()
        .any(|integration| integration.kind == addon);
    match installed {
        true => Ok(()),
        false => Err(ServiceError::invalid(format!("this kind needs the {addon} addon on the agent"))),
    }
}

fn valid_slug(slug: &str) -> bool {
    // `new` is the address of the page that creates one.
    !slug.is_empty()
        && slug != "new"
        && slug.len() <= 64
        && slug.chars().all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-' || c == '_')
}

/// Checks every rule a saved trigger must meet, whoever saves it.
pub(crate) fn check(kind: &dyn TriggerKind, trigger: &Trigger) -> Result<(), ServiceError> {
    if !valid_slug(&trigger.slug) {
        return Err(ServiceError::invalid("the name may use lowercase letters, digits, - and _ (at most 64), and not be 'new'"));
    }
    if trigger.title.trim().is_empty() {
        return Err(ServiceError::invalid("a trigger needs a title"));
    }
    if trigger.instruction.trim().is_empty() {
        return Err(ServiceError::invalid("a trigger needs an instruction"));
    }
    kind.validate(&trigger.settings).map_err(ServiceError::invalid)?;
    if let Some(entries) = &trigger.tool_scope {
        scope::validate(entries).map_err(ServiceError::invalid)?;
    }
    if trigger.hourly_limit == Some(0) {
        return Err(ServiceError::invalid("the run limit must be at least 1 an hour, or switched off"));
    }
    if let Some(triage) = &trigger.triage {
        if triage.criteria.trim().is_empty() {
            return Err(ServiceError::invalid("triage needs criteria"));
        }
        if triage.calls_per_hour == 0 {
            return Err(ServiceError::invalid("triage needs at least 1 call an hour"));
        }
    }
    if trigger.summary_hours == Some(0) {
        return Err(ServiceError::invalid("the summary window must be at least an hour, or switched off"));
    }
    Ok(())
}

/// A person saves triage only with a model (FR-042). One an agent made may wait for its model
/// until it is approved.
fn require_triage_model(trigger: &Trigger) -> Result<(), ServiceError> {
    if !trigger.awaiting_approval && trigger.triage.as_ref().is_some_and(|t| t.model().is_none()) {
        return Err(ServiceError::invalid("choose a model for this trigger's triage"));
    }
    Ok(())
}

pub async fn list(s: &Services, who: &Principal, agent_id: &str) -> Result<Vec<TriggerResponse>, ServiceError> {
    viewable(s, who, agent_id).await?;
    let triggers = s.deps().storage.list_triggers(Some(agent_id)).await.map_err(internal)?;
    let mut out = Vec::with_capacity(triggers.len());
    for trigger in triggers {
        out.push(respond_full(s.deps(), trigger).await);
    }
    Ok(out)
}

pub async fn summary(s: &Services, who: &Principal, agent_id: &str) -> Result<TriggerSummary, ServiceError> {
    viewable(s, who, agent_id).await?;
    let deps = s.deps();
    let triggers = deps.storage.list_triggers(Some(agent_id)).await.map_err(internal)?;
    let mut summary = TriggerSummary { total: triggers.len(), ..Default::default() };
    for trigger in triggers {
        let stopped = trigger.kind == "monitor"
            && deps.storage.monitor_record(&trigger.id).await.map_err(internal)?.failed_at.is_some();
        if trigger.accepts_events() && !stopped {
            summary.active += 1;
        }
        let waits = trigger.awaiting_approval
            || trigger.needs_attention.is_some()
            || (trigger.is_active && stopped);
        if waits {
            summary.needs_person += 1;
        }
    }
    Ok(summary)
}

pub async fn get(s: &Services, who: &Principal, agent_id: &str, slug: &str) -> Result<TriggerResponse, ServiceError> {
    viewable(s, who, agent_id).await?;
    Ok(respond_full(s.deps(), find(s, agent_id, slug).await?).await)
}

pub async fn create(
    s: &Services,
    who: &Principal,
    agent_id: &str,
    input: TriggerInput,
) -> Result<CreatedTrigger, ServiceError> {
    let config = manageable(s, who, agent_id).await?;
    let deps = s.deps();
    let kind_id = input.kind.clone().ok_or_else(|| ServiceError::invalid("a trigger needs a kind"))?;
    let kind = deps
        .triggers
        .kinds
        .get(&kind_id)
        .ok_or_else(|| ServiceError::invalid(format!("there is no trigger kind '{kind_id}'")))?;
    let caller = Caller::from(who);
    if kind.is_public() && !access::can_publish_webhooks(caller, &config) {
        return Err(ServiceError::Forbidden("You may not open public webhooks on this agent".into()));
    }
    addon_present(deps, agent_id, kind.id()).await?;
    let existing = deps.storage.list_triggers(Some(agent_id)).await.map_err(internal)?;
    let max = deps.triggers.limits.max_per_agent;
    if existing.len() >= max {
        return Err(ServiceError::Conflict {
            message: format!("an agent may have at most {max} triggers"),
            code: Some("too_many_triggers".into()),
            data: None,
        });
    }
    let mut settings = input.settings.clone().unwrap_or_else(|| Value::Object(Default::default()));
    let secret = kind.prepare(&mut settings);
    let triage = input.triage.clone().flatten();
    if triage.as_ref().is_some_and(|t| t.model().is_some()) && !access::can_choose_model(caller) {
        return Err(ServiceError::Forbidden("You may not choose a model for triage".into()));
    }
    let trigger = Trigger {
        id: format!("trg_{}", uuid::Uuid::new_v4().simple()),
        agent_id: agent_id.to_string(),
        slug: input.slug.clone().unwrap_or_default(),
        kind: kind_id,
        title: input.title.clone().unwrap_or_default(),
        instruction: input.instruction.clone().unwrap_or_default(),
        settings,
        requester: Requester::User(who.username.clone()),
        delivers_to: Some(who.user_id.clone()),
        is_active: input.is_active.unwrap_or(true),
        awaiting_approval: false,
        needs_attention: None,
        tool_scope: input.tool_scope.clone().unwrap_or_else(|| kind.default_scope()),
        overlap: input.overlap.unwrap_or_default(),
        hourly_limit: input.hourly_limit.unwrap_or(Some(DEFAULT_HOURLY_LIMIT)),
        cooldown_secs: input.cooldown_secs.unwrap_or(0),
        triage,
        destination: input.destination.clone().flatten(),
        summary_hours: input.summary_hours.unwrap_or(Some(DEFAULT_SUMMARY_HOURS)),
        last_summary_at: None,
        created_at: crate::time::now(),
        last_fired_at: None,
    };
    check(kind.as_ref(), &trigger)?;
    require_triage_model(&trigger)?;
    check_destination(deps, &trigger, &who.username).await?;
    if existing.iter().any(|t| t.slug == trigger.slug) {
        return Err(ServiceError::Conflict {
            message: format!("a trigger named '{}' already exists on this agent", trigger.slug),
            code: Some("slug_taken".into()),
            data: None,
        });
    }
    deps.storage.create_trigger(&trigger).await.map_err(internal)?;
    Ok(CreatedTrigger { trigger: respond_full(deps, trigger).await, secret })
}

pub async fn update(
    s: &Services,
    who: &Principal,
    agent_id: &str,
    slug: &str,
    input: TriggerInput,
) -> Result<TriggerResponse, ServiceError> {
    let config = manageable(s, who, agent_id).await?;
    let deps = s.deps();
    let mut trigger = find(s, agent_id, slug).await?;
    let kind = deps
        .triggers
        .kinds
        .get(&trigger.kind)
        .ok_or_else(|| ServiceError::invalid("this trigger's kind is no longer available"))?;
    let caller = Caller::from(who);
    if input.kind.as_ref().is_some_and(|k| *k != trigger.kind) {
        return Err(ServiceError::invalid("a trigger's kind cannot change"));
    }
    if kind.is_public() && input.is_active == Some(true) && !access::can_publish_webhooks(caller, &config) {
        return Err(ServiceError::Forbidden("You may not open public webhooks on this agent".into()));
    }
    let new_triage = input.triage.clone();
    if let Some(Some(triage)) = &new_triage
        && triage.model() != trigger.triage.as_ref().and_then(|t| t.model())
        && triage.model().is_some()
        && !access::can_choose_model(caller)
    {
        return Err(ServiceError::Forbidden("You may not choose a model for triage".into()));
    }
    if let Some(slug) = input.slug.clone() {
        trigger.slug = slug;
    }
    if let Some(title) = input.title.clone() {
        trigger.title = title;
    }
    if let Some(instruction) = input.instruction.clone() {
        trigger.instruction = instruction;
    }
    if let Some(settings) = input.settings.clone() {
        trigger.settings = kind.merge_secrets(&trigger.settings, settings);
    }
    if let Some(active) = input.is_active {
        trigger.is_active = active;
    }
    if let Some(entries) = input.tool_scope.clone() {
        trigger.tool_scope = entries;
    }
    if let Some(overlap) = input.overlap {
        trigger.overlap = overlap;
    }
    if let Some(limit) = input.hourly_limit {
        trigger.hourly_limit = limit;
    }
    if let Some(cooldown) = input.cooldown_secs {
        trigger.cooldown_secs = cooldown;
    }
    let triage_changed = new_triage.is_some();
    if let Some(triage) = new_triage {
        trigger.triage = triage;
        trigger.needs_attention = None;
    }
    if let Some(destination) = input.destination.clone() {
        trigger.destination = destination;
    }
    if let Some(hours) = input.summary_hours {
        trigger.summary_hours = hours;
    }
    check(kind.as_ref(), &trigger)?;
    if triage_changed {
        require_triage_model(&trigger)?;
    }
    if input.destination.is_some() {
        check_destination(deps, &trigger, &who.username).await?;
    }
    if trigger.slug != slug && deps.storage.get_trigger(agent_id, &trigger.slug).await.map_err(internal)?.is_some() {
        return Err(ServiceError::Conflict {
            message: format!("a trigger named '{}' already exists on this agent", trigger.slug),
            code: Some("slug_taken".into()),
            data: None,
        });
    }
    deps.storage.save_trigger(&trigger).await.map_err(internal)?;
    // Editing or reactivating a monitor clears its failure (FR-029).
    crate::triggers::monitor::reset(deps, &trigger.id).await;
    crate::triggers::kick(deps, &trigger.id);
    Ok(respond_full(deps, trigger).await)
}

/// Run a monitor's check now and say what it returned, without firing or saving state (FR-035).
/// The tool calls the script makes really happen.
pub async fn test(
    s: &Services,
    who: &Principal,
    agent_id: &str,
    slug: &str,
) -> Result<crate::schema::trigger::CheckOutcome, ServiceError> {
    manageable(s, who, agent_id).await?;
    let trigger = find(s, agent_id, slug).await?;
    test_trigger(s.deps(), &trigger).await
}

pub(crate) async fn test_trigger(
    deps: &VizierDependencies,
    trigger: &Trigger,
) -> Result<crate::schema::trigger::CheckOutcome, ServiceError> {
    use crate::triggers::monitor;
    let Some(script) = monitor::script(&trigger.settings) else {
        return Err(ServiceError::invalid("only a monitor can be tested"));
    };
    let record = deps.storage.monitor_record(&trigger.id).await.map_err(internal)?;
    let check = crate::schema::trigger::MonitorCheck {
        trigger_slug: trigger.slug.clone(),
        script: script.to_string(),
        state: record.state,
        scope: monitor::check_scope(&trigger.settings),
        timeout_secs: monitor::check_timeout_secs(&trigger.settings, &deps.triggers.limits),
    };
    let mut outcome = deps.agents.run_check(&trigger.agent_id, check).await;
    // Say how the value would be read, so a test shows a bad shape as the check would.
    if outcome.error.is_none()
        && let Err(problem) = monitor::read_finding(outcome.value.clone())
    {
        outcome.error = Some(problem);
    }
    Ok(outcome)
}

/// Approve a trigger an agent made (FR-036), choosing its triage model when it has triage. The
/// agent never saw a webhook's secret, so approving one sets a fresh secret and shows it to the
/// person approving, this once.
pub async fn approve(
    s: &Services,
    who: &Principal,
    agent_id: &str,
    slug: &str,
    triage_model: Option<(String, String)>,
) -> Result<CreatedTrigger, ServiceError> {
    let config = manageable(s, who, agent_id).await?;
    let deps = s.deps();
    let mut trigger = find(s, agent_id, slug).await?;
    let kind = deps
        .triggers
        .kinds
        .get(&trigger.kind)
        .ok_or_else(|| ServiceError::invalid("this trigger's kind is no longer available"))?;
    let caller = Caller::from(who);
    if kind.is_public() && !access::can_publish_webhooks(caller, &config) {
        return Err(ServiceError::Forbidden("You may not open public webhooks on this agent".into()));
    }
    if let Some((provider, model)) = triage_model {
        if !access::can_choose_model(caller) {
            return Err(ServiceError::Forbidden("You may not choose a model for triage".into()));
        }
        let Some(triage) = trigger.triage.as_mut() else {
            return Err(ServiceError::invalid("this trigger has no triage"));
        };
        triage.provider = Some(provider);
        triage.model = Some(model);
    }
    if trigger.triage.as_ref().is_some_and(|t| t.model().is_none()) {
        return Err(ServiceError::invalid("choose a model for this trigger's triage to approve it"));
    }
    let secret = match trigger.awaiting_approval {
        true => kind.replace_secret(&mut trigger.settings),
        false => None,
    };
    trigger.awaiting_approval = false;
    trigger.is_active = true;
    deps.storage.save_trigger(&trigger).await.map_err(internal)?;
    crate::triggers::monitor::reset(deps, &trigger.id).await;
    crate::triggers::kick(deps, &trigger.id);
    Ok(CreatedTrigger { trigger: respond_full(deps, trigger).await, secret })
}

pub async fn replace_secret(
    s: &Services,
    who: &Principal,
    agent_id: &str,
    slug: &str,
) -> Result<CreatedTrigger, ServiceError> {
    let config = manageable(s, who, agent_id).await?;
    let deps = s.deps();
    let mut trigger = find(s, agent_id, slug).await?;
    let kind = deps
        .triggers
        .kinds
        .get(&trigger.kind)
        .ok_or_else(|| ServiceError::invalid("this trigger's kind is no longer available"))?;
    if !access::can_publish_webhooks(Caller::from(who), &config) {
        return Err(ServiceError::Forbidden("You may not manage public webhooks on this agent".into()));
    }
    let Some(secret) = kind.replace_secret(&mut trigger.settings) else {
        return Err(ServiceError::invalid("this kind of trigger has no secret"));
    };
    deps.storage.save_trigger(&trigger).await.map_err(internal)?;
    Ok(CreatedTrigger { trigger: respond_full(deps, trigger).await, secret: Some(secret) })
}

pub async fn delete(s: &Services, who: &Principal, agent_id: &str, slug: &str) -> Result<(), ServiceError> {
    manageable(s, who, agent_id).await?;
    let trigger = find(s, agent_id, slug).await?;
    s.deps().storage.delete_trigger(&trigger.id).await.map_err(internal)
}

fn page(limit: Option<usize>) -> usize {
    limit.unwrap_or(DEFAULT_PAGE).clamp(1, MAX_PAGE)
}

pub async fn events(
    s: &Services,
    who: &Principal,
    agent_id: &str,
    slug: &str,
    before: Option<(crate::time::Timestamp, i64)>,
    limit: Option<usize>,
) -> Result<Vec<TriggerEvent>, ServiceError> {
    viewable(s, who, agent_id).await?;
    let trigger = find(s, agent_id, slug).await?;
    s.deps().storage.list_trigger_events(&trigger.id, before, page(limit)).await.map_err(internal)
}

pub async fn runs(
    s: &Services,
    who: &Principal,
    agent_id: &str,
    slug: &str,
    before: Option<(crate::time::Timestamp, i64)>,
    limit: Option<usize>,
) -> Result<Vec<TriggerRun>, ServiceError> {
    viewable(s, who, agent_id).await?;
    let trigger = find(s, agent_id, slug).await?;
    s.deps().storage.list_trigger_runs(&trigger.id, before, page(limit)).await.map_err(internal)
}

/// What a run replied: the last message in its conversation.
pub(crate) async fn run_reply(deps: &VizierDependencies, run: &TriggerRun) -> Option<String> {
    use crate::schema::{SessionHistoryContent, VizierResponseContent};
    let history = deps.storage.list_session_history(run_session(run), None, None, None).await.ok()?;
    history.iter().rev().find_map(|entry| match &entry.content {
        SessionHistoryContent::Response(response) => match &response.content {
            VizierResponseContent::Message { content, .. } => Some(content.clone()),
            _ => None,
        },
        _ => None,
    })
}

/// A run, its events and its conversation.
#[derive(Debug, Serialize, Clone)]
#[cfg_attr(feature = "utoipa", derive(utoipa::ToSchema))]
pub struct RunDetail {
    pub run: TriggerRun,
    pub events: Vec<TriggerEvent>,
    pub history: Vec<SessionHistory>,
}

pub async fn run_detail(
    s: &Services,
    who: &Principal,
    agent_id: &str,
    slug: &str,
    run_id: i64,
) -> Result<RunDetail, ServiceError> {
    viewable(s, who, agent_id).await?;
    let trigger = find(s, agent_id, slug).await?;
    let run = s
        .deps()
        .storage
        .get_trigger_run(run_id)
        .await
        .map_err(internal)?
        .filter(|run| run.trigger_id == trigger.id)
        .ok_or_else(|| ServiceError::NotFound(format!("no run {run_id} of {slug}")))?;
    let events = s.deps().storage.trigger_run_events(run.id).await.map_err(internal)?;
    let history = s
        .deps()
        .storage
        .list_session_history(run_session(&run), None, None, None)
        .await
        .map_err(internal)?;
    Ok(RunDetail { run, events, history })
}
