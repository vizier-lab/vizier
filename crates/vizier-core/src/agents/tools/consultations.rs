//! The `consultations` tools (`specs/064-consult-agents`, contracts/consultation-tools.md): an
//! agent consults another, real agent the person may use, waits for its answer, replies to its
//! questions, follows up and stops it; the consulted agent asks back with
//! `ask_consulting_agent`. They are thin over the subagent engine, which manages consultations
//! as rows with a consultee. Every refusal is a `status` result, never an exception.

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::{
    agents::{
        subagents::{CONSULTEE_ASK_TOOL, Until, is_consultee, validate_timeout},
        tools::{ToolCategory, ToolContext, VizierTool},
    },
    consultations::access::{self, Consulter, Refusal},
    dependencies::VizierDependencies,
    error::VizierError,
    schema::{AgentRunState, Subagent, SubagentState},
};

/// One call may consult at most this many agents (research D7): each one spends another
/// agent's budget.
pub const MAX_CONSULTATIONS_PER_CALL: usize = 8;

fn summaries(rows: &[Subagent], full: bool) -> Vec<serde_json::Value> {
    rows.iter()
        .map(|row| serde_json::to_value(row.summary(full)).unwrap_or_default())
        .collect()
}

fn summary(row: &Subagent, full: bool) -> serde_json::Value {
    serde_json::to_value(row.summary(full)).unwrap_or_default()
}

fn invalid(message: impl Into<String>) -> serde_json::Value {
    serde_json::json!({ "status": "invalid_input", "message": message.into() })
}

/// Offered to every tool but `ask_consulting_agent` (FR-011, FR-011a, FR-015, FR-016).
fn offered_to_consulter(deps: &VizierDependencies, ctx: &ToolContext) -> bool {
    access::may_consult(ctx, deps.config.consultations.max_depth)
}

/// Whose access decides, or the refusal to return.
async fn consulter(
    deps: &VizierDependencies,
    ctx: &ToolContext,
) -> Result<Result<Consulter, serde_json::Value>, VizierError> {
    Ok(access::consulter(&deps.storage, ctx).await?.map_err(|r| r.to_json()))
}

/// Whether an agent can take requests now, from its published health view; an unknown view
/// counts as available (the request queues).
async fn unavailable_reason(deps: &VizierDependencies, agent_id: &str) -> Option<String> {
    let health = crate::agents::views::read_health(deps, agent_id).await?;
    match health.state {
        AgentRunState::Failed => Some(health.last_error.unwrap_or_else(|| "it failed to start".into())),
        AgentRunState::Stopped => Some("it is stopped".into()),
        AgentRunState::Starting | AgentRunState::Running => None,
    }
}

// --- consult_agent ---------------------------------------------------------------------------

#[derive(Debug, Deserialize, Serialize, JsonSchema)]
pub struct ConsultEntry {
    #[schemars(description = "the agent's id, from list_consultable_agents")]
    pub agent: String,
    #[schemars(description = "everything the agent needs: the goal, the context you already have, and the shape of the answer you want. It knows nothing else about this conversation")]
    pub request: String,
}

#[derive(Debug, Deserialize, Serialize, JsonSchema)]
pub struct ConsultArgs {
    #[schemars(description = "the agents to consult and what to ask each, 1 to 8")]
    pub consultations: Vec<ConsultEntry>,
    #[schemars(description = "[optional] each consultation's time limit in seconds, 1 to 3600 (default 600); time spent waiting for your reply does not count")]
    #[serde(default)]
    pub timeout_secs: Option<u64>,
}

pub struct ConsultAgentTool {
    deps: VizierDependencies,
}

impl ConsultAgentTool {
    pub fn new(deps: VizierDependencies) -> Self {
        Self { deps }
    }
}

