//! The `monitor` kind (`specs/071-triggers` FR-025 to FR-029): a short script checks something on
//! an interval, without a model call, and reports an event only when it finds one.
//!
//! One shared source runs every active monitor's checks ([`run_source`]); each check runs in the
//! agent's own instance through `AgentsClient::run_check`, so it reaches the agent's tools,
//! integrations and secrets without a model call.

use std::{
    collections::{HashMap, HashSet},
    sync::{Arc, Mutex},
    time::Duration,
};

use serde_json::{Value, json};

use super::kind::{Intake, TriggerKind};
use crate::{
    agents::tools::scope,
    config::TriggersConfig,
    dependencies::VizierDependencies,
    integrations::kind::{FieldType, KindField, KindListing},
    runtime::Gate,
    schema::{
        NotificationTone, Trigger, WhenFull,
        trigger::{CheckOutcome, MonitorCheck},
    },
    storage::trigger::{MonitorRecord, TriggerStorage},
};

/// The longest a check may run, whatever the interval (FR-028).
pub const MAX_CHECK_SECS: u64 = 30;
/// The largest state a script may keep (FR-026).
pub const MAX_STATE_BYTES: usize = 64 * 1024;

#[derive(Default)]
pub struct MonitorKind {
    /// The operator's limits (FR-025).
    pub limits: TriggersConfig,
}

/// The interval of a monitor's settings, clamped to the operator's floor.
pub fn interval_secs(settings: &Value, limits: &TriggersConfig) -> u64 {
    settings
        .get("interval_secs")
        .and_then(Value::as_u64)
        .unwrap_or(limits.monitor_interval_secs)
        .max(limits.monitor_min_interval_secs.max(1))
}

/// The tools a monitor's script may call (FR-031).
pub fn script_tools(settings: &Value) -> Vec<String> {
    settings
        .get("tools")
        .and_then(Value::as_array)
        .map(|items| items.iter().filter_map(Value::as_str).map(str::to_string).collect())
        .unwrap_or_default()
}

pub fn script(settings: &Value) -> Option<&str> {
    settings.get("script").and_then(Value::as_str).filter(|s| !s.trim().is_empty())
}

impl TriggerKind for MonitorKind {
    fn id(&self) -> &'static str {
        "monitor"
    }

    fn listing(&self) -> KindListing {
        KindListing {
            name: "Monitor",
            description: "A short script checks something on an interval and starts a run only when it finds something",
            icon: "activity",
            fields: vec![
                KindField::new("script", "Script (Python)", FieldType::Code).required(),
                KindField::new("interval_secs", "Seconds between checks", FieldType::Number)
                    .default(json!(self.limits.monitor_interval_secs)),
                KindField::new("tools", "Tool categories the script may call", FieldType::ToolCategories),
            ],
        }
    }

    fn validate(&self, settings: &Value) -> Result<(), String> {
        let Some(script) = script(settings) else {
            return Err("a monitor needs a script".into());
        };
        if script.len() > crate::sandbox::MAX_SCRIPT_BYTES {
            return Err("the script is too long".into());
        }
        if let Some(interval) = settings.get("interval_secs") {
            let min = self.limits.monitor_min_interval_secs.max(1);
            match interval.as_u64() {
                Some(n) if n >= min => {}
                _ => return Err(format!("the interval must be at least {min} seconds")),
            }
        }
        crate::agents::tools::scope::validate(&script_tools(settings))
    }

    /// What a script may call is a person's to widen (FR-031): a new or changed script starts
    /// with reading memory, and an unchanged one keeps what was saved.
    fn restrict_agent_settings(
        &self,
        settings: &mut Value,
        previous: Option<&Value>,
        turn_scope: Option<&[String]>,
    ) {
        let tools = match previous.filter(|old| script(old) == script(settings)) {
            Some(old) => script_tools(old),
            None => scope::within(scope::narrow_default(), turn_scope),
        };
        if let Some(map) = settings.as_object_mut() {
            map.insert("tools".into(), json!(tools));
        }
    }

    fn when_full(&self) -> WhenFull {
        WhenFull::RefuseNew
    }

    fn default_scope(&self) -> Option<Vec<String>> {
        Some(crate::agents::tools::scope::all_but_messaging())
    }
}

/// Categories a check may never call, whatever the monitor lists (FR-028): they reach a model, a
/// person, another agent, or something only a turn has. Model-backed tools elsewhere are kept out
/// by name in the agent (`VizierAgent::run_check`).
const NEVER_IN_A_CHECK: &[&str] = &[
    "subagents", "consultations", "media", "audio", "questions", "ui", "triggers", "sessions",
    "discord", "telegram", "webui", "custom_tools", "integrations", "scheduling",
];

