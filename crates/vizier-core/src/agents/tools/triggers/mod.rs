//! The trigger tools (`specs/071-triggers/contracts/agent-tools.md`), category `triggers`, the
//! 26th. `nothing_to_report` is offered only in a trigger run; the tools that read and manage
//! triggers are never offered there (FR-032).

use std::sync::Arc;

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::{
    agents::tools::{ToolCategory, ToolContext, VizierTool, scope},
    error::VizierError,
    storage::{VizierStorage, trigger::TriggerStorage},
};

pub const NOTHING_TO_REPORT: &str = "nothing_to_report";

/// Ends a trigger run with nothing to deliver (FR-050).
pub struct NothingToReport {
    pub storage: Arc<VizierStorage>,
}

#[derive(Debug, Deserialize, Serialize, JsonSchema)]
pub struct NothingToReportArgs {
    #[schemars(description = "One line on what you checked and why nothing needs the person's attention")]
    pub note: String,
}

#[derive(Debug, Deserialize, Serialize, JsonSchema)]
pub struct NothingToReportResult {
    /// `ok`, or `not_a_trigger_run`.
    pub status: String,
}

#[async_trait::async_trait]
impl VizierTool for NothingToReport {
    type Input = NothingToReportArgs;
    type Output = NothingToReportResult;

    fn name() -> String {
        NOTHING_TO_REPORT.into()
    }

    fn category() -> Option<ToolCategory> {
        Some(ToolCategory::Triggers)
    }

    fn ends_turn() -> bool {
        true
    }

    fn available(&self, ctx: &ToolContext) -> bool {
        ctx.session.is_trigger_run()
    }

    fn description(&self) -> String {
        "Finish this trigger run without sending the person anything, because nothing needs their attention. Your note goes into their periodic summary instead.".into()
    }

    async fn call(&self, args: Self::Input, ctx: &ToolContext) -> Result<Self::Output, VizierError> {
        if !ctx.session.is_trigger_run() {
            return Ok(NothingToReportResult { status: "not_a_trigger_run".into() });
        }
        let note: String = args.note.trim().chars().take(300).collect();
        let note = if note.is_empty() { "nothing to report".to_string() } else { note };
        self.storage
            .note_quiet_run(&ctx.session.to_slug(), &note)
            .await
            .map_err(|e| VizierError(format!("could not record the note: {e}")))?;
        Ok(NothingToReportResult { status: "ok".into() })
    }
}

// ---------------------------------------------------------------------------------------------
// Reading and managing triggers (FR-033 to FR-036, FR-041)
// ---------------------------------------------------------------------------------------------

use serde_json::{Value, json};

use crate::{
    access::Caller,
    dependencies::VizierDependencies,
    schema::{AgentConfig, ChannelPlatform, OverlapPolicy, Requester, TriageConfig, Trigger, VizierChannelId},
    storage::{agent::AgentStorage, platform_link::PlatformLinkStorage, user::UserStorage},
    triggers::access as perms,
};

/// The person a turn acts for, whose trigger permissions bound what the agent may do (FR-041).
pub struct Person {
    pub user_id: String,
    pub username: String,
    pub is_system: bool,
    pub permissions: Vec<String>,
}

impl Person {
    fn caller(&self) -> Caller<'_> {
        Caller { user_id: &self.user_id, is_system: self.is_system, permissions: &self.permissions }
    }
}

/// Whether the trigger tools are offered here: never in a trigger run, a worker, a consultation
/// or a Discord or Telegram group; in a dream only the run readers (FR-032, FR-034, FR-041).
fn may_manage_here(ctx: &ToolContext) -> bool {
    ctx.subagent_depth == 0
        && !ctx.session.is_trigger_run()
        && !matches!(
            ctx.session.1,
            VizierChannelId::Dream(..)
                | VizierChannelId::Subagent
                | VizierChannelId::Consultation(_)
                | VizierChannelId::Workspace(_)
                | VizierChannelId::DiscordChanel(_)
        )
        // A Telegram group's id is negative; a direct message's is the person's own id.
        && !matches!(ctx.session.1, VizierChannelId::TelegramChannel(id) if id < 0)
}

