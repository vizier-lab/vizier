# Research: Agent custom tools

Phase 0 decisions for [plan.md](plan.md). Each one: what was chosen, why, what else was weighed.
Code references are to the tree at `d92fbafb`.

## R1. Collapse per-turn catalogue sources behind one trait before adding a third

**Decision**: Introduce `TurnToolSource` (`agents/tools/turn_source.rs`):

```rust
#[async_trait]
pub trait TurnToolSource: Send + Sync {
    fn id(&self) -> &'static str;                       // "client", "project", "custom"
    async fn entries(&self, ctx: &ToolContext) -> Vec<CatalogueEntry>;
    fn categories(&self, ctx: &ToolContext) -> Vec<CategorySummary>;
    async fn call(&self, name: &str, args: serde_json::Value, ctx: &ToolContext,
                  bridge: &BridgeEnv) -> anyhow::Result<VizierResponse>;
}
```

`ToolTarget::Client { .. }` and `ToolTarget::Project` become one `ToolTarget::Turn { source }`.
`turn_catalogue` and `turn_categories` iterate the turn's sources (from `ToolContext`), and
`RouterBridge::dispatch_to` calls `source.call(..)` for any `Turn` target. Client tools keep their
call-id side channel by returning it through `BridgeEnv` (a small struct of per-call outputs).