/// What a check's script may call: the categories the monitor lists, minus the ones a check may
/// never use. An empty list allows nothing.
pub fn check_scope(settings: &Value) -> Vec<String> {
    let mut allowed: Vec<String> = script_tools(settings)
        .into_iter()
        .filter(|entry| !entry.starts_with('!') && !NEVER_IN_A_CHECK.contains(&entry.as_str()))
        .collect();
    if allowed.is_empty() {
        // A positive entry nothing is named: the check's own exclusions never widen it.
        allowed.push("__nothing__".into());
    }
    allowed
}

/// The check limit: the shorter of the interval and 30 seconds (FR-028).
pub fn check_timeout_secs(settings: &Value, limits: &TriggersConfig) -> u64 {
    interval_secs(settings, limits).min(MAX_CHECK_SECS)
}

/// What a check's value means (FR-026).
#[derive(Debug, Clone, PartialEq)]
pub enum Finding {
    /// Nothing happened; the state stays.
    Nothing,
    /// A new state, saved now, and nothing to report.
    Remember(Value),
    /// Something to report, and the state to save once it is accepted.
    Fire { payload: Value, state: Value },
}

/// Reads a script's value: `None`, or `{"state": …, "fire": …?}`. Anything else is a failed check.
pub fn read_finding(value: Option<Value>) -> Result<Finding, String> {
    let value = match value {
        None | Some(Value::Null) => return Ok(Finding::Nothing),
        Some(value) => value,
    };
    let Value::Object(map) = value else {
        return Err("the script must end with None or {\"state\": …, \"fire\": …}".into());
    };
    if map.keys().any(|k| k != "state" && k != "fire") {
        return Err("the script's result may only have \"state\" and \"fire\"".into());
    }
    let state = map.get("state").cloned().unwrap_or(Value::Null);
    if state.to_string().len() > MAX_STATE_BYTES {
        return Err(format!("the state is over {} KiB", MAX_STATE_BYTES / 1024));
    }
    match map.get("fire") {
        None | Some(Value::Null) => Ok(Finding::Remember(state)),
        Some(payload) => Ok(Finding::Fire { payload: payload.clone(), state }),
    }
}

/// The wait after `failures` consecutive failed checks: the longer of the interval and a delay
/// doubling from 2 s to 5 min (FR-029).
pub fn backoff(interval: u64, failures: u32) -> Duration {
    let doubling = Duration::from_secs(2)
        .saturating_mul(1u32 << failures.saturating_sub(1).min(16))
        .min(Duration::from_secs(300));
    doubling.max(Duration::from_secs(interval))
}

/// The checks running now, so a check that overruns its interval is skipped, never stacked.
#[derive(Default)]
pub struct Running(Mutex<HashSet<String>>);

/// Run one check of `trigger` now and act on it. Returns what the script produced.
pub async fn check_once(deps: &VizierDependencies, trigger: &Trigger) -> CheckOutcome {
    let Some(script) = script(&trigger.settings) else {
        return CheckOutcome::failed("the monitor has no script");
    };
    let record = deps.storage.monitor_record(&trigger.id).await.unwrap_or_default();
    let limits = &deps.triggers.limits;
    let interval = interval_secs(&trigger.settings, limits);
    let check = MonitorCheck {
        trigger_slug: trigger.slug.clone(),
        script: script.to_string(),
        state: record.state.clone(),
        scope: check_scope(&trigger.settings),
        timeout_secs: check_timeout_secs(&trigger.settings, limits),
    };
    let outcome = deps.agents.run_check(&trigger.agent_id, check).await;
    let now = crate::time::now();
    let finding = match &outcome.error {
        Some(error) => Err(error.clone()),
        None => read_finding(outcome.value.clone()),
    };
    match finding {
        Err(error) => {
            let failures = record.failures + 1;
            let failed = failures >= limits.monitor_max_failures.max(1);
            let next = now + chrono::Duration::from_std(backoff(interval, failures)).unwrap_or_default();
            let saved = MonitorRecord {
                last_check_at: Some(now),
                next_check_at: Some(next),
                last_error: Some(error.chars().take(500).collect()),
                failures,
                failed_at: failed.then_some(now),
                ..record
            };
            if let Err(e) = deps.storage.save_monitor_record(&trigger.id, &saved).await {
                tracing::warn!(trigger = %trigger.slug, "could not record a failed check: {e}");
            }
            if failed {
                super::delivery::notice(
                    deps,
                    trigger,
                    "monitor_failed",
                    format!("Monitor \u{201c}{}\u{201d} stopped after {failures} failed checks", trigger.title),
                    saved.last_error.clone(),
                    NotificationTone::Failed,
                )
                .await;
            }
        }
        Ok(finding) => {
            match finding {
                Finding::Nothing => {}
                Finding::Remember(state) => {
                    let mut current = deps.storage.monitor_record(&trigger.id).await.unwrap_or_default();
                    current.state = Some(state);
                    let _ = deps.storage.save_monitor_record(&trigger.id, &current).await;
                }
                Finding::Fire { payload, state } => {
                    let intake = Intake::Fire { payload, delivery_id: None };
                    // The state is saved by the admission itself, and only if the event is
                    // accepted (FR-027): a refused event leaves it, so the next check reports again.
                    if let Err(e) = super::accept(deps, trigger, intake, Some(state)).await {
                        tracing::warn!(trigger = %trigger.slug, "could not record a monitor event: {e:#}");
                    }
                }
            }
            let current = deps.storage.monitor_record(&trigger.id).await.unwrap_or_default();
            let saved = MonitorRecord {
                last_check_at: Some(now),
                next_check_at: Some(now + chrono::Duration::seconds(interval as i64)),
                last_error: None,
                failures: 0,
                failed_at: None,
                ..current
            };
            if let Err(e) = deps.storage.save_monitor_record(&trigger.id, &saved).await {
                tracing::warn!(trigger = %trigger.slug, "could not record a check: {e}");
            }
        }
    }
    outcome
}

