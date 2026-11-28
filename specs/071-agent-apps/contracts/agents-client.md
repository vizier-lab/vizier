# Contract addendum: `AgentsClient::app_function`

This is an addendum to `specs/048-agent-service-seam/contracts/agents-client.md`. That file gets a new section with this text when the operation lands.

## Operation

```rust
async fn app_function(&self, agent: &AgentId, call: AppCall) -> crate::Result<AppCallOutcome>;

pub struct AppCall {
    pub app_id: String,
    pub version: u32,          // the version whose manifest defines the function
    pub function: String,
    pub input: serde_json::Value,
    pub caller: AppCaller,     // Page { user_id } | Schedule { name }
    pub deadline_ms: u64,      // caller's budget; the host also applies the function's timeout
}

pub enum AppCallOutcome {
    Ok { output: serde_json::Value },
    Refused { status: String, reason: String },   // unavailable | not_found | ...
    Failed { report: serde_json::Value },          // the redacted ExecutionReport
}
```

## Semantics

- It addresses the **agent** on `agent.{id}.cmd`, like `restart` and `update`, so only the owning host answers.
- Availability errors are those of every operation: `ErrorKind::AgentUnavailable` with *no owner* or *owner lost* in the message, read back by `AgentUnavailable::parse`.
- **The gateway** (`service/apps.rs`) checks ownership, `page_callable`, the approval and the rate limit **before** calling. The host trusts none of that and re-checks only what it alone knows:
  1. the app still names this agent as driver;
  2. the function exists in `version`;
  3. the requirement report for this function is `Ready`.
- **The host** builds a `ToolContext` for session `(agent, App(owner, app), current thread topic)` with:
  - `interactive: false`
  - `detached: false`
  - `client: None`
  - `app: Some(AppTurnContext)`
  - a usage turn id `app-fn:<uuid>`

  It runs the function with `custom_tools::run` against the agent's current `VizierTools` snapshot. The secret redactor applies to the output and the report.
- It never writes to the thread's history: a page call is not a turn.
- `FakeAgentsClient` records the call and answers from its script, so `vizier-http/tests/apps.rs` tests the route with no agent host.