/// The person behind this turn: the one in a WebUI or project chat, the one who linked the account
/// a Telegram direct message comes from (`/connect`, spec 069), or the agent's owner in a scheduled
/// run. `None` when nobody can be named.
pub async fn person(deps: &VizierDependencies, ctx: &ToolContext) -> Option<Person> {
    let user = match &ctx.session.1 {
        VizierChannelId::HTTP(..) | VizierChannelId::Project(..) => {
            deps.storage.get_user(ctx.session.1.web_user()?).await.ok().flatten()?
        }
        // A direct message's channel id is the sender's own account id.
        VizierChannelId::TelegramChannel(id) if *id > 0 => {
            let link = deps.storage.find_link(ChannelPlatform::Telegram, &id.to_string()).await.ok().flatten()?;
            deps.storage.get_user_by_id(&link.user_id).await.ok().flatten()?
        }
        VizierChannelId::Task(..) => {
            let owner = deps.storage.get_agent(&ctx.session.0).await.ok().flatten()?.owner_id?;
            deps.storage.get_user_by_id(&owner).await.ok().flatten()?
        }
        _ => return None,
    };
    let role = deps.storage.get_role(&user.role_id).await.ok().flatten()?;
    Some(Person {
        user_id: user.user_id,
        username: user.username,
        is_system: role.is_system,
        permissions: role.permissions,
    })
}

fn refusal(status: &str, message: impl Into<String>) -> Value {
    json!({ "status": status, "message": message.into() })
}

async fn agent_config(deps: &VizierDependencies, agent_id: &str) -> Result<AgentConfig, Value> {
    match deps.storage.get_agent(agent_id).await {
        Ok(Some(config)) => Ok(config),
        _ => Err(refusal("error", "could not read this agent")),
    }
}

/// The person, and whether they may view (`manage = false`) or manage the triggers.
async fn authorise(deps: &VizierDependencies, ctx: &ToolContext, manage: bool) -> Result<(Person, AgentConfig), Value> {
    let Some(person) = person(deps, ctx).await else {
        return Err(refusal(
            "no_person",
            "nobody this turn acts for can be named, so triggers cannot be used here",
        ));
    };
    let config = agent_config(deps, &ctx.session.0).await?;
    let allowed = match manage {
        true => perms::can_manage(person.caller(), &config),
        false => perms::can_view(person.caller(), &config),
    };
    if !allowed {
        return Err(refusal(
            "forbidden",
            format!("{} may not {} this agent's triggers", person.username, if manage { "manage" } else { "see" }),
        ));
    }
    Ok((person, config))
}

async fn trigger_summary(deps: &VizierDependencies, trigger: Trigger) -> Value {
    let response = crate::service::triggers::respond_full(deps, trigger).await;
    serde_json::to_value(response).unwrap_or(Value::Null)
}

macro_rules! trigger_tool {
    ($name:ident, $tool:literal, $args:ty, $desc:literal, $avail:expr) => {
        pub struct $name {
            pub deps: VizierDependencies,
        }

        #[async_trait::async_trait]
        impl VizierTool for $name {
            type Input = $args;
            type Output = Value;

            fn name() -> String {
                $tool.into()
            }

            fn category() -> Option<ToolCategory> {
                Some(ToolCategory::Triggers)
            }

            fn available(&self, ctx: &ToolContext) -> bool {
                let avail: fn(&ToolContext) -> bool = $avail;
                avail(ctx)
            }

            fn description(&self) -> String {
                $desc.into()
            }

            async fn call(&self, args: Self::Input, ctx: &ToolContext) -> Result<Self::Output, VizierError> {
                Ok(self.run(args, ctx).await.unwrap_or_else(|refused| refused))
            }
        }
    };
}