/// Clear a monitor's failure, as editing or reactivating it does (FR-029).
pub async fn reset(deps: &VizierDependencies, trigger_id: &str) {
    if let Ok(mut record) = deps.storage.monitor_record(trigger_id).await {
        record.failures = 0;
        record.failed_at = None;
        record.last_error = None;
        record.next_check_at = None;
        let _ = deps.storage.save_monitor_record(trigger_id, &record).await;
    }
}

/// The one source for every monitor (research R10): each second, start the checks that are due,
/// only while this node owns control (FR-039). Never returns.
pub async fn run_source(deps: VizierDependencies, gate: Arc<dyn Gate>) {
    let running = Arc::new(Running::default());
    let mut triggers: Vec<Trigger> = vec![];
    let mut listed_at = std::time::Instant::now() - Duration::from_secs(60);
    let mut tick = tokio::time::interval(Duration::from_secs(1));
    loop {
        tick.tick().await;
        if !gate.may_run() {
            continue;
        }
        // The list is re-read every few seconds, so an edit needs no restart.
        if listed_at.elapsed() >= Duration::from_secs(5) {
            listed_at = std::time::Instant::now();
            triggers = match deps.storage.list_triggers(None).await {
                Ok(all) => all.into_iter().filter(|t| t.kind == "monitor" && t.accepts_events()).collect(),
                Err(e) => {
                    tracing::warn!("could not list monitors: {e}");
                    continue;
                }
            };
        }
        let now = crate::time::now();
        let mut due: HashMap<String, Trigger> = HashMap::new();
        for trigger in &triggers {
            let record = deps.storage.monitor_record(&trigger.id).await.unwrap_or_default();
            if record.failed_at.is_some() {
                continue;
            }
            if record.next_check_at.is_none_or(|next| next <= now) {
                due.insert(trigger.id.clone(), trigger.clone());
            }
        }
        for (id, trigger) in due {
            if !running.0.lock().unwrap_or_else(|p| p.into_inner()).insert(id.clone()) {
                continue;
            }
            let deps = deps.clone();
            let running = running.clone();
            tokio::spawn(async move {
                check_once(&deps, &trigger).await;
                running.0.lock().unwrap_or_else(|p| p.into_inner()).remove(&id);
            });
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn kind() -> MonitorKind {
        MonitorKind::default()
    }

    /// The shape the documentation shows: branches that assign, then the value as the last line.
    #[tokio::test]
    async fn the_documented_script_shape_runs_in_the_sandbox() {
        let script = "s = input[\"state\"]\nlatest = \"up\"\nif s is None:\n    result = {\"state\": latest}\nelif latest != s:\n    result = {\"state\": latest, \"fire\": {\"was\": s, \"now\": latest}}\nelse:\n    result = None\nresult";
        let limits = crate::sandbox::SandboxLimits { timeout: Duration::from_secs(5), tools_enabled: false };
        let run = |state: Value| {
            crate::sandbox::execute_with_input(
                script,
                Some(json!({ "state": state })),
                limits,
                Arc::new(crate::sandbox::NoToolsBridge),
            )
        };
        let first = run(Value::Null).await;
        assert!(first.ok, "{:?}", first.error);
        assert_eq!(read_finding(Some(first.result)), Ok(Finding::Remember(json!("up"))));
        let changed = run(json!("down")).await;
        assert_eq!(
            read_finding(Some(changed.result)),
            Ok(Finding::Fire { payload: json!({"was": "down", "now": "up"}), state: json!("up") })
        );
        let same = run(json!("up")).await;
        assert_eq!(read_finding(Some(same.result)), Ok(Finding::Nothing));
    }

    #[test]
    fn a_result_is_none_or_a_state_record() {
        assert_eq!(read_finding(None), Ok(Finding::Nothing));
        assert_eq!(read_finding(Some(json!(null))), Ok(Finding::Nothing));
        assert_eq!(read_finding(Some(json!({"state": 1}))), Ok(Finding::Remember(json!(1))));
        assert_eq!(
            read_finding(Some(json!({"state": 2, "fire": {"n": 2}}))),
            Ok(Finding::Fire { payload: json!({"n": 2}), state: json!(2) })
        );
        assert!(read_finding(Some(json!(5))).is_err());
        assert!(read_finding(Some(json!({"state": 1, "other": 2}))).is_err());
        assert!(read_finding(Some(json!({"state": "x".repeat(MAX_STATE_BYTES + 1)}))).is_err());
    }

    #[test]
    fn backoff_doubles_to_five_minutes_and_never_beats_the_interval() {
        assert_eq!(backoff(5, 1), Duration::from_secs(5));
        assert_eq!(backoff(1, 2), Duration::from_secs(4));
        assert_eq!(backoff(1, 20), Duration::from_secs(300));
        assert_eq!(backoff(600, 3), Duration::from_secs(600));
    }

    #[test]
    fn a_check_never_reaches_a_model_a_person_or_nothing_listed() {
        let scope = check_scope(&json!({"tools": ["web", "secrets", "subagents", "telegram", "!memory"]}));
        assert_eq!(scope, vec!["web".to_string(), "secrets".to_string()]);
        let none = check_scope(&json!({}));
        assert!(!crate::agents::tools::scope::admits(&none, "web", "fetch"));
        let limits = TriggersConfig::default();
        assert_eq!(check_timeout_secs(&json!({"interval_secs": 600}), &limits), MAX_CHECK_SECS);
        assert_eq!(check_timeout_secs(&json!({"interval_secs": 10}), &limits), 10);
    }

    #[test]
    fn an_agent_never_widens_what_a_script_may_call() {
        let mut new = json!({"script": "None", "tools": ["shell", "secrets", "memory"]});
        kind().restrict_agent_settings(&mut new, None, None);
        assert_eq!(script_tools(&new), vec!["memory_read".to_string()]);

        let saved = json!({"script": "None", "tools": ["web"]});
        let mut same = json!({"script": "None", "tools": ["web", "shell"], "interval_secs": 90});
        kind().restrict_agent_settings(&mut same, Some(&saved), None);
        assert_eq!(script_tools(&same), vec!["web".to_string()], "a person's grant stays, nothing is added");

        let mut rewritten = json!({"script": "1", "tools": ["web"]});
        kind().restrict_agent_settings(&mut rewritten, Some(&saved), None);
        assert_eq!(script_tools(&rewritten), vec!["memory_read".to_string()], "a new script starts narrow");

        let mut scoped = json!({"script": "None"});
        kind().restrict_agent_settings(&mut scoped, None, Some(&["triggers".to_string()]));
        assert!(script_tools(&scoped).is_empty(), "never more than the calling turn has");
    }

    #[test]
    fn the_interval_has_a_floor_and_a_script_is_required() {
        assert!(kind().validate(&json!({})).is_err());
        assert!(kind().validate(&json!({"script": "return None", "interval_secs": 4})).is_err());
        assert!(kind().validate(&json!({"script": "return None", "interval_secs": 5})).is_ok());
        let limits = TriggersConfig::default();
        assert_eq!(interval_secs(&json!({"interval_secs": 1}), &limits), 5);
        assert_eq!(interval_secs(&json!({}), &limits), 60);
        assert_eq!(kind().when_full(), WhenFull::RefuseNew);

        // An operator may raise the floor.
        let slow = MonitorKind {
            limits: TriggersConfig { monitor_min_interval_secs: 120, ..Default::default() },
        };
        assert!(slow.validate(&json!({"script": "None", "interval_secs": 60})).is_err());
        assert_eq!(interval_secs(&json!({}), &slow.limits), 120);
    }
}