**Rationale**: today `RouterBridge::dispatch_to` (python/bridge.rs:101–125), `ToolRouter::dispatch`
(mod.rs:1142–1151), the ranking in `ToolRouter::catalogue` (mod.rs:1072–1080), `turn_catalogue`
(mod.rs:841–856) and `turn_categories` (mod.rs:886–894) each branch on client vs project. Custom
tools are the same shape — per-turn entries the router does not hold, dispatched by the bridge — so
they would be the third copy. The constitution (Principle II, "When a third near-identical code path
appears, it MUST be collapsed … before new work continues") makes this mandatory, and it is cheap:
two existing implementations move, behaviour is pinned by the existing client-tool and project-tool
tests.

**Alternatives**: a third `ToolTarget::Custom` arm in each match (rejected: the constitution
violation itself); making custom tools native `VizierTool`s inside `ToolRouter` (rejected: the
router is composed per instance and rebuilt only on integration changes, while custom tools change
on every save and must be callable in the same turn they are saved — SC-001).

## R2. A call is `run_script` of the saved script with a bound `input`

**Decision**: A custom tool's script is ordinary sandbox Python. Its validated input is bound to a
global named `input` (a dict), and, as in `execute_python`, the value of the last expression is the
return value. `sandbox::runtime` gains `execute_with_input(code, input, limits, bridge)`: the
existing `MontyRun::new(code, "main.py", vec![], …)` / `runner.start(vec![], …)` take input names and
values (runtime.rs:134, :156), so this is `vec!["input"]` and `vec![json_to_monty(input)]`.
`python::run_script` gets an `input: Option<Value>` parameter. The call is made by
`CustomTurnSource::call`, which loads the **current** version from storage (so a disable, delete or
update made earlier in the turn takes effect at the next call — spec edge "Mid-turn change"), checks
the input (R6), checks the call chain for a cycle (R5), and runs `run_script` with the bridge's own
router and the caller's `ToolContext`, under the caller's per-call tool timeout and deadline.

FR-005's "declares the entry it is called through" is therefore satisfied by the script parsing;
there is no entry function to declare. The create tool's description says "`input` holds the
arguments; the last expression is the result".

**Rationale**: no new execution model, no new limits; nested calls inherit hooks, availability,
redaction, timeouts and spans because they go through `RouterBridge` like any script's.
`run_script` already returns a report instead of failing, which maps directly onto FR-014.

**Alternatives**: a `def run(input):` entry (rejected: one more rule to validate and explain, and
agents already write last-expression scripts); a declarative flow format (out of scope, FR-003).

**Nesting**: the outer interpreter blocks in `handle.block_on` while the nested `execute` runs on
another `spawn_blocking` thread (runtime.rs:75), so nesting works with no engine change. Each
level holds one blocking-pool thread; with no depth limit, the caller's deadline is the bound
(FR-015). The sandbox's existing 64 KiB script cap (`MAX_SCRIPT_BYTES`) applies to custom tool
scripts too: it is the sandbox's limit, not a custom-tool limit, so FR-015a holds.

## R3. Discovery: catalogue entries per script run, category lines per turn

**Decision**:
- **Entries** come from `CustomTurnSource::entries`, one storage query (`list_enabled(agent)`)
  each time `RouterBridge` fills its per-run catalogue (`OnceCell`, bridge.rs:65). A tool saved in
  this turn is callable from the next script run in the same turn (SC-001).
- **Category lines** are snapshotted once at turn start into `ToolContext.custom_categories`
  (where `VizierAgent` already resolves the project context and client tools), and
  `turn_categories` appends them after built-in, addon, MCP and client lines. `list_tool_functions`
  reads the same snapshot, so the prompt and the listing agree for the turn (FR-010f, SC-010).
- An entry's name is `custom_<name>`; its `CatalogueEntry.category` is the stored category name;
  its definition's description is the tool's description plus a one-line suffix
  (`Custom tool v3 by you (dream 2026-11-14). Calls a model-backed tool.` / `Needs attention:
  missing stripe_list_charges`). `describe_tool_function` adds `version`, `author`,
  `model_backed`, `needs_attention` fields through the source's doc hook (FR-010).
- The full listing marks custom entries with `custom: true` in `ToolFunctionDoc` (FR-010).
- Disabled tools are never entries (Story 2 #8).

**Rationale**: the prompt prefix stays cacheable (only category lines are in it); the listing is
always current; no bus event is needed for a change to reach the agent, because both reads are
storage reads made by the agent itself.

**Alternatives**: rebuilding `VizierTools` on every save via `refresh_integrations` (rejected: a
save mid-turn would not be callable until the next turn, and composition is heavier than a query);
putting categories in `context_md` (rejected: the category list belongs in `SANDBOX.md`, and moving
custom ones elsewhere splits the "pick a category" instruction).

## R4. Finding the tools a script calls: `ruff_python_parser`

**Decision**: `custom_tools/calls.rs` parses the script with `ruff_python_parser` 0.0.14 and walks
the AST (`ruff_python_ast` visitor) for `Call` nodes whose `func` is a bare `Name`. The set of
names, minus Python builtins and names assigned in the script, is the tool's **calls** list. It is
stored on each version and used for: unknown-tool refusal (FR-005), cycle detection (R5),
`needs attention` (R8) and the model-backed flag (R7). A parse error is reported with line and
column (FR-005).

**Rationale**: both crates are already compiled into the binary through `monty` at exactly this
version (Cargo.lock: `ruff_python_parser 0.0.14`, registry source), so a direct dependency adds no
weight (Principle I, "prefer crates already in the tree"). A regex over `name(` would misread
strings, comments and attribute calls.

**Limits accepted**: calls made indirectly (`f = memory_write; f(...)`, `getattr`) are not seen;
they still work at run time and fail at that step if the tool is missing — the same answer a
direct call gets. Documented in the create tool's description.

**Alternatives**: asking `monty` for its parse tree (not exposed in 1.0.0); a test run as the only
check (rejected: it has side effects and is skipped in dreams and the WebUI).

## R5. Cycles: refused at save time and at call time

**Decision**: at save, build the graph `custom tool → custom tools it calls` from the stored
`calls` of the agent's current versions plus the candidate, and refuse if the candidate reaches
itself; the refusal names the cycle (`a → b → a`). At call time, `ToolContext.custom_chain`
(a `Vec<String>`, copied into the nested call's context) refuses a name already on the chain —
this covers indirect calls R4 cannot see and edits that raced.

**Rationale**: FR-015 asks for both; the call-time check is three lines and makes the save-time
check a convenience rather than the only guard.

## R6. Input schemas: a documented JSON Schema subset, checked by hand

**Decision**: `custom_tools/schema.rs` accepts an object schema built from: `type`
(object/string/number/integer/boolean/array/null, or a list of them), `properties`, `required`,
`additionalProperties` (bool), `items`, `enum`, `const`, `description`, `default`, `title`,
`minimum`/`maximum`, `minLength`/`maxLength`, `minItems`/`maxItems`. At save, an unknown keyword is
a problem naming the keyword and the path. At call, `validate(schema, input)` returns every mismatch
with its JSON path (`$.items[2].id: expected string, got number`) (FR-012). `default`s are filled
before the script runs.

**Rationale**: there is no JSON Schema validator in the tree (client tool declarations only check
that schemas are objects, `client/declare.rs`), the full spec pulls in a heavy crate (regex
engines, format validators, optional HTTP for `$ref`), and agents write simple input shapes. The
subset is what `describe_tool_function` already renders well.

**Alternatives**: the `jsonschema` crate (rejected for weight, Principle I and the dependency rule);
no validation (rejected: FR-012).

## R7. "Model-backed" is a tool property, propagated through calls

**Decision**: `VizierToolDyn` gains `fn model_backed(&self) -> bool` (default `false`, a trait
method like `ends_turn`). It is `true` for tools whose result comes from a model:
`spawn_subagents`, `mention_agent`, image generation, `read_image` with a vision model, STT/TTS.
MCP tools are `false` (unknown). A custom tool is model-backed when any tool in its `calls` is, or
any custom tool it calls is (transitively, computed with R5's graph). Shown in the description
suffix and `describe_tool_function` (FR-016).

**Rationale**: trait-based (Principle II) rather than a name list; computed, never stored stale.

## R8. "Needs attention" and the gateway: a published view of the agent's tool names

**Decision**: the agent host publishes `agent.{id}.tool_names` to shared state — the sorted set of
script-callable names of its composed `ToolRouter` (native, addon, MCP), with the usual
`{"expires_at_ms", "data"}` envelope of `agents/views.rs` — on take, on every
`refresh_integrations`, and on the views' regular refresh. `CustomTools` uses it to:
- compute **needs attention** on every read: a version's `calls` minus the published names minus
  the agent's enabled custom tool names (FR-026; derived, never a stored flag that can go stale);
- check **unknown tools** for a person's edit from the WebUI (FR-022a), where no router exists. An
  expired or missing view makes that one check a warning ("could not check the tools this calls"),
  never a false refusal.

Inside a turn, the save check uses the turn's own catalogue instead (`ctx` carries the router; R9),
so the agent is judged against exactly what it can call. A dream save is judged against the
agent's full router, not the dream's subset (FR-009a).

**Rationale**: CLAUDE.md's seam rule — the gateway never reads a running agent's state; published
views are the sanctioned way (`agent.{id}.health`, `agent.{id}.setup`). `scripts/048-agent-state-
read-check.sh` stays clean.

**Alternatives**: storing `missing_tools` on the row and reconciling (rejected: stale between
reconciles, and a second write path); skipping the unknown-tool check for people (rejected: FR-022a).

## R9. Authoring tools need the router: carry it on the context, not in the tool

**Decision**: `ToolContext` gains `script_router: Option<ToolRouter>`. `RouterBridge::new` sets it
on its own copy of the context, so any native tool called from a script (the authoring tools)
can validate against the turn's catalogue and perform the **sample run** (FR-006) through
`run_script`. `dream_call` sets it to the agent's full router with a `dream` marker, so dream saves
validate against the full toolset but skip the sample run (FR-009a). HTTP edits have none: no
sample run (FR-022a), unknown-tool check through R8.

**Rationale**: the authoring tools live in `default_toolset`, which is inside the router; holding a
router in the tool would be a reference cycle. The context is per call and already carries
per-turn capabilities (`client`, `project`).

## R10. Storage: four tables, ids not names, revision vocabulary reused

**Decision**: tables `custom_tool`, `custom_tool_version`, `custom_tool_category`,
`custom_tool_event` (see [data-model.md](data-model.md)) behind `CustomToolStorage` on
`VizierStorageProvider`, one implementation on `SqlStorage` for both dialects. A tool is keyed by a
UUID so a rename keeps its history. Versions reuse `RevisionActor` / `RevisionTrigger` /
`RevisionOrigin` from `schema/revision.rs` (with `dream_cycle_id`, as memory revisions do) and
`storage/diff.rs::diff_lines` for diffs. Every save is one transaction: validate-time checks that
depend on stored state (name free, category exists or created, no cycle, `current_seq` equals the
expected version) are re-checked inside it, so concurrent saves never lose a version (FR-020).
`delete_agent` deletes the agent's rows. Not fenced: like skills and memory, custom tools are not
in-flight work (#164 contract table unchanged).

**Rationale**: same shape as `core_revision`/`memory_revision` (spec 006), which people already
read in the WebUI.

## R11. One write path for agent, dream and person

**Decision**: `custom_tools::CustomTools` (on `VizierDependencies`) owns `create`, `update`,
`rollback`, `delete`, `set_enabled`, `update_category`, each taking an `Actor` (agent in a turn,
agent in a dream with the cycle id, or person) and returning `status`-shaped outcomes. The agent's
authoring tools, `dream_call` and `service::custom_tools` (HTTP) all call it, the way
`integrations/service.rs` is shared by tools and handlers. Authorization for people lives in
`service::custom_tools` using `access::can_view_agent` / `can_edit_agent` (new authorization goes
in core). Rules owned by the actor: the agent cannot re-enable a person-disabled tool (FR-023);
dreams cannot delete, enable or disable (FR-009).

## R12. Categories

**Decision**: `custom_tool_category (agent_id, name, description, updated_by…)`. A save naming an
unknown category creates the row in the same transaction and requires `category_description`
(FR-010b). The default `custom` row is created on first use with a fixed description ("Tools you
made that have no category yet"). The stored name is computed by `naming::category_name(requested,
reserved)` = the same `reserved_category_name(.., "custom")` rule MCP and client categories use
(mod.rs:817), against built-in names, addon ids and the agent's MCP keys **at save time**. If an MCP
server attached later takes a custom category's name, the turn source shows that custom category
as `<name>_custom` for as long as the clash lasts (entries and line alike; the stored name is
unchanged), so the server keeps its usual name and the clash rule stays one function. A category row with no tools is deleted in the
transaction that removes its last tool (FR-010c). Renames update the tools' `category` in the same
transaction (FR-010e).

**Tool names**: `^[a-z][a-z0-9_]*$`, reached as `custom_<name>`. `custom_` becomes a reserved prefix:
a test pins that no native, addon or retired tool name starts with it, and MCP/client naming
(`assign_names`, `client_category_name`) skips it (FR-011).

## R13. Nested calls in reports and traces (FR-028)

**Decision**: `ToolInvocationRecord` gains `calls: Vec<ToolInvocationRecord>` (serde `skip if
empty`). A custom tool's record in the caller's report carries the nested run's records; the
nested records are not repeated at the outer level (the inner bridge has its own list). The span
is the normal `execute_tool custom_<name>` from `ToolRouter`'s path with `vizier.custom_tool.id`
and `vizier.custom_tool.version`; nested `execute_tool` spans are its children because the nested
run executes inside it. `last_called_at` is written best-effort after each call (a failed write is
logged).

## R14. Dream prompt and dream tools

**Decision**: `scheduler/dream/prompts.rs`:
- `EXTRACTION_PROMPT` gains `### Reusable Workflows` (steps, inputs that varied, whether a step
  needed judgment, how many times seen, the script if one ran).
- `CONSOLIDATION_PROMPT_TEMPLATE` step 4 becomes "Create or update skills and custom tools" with the
  rule (fixed steps → custom tool; judgment → skill; one-off → nothing), "check `list_custom_tools`
  and `list_skills` first, update rather than duplicate, prefer an existing category, suggest
  merging categories of one or two tools"; the audit step adds "N skills, M custom tools created or
  updated; workflows left alone and why".
- `DREAM_TOOL_NAMES` adds `list_custom_tools`, `get_custom_tool`, `get_custom_tool_history`,
  `diff_custom_tool`, `create_custom_tool`, `update_custom_tool`, `rollback_custom_tool`,
  `update_custom_tool_category`; never `delete_custom_tool` or `set_custom_tool_enabled`; the
  custom tools themselves are never dream tools (FR-009). A test pins both lists.

Dream saves take the cycle id from the request metadata the way memory revisions do
(`agents/process.rs`, `dream_cycle_id`).

**Note**: dreams call tools directly (not through `execute_python`), so they see authoring tools as
plain tool definitions; the save input is the same JSON.

## R15. WebUI

**Decision**: route `/:agentId/custom-tools` (list grouped by category) and
`/:agentId/custom-tools/:toolId` (script, schema, history, diff, editor); an `AgentMenu` entry
beside Skills, and a row in `lib/agentDetails.ts` for the phone. Pure logic (grouping, needs
attention text, "changed underneath you" detection) in `lib/customTools.ts` with vitest tests.
Components drawn first (constitution), then built in `design/` with stories: `CustomToolGroup`
(heading with name, description, edit action), `CustomToolRow` (version, author, time, switch,
warning), reusing the existing revision list and diff view from memory history where they fit.
The editor uses the existing code/markdown editor used by skill editing. No live push: the page
re-reads after its own writes; a stale save gets `409 out_of_date` and offers reload.