/// The run readers are offered in dreams too (FR-033); in a dream the agent reads its own.
fn may_read_runs(ctx: &ToolContext) -> bool {
    may_manage_here(ctx) || ctx.in_dream()
}

#[derive(Debug, Deserialize, Serialize, JsonSchema)]
pub struct NoArgs {}

trigger_tool!(ListTriggerKinds, "list_trigger_kinds", NoArgs,
    "List the kinds of trigger you can create, with the settings each takes.", may_manage_here);
impl ListTriggerKinds {
    async fn run(&self, _args: NoArgs, _ctx: &ToolContext) -> Result<Value, Value> {
        Ok(json!({ "status": "ok", "kinds": crate::service::triggers::kinds(&self.deps) }))
    }
}

trigger_tool!(ListTriggers, "list_triggers", NoArgs,
    "List your triggers: what each watches for, whether it is active or awaiting approval, and its limits.", may_manage_here);
impl ListTriggers {
    async fn run(&self, _args: NoArgs, ctx: &ToolContext) -> Result<Value, Value> {
        authorise(&self.deps, ctx, false).await?;
        let triggers = self
            .deps
            .storage
            .list_triggers(Some(&ctx.session.0))
            .await
            .map_err(|e| refusal("error", e.to_string()))?;
        let mut items = Vec::with_capacity(triggers.len());
        for trigger in triggers {
            items.push(trigger_summary(&self.deps, trigger).await);
        }
        Ok(json!({ "status": "ok", "triggers": items }))
    }
}

#[derive(Debug, Deserialize, Serialize, JsonSchema)]
pub struct CreateTriggerArgs {
    #[schemars(description = "A short name: lowercase letters, digits, - and _")]
    pub slug: String,
    #[schemars(description = "The kind: `webhook` or `monitor` (see list_trigger_kinds)")]
    pub kind: String,
    pub title: String,
    #[schemars(description = "What to do when it fires, in the person's words")]
    pub instruction: String,
    #[schemars(description = "The kind's settings. A monitor: {\"script\": \"...\", \"interval_secs\": 60, \"tools\": [\"web\", \"secrets\"]}. Its script reads input[\"state\"] and ends with None or {\"state\": ..., \"fire\": ...}. A webhook: {\"filter\": [\"body.ref=refs/heads/master\"], \"delivery_id\": \"header.x-github-delivery\"}")]
    pub settings: Value,
    #[schemars(description = "Set true when you are setting this up on your own initiative, with nobody having asked")]
    pub on_own_initiative: bool,
    #[schemars(description = "What counts as important, if a model should judge each event first. A person picks the model when approving")]
    pub triage_criteria: Option<String>,
    #[schemars(description = "Runs per hour, at most 20 (the default)")]
    pub hourly_limit: Option<u32>,
}

trigger_tool!(CreateTrigger, "create_trigger", CreateTriggerArgs,
    "Create a trigger that starts a run when something happens. It starts inactive: a person must approve it before it does anything, and the agent's owner is told. Its runs can read memory but not write it or send messages; Vizier delivers each run's final message to the person.", may_manage_here);