pub const CONSULT_DESCRIPTION: &str = "Consult other agents: send each a request and return handles at once without waiting: {\"group\", \"consultations\": [{\"id\", \"consultee\", \"state\": \"running\", ...}]} in the order given. A consulted agent is a separate agent, not a copy of you: it answers as itself, with its own knowledge, memory and tools, at its own cost, and it cannot see this conversation, so give it the goal, the context and the shape of the answer you want. Never pass on secret values. It can ask you a question (it then reads `asking`); answer with reply_consultation, which also sends a follow-up once it has answered. Consultations keep running when this turn ends; answers you have not waited for arrive later as a `[Consultation update]` message. Use wait_consultations to collect them. For help with your own work, hire a subagent instead.";

#[async_trait::async_trait]
impl VizierTool for ConsultAgentTool {
    type Input = ConsultArgs;
    type Output = serde_json::Value;

    fn category() -> Option<ToolCategory> {
        Some(ToolCategory::Consultations)
    }

    fn name() -> String {
        "consult_agent".to_string()
    }

    /// Its result comes from a model (spec 061 R7).
    fn model_backed() -> bool {
        true
    }

    fn description(&self) -> String {
        CONSULT_DESCRIPTION.into()
    }

    fn available(&self, ctx: &ToolContext) -> bool {
        offered_to_consulter(&self.deps, ctx)
    }

    async fn call(&self, args: Self::Input, ctx: &ToolContext) -> Result<Self::Output, VizierError> {
        let timeout_secs = match validate_timeout(args.timeout_secs) {
            Ok(secs) => secs,
            Err(e) => return Ok(invalid(e.0)),
        };
        if args.consultations.is_empty() {
            return Ok(invalid("consultations must not be empty"));
        }
        if args.consultations.len() > MAX_CONSULTATIONS_PER_CALL {
            return Ok(invalid(format!(
                "at most {MAX_CONSULTATIONS_PER_CALL} consultations per call"
            )));
        }
        let consulter = match consulter(&self.deps, ctx).await? {
            Ok(consulter) => consulter,
            Err(refusal) => return Ok(refusal),
        };
        // Every entry is checked before any starts (contracts/consultation-tools.md).
        for entry in &args.consultations {
            if let Err(refusal) =
                access::consultable(&self.deps.storage, ctx, &consulter, &entry.agent).await?
            {
                return Ok(refusal.to_json());
            }
            if let Some(reason) = unavailable_reason(&self.deps, &entry.agent).await {
                return Ok(Refusal::Unavailable {
                    agent: entry.agent.clone(),
                    reason,
                }
                .to_json());
            }
        }
        let entries = args
            .consultations
            .into_iter()
            .map(|entry| (entry.agent, entry.request))
            .collect();
        match self
            .deps
            .subagents
            .consult(ctx, entries, timeout_secs, consulter.on_behalf(ctx))
            .await
        {
            Ok(rows) => Ok(serde_json::json!({
                "group": rows.first().map(|row| row.group_id.clone()),
                "consultations": summaries(&rows, false),
            })),
            Err(e) if e.0.contains("must not be empty") || e.0.contains("the limit is") => {
                Ok(invalid(e.0))
            }
            Err(e) => Err(e),
        }
    }
}

// --- wait_consultations ----------------------------------------------------------------------

#[derive(Debug, Clone, Copy, Default, Deserialize, Serialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum WaitUntil {
    Any,
    #[default]
    All,
}

#[derive(Debug, Deserialize, Serialize, JsonSchema)]
pub struct WaitArgs {
    #[schemars(description = "the consultations to wait for (`c_…`)")]
    pub ids: Vec<String>,
    #[schemars(description = "[optional] `all` (default): until every one has settled; `any`: until one has. A consultation that is asking you counts as settled")]
    #[serde(default)]
    pub until: WaitUntil,
}

pub struct WaitConsultationsTool {
    deps: VizierDependencies,
}

impl WaitConsultationsTool {
    pub fn new(deps: VizierDependencies) -> Self {
        Self { deps }
    }
}

#[async_trait::async_trait]
impl VizierTool for WaitConsultationsTool {
    type Input = WaitArgs;
    type Output = serde_json::Value;

    fn category() -> Option<ToolCategory> {
        Some(ToolCategory::Consultations)
    }

