# Contract: the per-turn tool source seam (refactor before the feature)

Replaces the client/project special cases in `RouterBridge`, `ToolRouter`, `turn_catalogue` and
`turn_categories` (research R1). Behaviour of client and project tools is unchanged; their existing
tests are the acceptance test of the refactor.

```rust
pub trait TurnToolSource: Send + Sync {
    /// Stable id, also the `ToolTarget::Turn { source }` value.
    fn id(&self) -> &'static str;
    /// Script-callable entries for this turn (names already final, clash rules applied).
    async fn entries(&self, ctx: &ToolContext) -> Vec<CatalogueEntry>;
    /// Category lines this source adds to SANDBOX.md for this turn.
    fn categories(&self, ctx: &ToolContext) -> Vec<CategorySummary>;
    /// Extra fields for describe_tool_function (custom: version/author/…); default None.
    fn describe_extra(&self, _name: &str) -> Option<serde_json::Value> { None }
    /// Run one call. `env` carries the bridge's router and per-call outputs (client call id,
    /// nested invocation records).
    async fn call(&self, name: &str, arguments: serde_json::Value, ctx: &ToolContext,
                  env: &BridgeEnv<'_>) -> anyhow::Result<VizierResponse>;
}
```

- `ToolContext.turn_sources: Vec<Arc<dyn TurnToolSource>>` replaces reading `ctx.client` /
  `ctx.project` in the catalogue code (the fields stay for their own uses).
- Order of sources (and so of entries and lines): client, project, custom.
- `ToolTarget::{Client, Project}` → `ToolTarget::Turn { source: &'static str }`.
- `ToolRouter::dispatch` answers a `Turn` target with "`<name>` is only callable from a script in
  this turn" (as the two arms do today).
- `ctx.detached` still drops turn-ending entries after all sources are collected.
- `custom_chain: Vec<String>` and `custom_categories: Arc<Vec<CategorySummary>>` are new
  `ToolContext` fields; `script_router: Option<ToolRouter>` is set by `RouterBridge::new` and by
  `dream_call` (with `dream: true`).