impl CreateTrigger {
    async fn run(&self, args: CreateTriggerArgs, ctx: &ToolContext) -> Result<Value, Value> {
        let (person, config) = authorise(&self.deps, ctx, true).await?;
        let Some(kind) = self.deps.triggers.kinds.get(&args.kind) else {
            return Err(refusal("unknown_kind", format!("there is no trigger kind '{}'", args.kind)));
        };
        if kind.is_public() && !perms::can_publish_webhooks(person.caller(), &config) {
            return Err(refusal("forbidden", format!("{} may not open public webhooks", person.username)));
        }
        let existing = self.deps.storage.list_triggers(Some(&ctx.session.0)).await.map_err(|e| refusal("error", e.to_string()))?;
        if existing.len() >= self.deps.triggers.limits.max_per_agent {
            return Err(refusal("too_many", "this agent already has the most triggers it may have"));
        }
        if existing.iter().any(|t| t.slug == args.slug) {
            return Err(refusal("slug_taken", format!("a trigger named '{}' already exists", args.slug)));
        }
        let mut settings = args.settings.clone();
        if !settings.is_object() {
            settings = json!({});
        }
        let turn_scope = ctx.tool_scope.as_ref().map(|s| s.as_slice());
        kind.restrict_agent_settings(&mut settings, None, turn_scope);
        // The secret is never shown to the agent: the person gets it when approving.
        let _ = kind.prepare(&mut settings);
        let limit = args.hourly_limit.unwrap_or(crate::schema::trigger::DEFAULT_HOURLY_LIMIT);
        let trigger = Trigger {
            id: format!("trg_{}", uuid::Uuid::new_v4().simple()),
            agent_id: ctx.session.0.clone(),
            slug: args.slug.clone(),
            kind: args.kind.clone(),
            title: args.title.clone(),
            instruction: args.instruction.clone(),
            settings,
            requester: if args.on_own_initiative {
                Requester::Agent(ctx.session.0.clone())
            } else {
                Requester::User(person.username.clone())
            },
            delivers_to: (!args.on_own_initiative).then(|| person.user_id.clone()),
            is_active: false,
            awaiting_approval: true,
            needs_attention: None,
            tool_scope: scope::intersect(Some(&scope::narrow_default()), turn_scope),
            overlap: OverlapPolicy::Wait,
            // An agent may lower the limit, never raise or switch it off (FR-016).
            hourly_limit: Some(limit.clamp(1, crate::schema::trigger::DEFAULT_HOURLY_LIMIT)),
            cooldown_secs: 0,
            triage: args.triage_criteria.clone().filter(|c| !c.trim().is_empty()).map(|criteria| TriageConfig {
                criteria,
                provider: None,
                model: None,
                calls_per_hour: crate::schema::trigger::DEFAULT_TRIAGE_CALLS_PER_HOUR,
            }),
            destination: None,
            summary_hours: Some(crate::schema::trigger::DEFAULT_SUMMARY_HOURS),
            last_summary_at: None,
            created_at: crate::time::now(),
            last_fired_at: None,
        };
        if let Err(e) = crate::service::triggers::check(kind.as_ref(), &trigger) {
            return Err(refusal("invalid", e.message().to_string()));
        }
        self.deps.storage.create_trigger(&trigger).await.map_err(|e| refusal("error", e.to_string()))?;
        notify_owner(&self.deps, &trigger, "created").await;
        Ok(json!({
            "status": "ok",
            "trigger": trigger_summary(&self.deps, trigger).await,
            "next": "A person must approve it on the agent's Triggers page before it does anything.",
        }))
    }
}

#[derive(Debug, Deserialize, Serialize, JsonSchema)]
pub struct UpdateTriggerArgs {
    pub slug: String,
    pub title: Option<String>,
    pub instruction: Option<String>,
    #[schemars(description = "The whole settings object, replacing the old one")]
    pub settings: Option<Value>,
    pub triage_criteria: Option<String>,
    #[schemars(description = "Runs per hour; you may only lower it")]
    pub hourly_limit: Option<u32>,
}

trigger_tool!(UpdateTrigger, "update_trigger", UpdateTriggerArgs,
    "Change one of your triggers. Any change puts it back to awaiting a person's approval.", may_manage_here);