    fn name() -> String {
        "wait_consultations".to_string()
    }

    fn description(&self) -> String {
        "Wait until one (`until: \"any\"`) or every (`until: \"all\"`, the default) given consultation has settled: answered, failed, timed out, stopped, or asking you a question. Returns {\"met\", \"consultations\": [...]}; `met` is false when this call's time ran out first, and the consultations keep running. Answer an asking one with reply_consultation, then wait again. What this returns is not delivered to you again later.".into()
    }

    fn available(&self, ctx: &ToolContext) -> bool {
        offered_to_consulter(&self.deps, ctx)
    }

    async fn call(&self, args: Self::Input, ctx: &ToolContext) -> Result<Self::Output, VizierError> {
        if args.ids.is_empty() {
            return Ok(invalid("ids must not be empty"));
        }
        let until = match args.until {
            WaitUntil::Any => Until::Any,
            WaitUntil::All => Until::All,
        };
        Ok(
            match self.deps.subagents.wait_consultations(ctx, &args.ids, until).await? {
                Ok(waited) => serde_json::json!({
                    "met": waited.met,
                    "consultations": summaries(&waited.workers, false),
                }),
                Err(refusal) => refusal.to_json(),
            },
        )
    }
}

// --- reply_consultation ----------------------------------------------------------------------

#[derive(Debug, Deserialize, Serialize, JsonSchema)]
pub struct ReplyArgs {
    #[schemars(description = "the consultation (`c_…`)")]
    pub id: String,
    #[schemars(description = "your answer to its question, or your follow-up once it has answered")]
    pub text: String,
}

pub struct ReplyConsultationTool {
    deps: VizierDependencies,
}

impl ReplyConsultationTool {
    pub fn new(deps: VizierDependencies) -> Self {
        Self { deps }
    }
}

#[async_trait::async_trait]
impl VizierTool for ReplyConsultationTool {
    type Input = ReplyArgs;
    type Output = serde_json::Value;

    fn category() -> Option<ToolCategory> {
        Some(ToolCategory::Consultations)
    }

    fn name() -> String {
        "reply_consultation".to_string()
    }

    fn description(&self) -> String {
        "Continue a consultation: answer the consulted agent's question while it is asking, or send a follow-up once it has answered. It continues the same conversation with everything said before in view. Returns {\"consultation\"} (now running); refused with status `not_waiting` when it is running, failed, timed out or stopped.".into()
    }

    fn available(&self, ctx: &ToolContext) -> bool {
        offered_to_consulter(&self.deps, ctx)
    }

    async fn call(&self, args: Self::Input, ctx: &ToolContext) -> Result<Self::Output, VizierError> {
        // Access is checked again on every reply and follow-up (FR-013).
        let consulter = match consulter(&self.deps, ctx).await? {
            Ok(consulter) => consulter,
            Err(refusal) => return Ok(refusal),
        };
        let row = match self.deps.subagents.get_consultation_quiet(ctx, &args.id).await? {
            Ok(row) => row,
            Err(refusal) => return Ok(refusal.to_json()),
        };
        if let Some(consultee) = &row.consultee_id
            && let Err(refusal) =
                access::consultable(&self.deps.storage, ctx, &consulter, consultee).await?
        {
            return Ok(refusal.to_json());
        }
        match self.deps.subagents.reply_consultation(ctx, &args.id, &args.text).await {
            Ok(Ok(row)) => Ok(serde_json::json!({ "consultation": summary(&row, false) })),
            Ok(Err(refusal)) => Ok(refusal.to_json()),
            Err(e) if e.0.contains("must not be empty") || e.0.contains("the limit is") => {
                Ok(invalid(e.0))
            }
            Err(e) => Err(e),
        }
    }
}

// --- stop_consultation -----------------------------------------------------------------------

#[derive(Debug, Deserialize, Serialize, JsonSchema)]
pub struct ConsultationArgs {
    #[schemars(description = "the consultation's id (`c_…`)")]
    pub id: String,
}