impl UpdateTrigger {
    async fn run(&self, args: UpdateTriggerArgs, ctx: &ToolContext) -> Result<Value, Value> {
        let (person, config) = authorise(&self.deps, ctx, true).await?;
        let Ok(Some(mut trigger)) = self.deps.storage.get_trigger(&ctx.session.0, &args.slug).await else {
            return Err(refusal("unknown_trigger", format!("there is no trigger '{}'", args.slug)));
        };
        let Some(kind) = self.deps.triggers.kinds.get(&trigger.kind) else {
            return Err(refusal("unknown_kind", "this trigger's kind is no longer available"));
        };
        if kind.is_public() && !perms::can_publish_webhooks(person.caller(), &config) {
            return Err(refusal("forbidden", format!("{} may not manage public webhooks", person.username)));
        }
        if let Some(title) = args.title {
            trigger.title = title;
        }
        if let Some(instruction) = args.instruction {
            trigger.instruction = instruction;
        }
        if let Some(settings) = args.settings {
            let mut merged = kind.merge_secrets(&trigger.settings, settings);
            let turn_scope = ctx.tool_scope.as_ref().map(|s| s.as_slice());
            kind.restrict_agent_settings(&mut merged, Some(&trigger.settings), turn_scope);
            trigger.settings = merged;
        }
        if let Some(criteria) = args.triage_criteria {
            match trigger.triage.as_mut() {
                Some(triage) => triage.criteria = criteria,
                None => {
                    trigger.triage = Some(TriageConfig {
                        criteria,
                        provider: None,
                        model: None,
                        calls_per_hour: crate::schema::trigger::DEFAULT_TRIAGE_CALLS_PER_HOUR,
                    })
                }
            }
        }
        if let Some(limit) = args.hourly_limit {
            let current = trigger.hourly_limit.unwrap_or(u32::MAX);
            trigger.hourly_limit = Some(limit.max(1).min(current));
        }
        trigger.is_active = false;
        trigger.awaiting_approval = true;
        if let Err(e) = crate::service::triggers::check(kind.as_ref(), &trigger) {
            return Err(refusal("invalid", e.message().to_string()));
        }
        self.deps.storage.save_trigger(&trigger).await.map_err(|e| refusal("error", e.to_string()))?;
        notify_owner(&self.deps, &trigger, "changed").await;
        Ok(json!({ "status": "ok", "trigger": trigger_summary(&self.deps, trigger).await }))
    }
}

#[derive(Debug, Deserialize, Serialize, JsonSchema)]
pub struct SlugArgs {
    pub slug: String,
}

trigger_tool!(DeleteTrigger, "delete_trigger", SlugArgs,
    "Delete one of your triggers, with everything it recorded.", may_manage_here);
impl DeleteTrigger {
    async fn run(&self, args: SlugArgs, ctx: &ToolContext) -> Result<Value, Value> {
        authorise(&self.deps, ctx, true).await?;
        let Ok(Some(trigger)) = self.deps.storage.get_trigger(&ctx.session.0, &args.slug).await else {
            return Err(refusal("unknown_trigger", format!("there is no trigger '{}'", args.slug)));
        };
        self.deps.storage.delete_trigger(&trigger.id).await.map_err(|e| refusal("error", e.to_string()))?;
        Ok(json!({ "status": "ok" }))
    }
}

trigger_tool!(TestTrigger, "test_trigger", SlugArgs,
    "Run a monitor's script once and see what it returns. Nothing fires and no state is saved, but the tool calls the script makes really happen. A monitor awaiting approval cannot be tested.", may_manage_here);
impl TestTrigger {
    async fn run(&self, args: SlugArgs, ctx: &ToolContext) -> Result<Value, Value> {
        authorise(&self.deps, ctx, true).await?;
        let Ok(Some(trigger)) = self.deps.storage.get_trigger(&ctx.session.0, &args.slug).await else {
            return Err(refusal("unknown_trigger", format!("there is no trigger '{}'", args.slug)));
        };
        // A script no person has approved is never run on the agent's own say.
        if trigger.awaiting_approval {
            return Err(refusal(
                "awaiting_approval",
                "a person must approve this trigger before its script is run",
            ));
        }
        if let Some(outer) = &ctx.tool_scope {
            let listed = crate::triggers::monitor::script_tools(&trigger.settings);
            if scope::within(listed.clone(), Some(outer)) != listed {
                return Err(refusal("forbidden", "its script may call tools this turn may not"));
            }
        }
        match crate::service::triggers::test_trigger(&self.deps, &trigger).await {
            Ok(outcome) => Ok(json!({ "status": "ok", "result": outcome })),
            Err(e) => Err(refusal("invalid", e.message().to_string())),
        }
    }
}