pub struct StopConsultationTool {
    deps: VizierDependencies,
}

impl StopConsultationTool {
    pub fn new(deps: VizierDependencies) -> Self {
        Self { deps }
    }
}

#[async_trait::async_trait]
impl VizierTool for StopConsultationTool {
    type Input = ConsultationArgs;
    type Output = serde_json::Value;

    fn category() -> Option<ToolCategory> {
        Some(ToolCategory::Consultations)
    }

    fn name() -> String {
        "stop_consultation".to_string()
    }

    fn description(&self) -> String {
        "Stop one running or asking consultation; the consulted agent's turn on it ends and the others keep going. Returns {\"consultation\"} (stopped). One that already finished is returned as it is with status `already_finished`. A consultation you stopped is never delivered to you.".into()
    }

    fn available(&self, ctx: &ToolContext) -> bool {
        offered_to_consulter(&self.deps, ctx)
    }

    async fn call(&self, args: Self::Input, ctx: &ToolContext) -> Result<Self::Output, VizierError> {
        Ok(match self.deps.subagents.stop_consultation(ctx, &args.id).await? {
            Ok((row, true)) => serde_json::json!({ "consultation": summary(&row, false) }),
            Ok((row, false)) => serde_json::json!({
                "status": "already_finished",
                "consultation": summary(&row, false),
            }),
            Err(refusal) => refusal.to_json(),
        })
    }
}

// --- list_consultations / get_consultation ---------------------------------------------------

#[derive(Debug, Default, Deserialize, Serialize, JsonSchema)]
pub struct ListArgs {}

pub struct ListConsultationsTool {
    deps: VizierDependencies,
}

impl ListConsultationsTool {
    pub fn new(deps: VizierDependencies) -> Self {
        Self { deps }
    }
}

#[async_trait::async_trait]
impl VizierTool for ListConsultationsTool {
    type Input = ListArgs;
    type Output = serde_json::Value;

    fn category() -> Option<ToolCategory> {
        Some(ToolCategory::Consultations)
    }

    fn name() -> String {
        "list_consultations".to_string()
    }

    fn description(&self) -> String {
        "The consultations you started in this conversation: running and asking first, then the last 50 finished ones. Returns {\"consultations\": [...]}. Listing does not count as reading an answer.".into()
    }

    fn available(&self, ctx: &ToolContext) -> bool {
        offered_to_consulter(&self.deps, ctx)
    }

    async fn call(&self, _args: Self::Input, ctx: &ToolContext) -> Result<Self::Output, VizierError> {
        let rows = self.deps.subagents.list_consultations(ctx).await?;
        Ok(serde_json::json!({ "consultations": summaries(&rows, false) }))
    }
}

pub struct GetConsultationTool {
    deps: VizierDependencies,
}

impl GetConsultationTool {
    pub fn new(deps: VizierDependencies) -> Self {
        Self { deps }
    }
}

#[async_trait::async_trait]
impl VizierTool for GetConsultationTool {
    type Input = ConsultationArgs;
    type Output = serde_json::Value;

    fn category() -> Option<ToolCategory> {
        Some(ToolCategory::Consultations)
    }

    fn name() -> String {
        "get_consultation".to_string()
    }

    fn description(&self) -> String {
        "One consultation with its whole request and answer: {\"consultation\"}. Its open question or answer counts as read, so it is not delivered to you again.".into()
    }

    fn available(&self, ctx: &ToolContext) -> bool {
        offered_to_consulter(&self.deps, ctx)
    }

    async fn call(&self, args: Self::Input, ctx: &ToolContext) -> Result<Self::Output, VizierError> {
        Ok(match self.deps.subagents.get_consultation(ctx, &args.id).await? {
            Ok(row) => serde_json::json!({ "consultation": summary(&row, true) }),
            Err(refusal) => refusal.to_json(),
        })
    }
}

// --- list_consultable_agents -----------------------------------------------------------------

pub struct ListConsultableAgentsTool {
    deps: VizierDependencies,
}

impl ListConsultableAgentsTool {
    pub fn new(deps: VizierDependencies) -> Self {
        Self { deps }
    }
}

#[async_trait::async_trait]
impl VizierTool for ListConsultableAgentsTool {
    type Input = ListArgs;
    type Output = serde_json::Value;

    fn category() -> Option<ToolCategory> {
        Some(ToolCategory::Consultations)
    }

    fn name() -> String {
        "list_consultable_agents".to_string()
    }

    fn description(&self) -> String {
        "The agents you may consult from this conversation, the ones available to the person you are working for: {\"agents\": [{\"id\", \"name\", \"description\", \"available\"}]}. `available` is false when an agent cannot take requests now.".into()
    }

    fn available(&self, ctx: &ToolContext) -> bool {
        offered_to_consulter(&self.deps, ctx)
    }

    async fn call(&self, _args: Self::Input, ctx: &ToolContext) -> Result<Self::Output, VizierError> {
        let consulter = match consulter(&self.deps, ctx).await? {
            Ok(consulter) => consulter,
            Err(refusal) => return Ok(refusal),
        };
        let mut agents = vec![];
        for (id, config) in access::consultable_agents(&self.deps.storage, ctx, &consulter).await? {
            let available = unavailable_reason(&self.deps, &id).await.is_none();
            agents.push(serde_json::json!({
                "id": id,
                "name": config.name,
                "description": config.description,
                "available": available,
            }));
        }
        Ok(serde_json::json!({ "agents": agents }))
    }
}

// --- ask_consulting_agent (the consulted side only) ------------------------------------------

#[derive(Debug, Deserialize, Serialize, JsonSchema)]
pub struct AskArgs {
    #[schemars(description = "one clear question only the agent consulting you can answer")]
    pub question: String,
}

pub struct AskConsultingAgentTool {
    deps: VizierDependencies,
}

impl AskConsultingAgentTool {
    pub fn new(deps: VizierDependencies) -> Self {
        Self { deps }
    }
}

#[async_trait::async_trait]
impl VizierTool for AskConsultingAgentTool {
    type Input = AskArgs;
    type Output = serde_json::Value;

    fn category() -> Option<ToolCategory> {
        Some(ToolCategory::Consultations)
    }

    fn name() -> String {
        CONSULTEE_ASK_TOOL.to_string()
    }

    fn description(&self) -> String {
        "Ask the agent consulting you one question when you cannot answer without something only it can tell you. Your step ends here; its answer arrives as your next message and you continue from where you stopped. You cannot ask the person. Do not ask what you can reasonably decide yourself.".into()
    }

    /// Offered only in a consultation conversation.
    fn available(&self, ctx: &ToolContext) -> bool {
        is_consultee(ctx)
    }

    fn ends_turn() -> bool {
        true
    }

    async fn call(&self, args: Self::Input, ctx: &ToolContext) -> Result<Self::Output, VizierError> {
        // A refusal is an error here, not a status: a successful call ends the turn, and a
        // question that was not recorded must not.
        let asked = self.deps.subagents.ask(ctx, &args.question).await?;
        debug_assert_eq!(asked.state, SubagentState::Asking);
        Ok(serde_json::json!({ "status": "asked" }))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_consult_description_says_what_a_consultation_is() {
        assert!(CONSULT_DESCRIPTION.contains("without waiting"));
        assert!(CONSULT_DESCRIPTION.contains("not a copy of you"));
        assert!(CONSULT_DESCRIPTION.contains("reply_consultation"));
        assert!(CONSULT_DESCRIPTION.contains("[Consultation update]"));
        assert!(CONSULT_DESCRIPTION.contains("secret"));
    }

    #[test]
    fn only_asking_back_ends_the_turn() {
        assert!(<AskConsultingAgentTool as VizierTool>::ends_turn());
        assert!(!<ConsultAgentTool as VizierTool>::ends_turn());
        assert!(!<WaitConsultationsTool as VizierTool>::ends_turn());
        assert!(!<ReplyConsultationTool as VizierTool>::ends_turn());
    }
}