#[derive(Debug, Deserialize, Serialize, JsonSchema)]
pub struct ListRunsArgs {
    pub slug: String,
    #[schemars(description = "At most 50; 10 when left out")]
    pub limit: Option<usize>,
}

trigger_tool!(ListTriggerRuns, "list_trigger_runs", ListRunsArgs,
    "List the recent runs of one of your triggers, newest first, with how each ended.", may_read_runs);
impl ListTriggerRuns {
    async fn run(&self, args: ListRunsArgs, ctx: &ToolContext) -> Result<Value, Value> {
        if !ctx.in_dream() {
            authorise(&self.deps, ctx, false).await?;
        }
        let Ok(Some(trigger)) = self.deps.storage.get_trigger(&ctx.session.0, &args.slug).await else {
            return Err(refusal("unknown_trigger", format!("there is no trigger '{}'", args.slug)));
        };
        let runs = self
            .deps
            .storage
            .list_trigger_runs(&trigger.id, None, args.limit.unwrap_or(10).clamp(1, 50))
            .await
            .map_err(|e| refusal("error", e.to_string()))?;
        Ok(json!({ "status": "ok", "runs": runs }))
    }
}

#[derive(Debug, Deserialize, Serialize, JsonSchema)]
pub struct RunDetailArgs {
    pub slug: String,
    pub run_id: i64,
}

trigger_tool!(GetTriggerRunDetail, "get_trigger_run_detail", RunDetailArgs,
    "Read one run of your trigger: the events it handled, with their triage verdicts, and what it replied.", may_read_runs);
impl GetTriggerRunDetail {
    async fn run(&self, args: RunDetailArgs, ctx: &ToolContext) -> Result<Value, Value> {
        if !ctx.in_dream() {
            authorise(&self.deps, ctx, false).await?;
        }
        let Ok(Some(trigger)) = self.deps.storage.get_trigger(&ctx.session.0, &args.slug).await else {
            return Err(refusal("unknown_trigger", format!("there is no trigger '{}'", args.slug)));
        };
        let Ok(Some(run)) = self.deps.storage.get_trigger_run(args.run_id).await else {
            return Err(refusal("unknown_run", format!("there is no run {}", args.run_id)));
        };
        if run.trigger_id != trigger.id {
            return Err(refusal("unknown_run", format!("there is no run {} of {}", args.run_id, args.slug)));
        }
        let events = self.deps.storage.trigger_run_events(run.id).await.unwrap_or_default();
        let reply = crate::service::triggers::run_reply(&self.deps, &run).await;
        Ok(json!({ "status": "ok", "run": run, "events": events, "reply": reply }))
    }
}

/// Tell the agent's owner an agent-made trigger needs approval (FR-036).
async fn notify_owner(deps: &VizierDependencies, trigger: &Trigger, what: &str) {
    let owner = deps.storage.get_agent(&trigger.agent_id).await.ok().flatten().and_then(|c| c.owner_id);
    let Some(owner) = owner else {
        return;
    };
    let now = crate::time::now();
    deps.notifications
        .trigger_notice(
            &trigger.agent_id,
            Some(owner),
            format!("trigger_approval:{}:{}", trigger.id, now.timestamp_millis()),
            &trigger.slug,
            &trigger.title,
            format!("The agent {what} trigger \u{201c}{}\u{201d}; it waits for your approval", trigger.title),
            Some(trigger.instruction.chars().take(300).collect()),
            crate::schema::NotificationTone::Default,
            now,
        )
        .await;
}
