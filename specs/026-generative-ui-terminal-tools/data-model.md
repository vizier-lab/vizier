# Data Model: Generative UI and Turn-Ending Tools

No new SQLite table and no migration. A drawn interface is an ordinary saved client tool call (`client_tool_call`, spec 019): its `args` hold the interface, its saved state holds the page state. What is new is a flag on tool definitions, one field on the execution report, one on the reply stats, and the WebUI-side shapes of the interface itself.

## 1. Turn-ending flag (Rust)

| Where | Change | Notes |
|---|---|---|
| `VizierTool` trait (`agents/tools/mod.rs`) | `fn ends_turn() -> bool { false }` | Default method. A tool opts in by overriding it. Mirrors `category()`. |
| `VizierToolDyn` | `fn ends_turn(&self) -> bool` | Filled by the existing blanket impl. |
| `CatalogueEntry` | `ends_turn: bool` | Set in `ToolRouter::catalogue()` (native, addon: from the tool; MCP: `false`) and `ClientTools::resolve` (from the declaration). |
| `ClientToolDeclaration` (`agents/client/declare.rs`) | `ends_turn: bool`, `#[serde(default)]` | Absent = `false`. Not serialised when `false`. Protocol version stays 1 (optional field). |
| `ToolInvocationRecord` (`sandbox/report.rs`) | `ends_turn: bool`, `#[serde(default, skip_serializing_if = "is_false")]` | Set by `RouterBridge` from the entry it dispatched. |
| `VizierResponseStats` | `ended_by_tool: Option<String>` | Tool name that ended the turn. Precedent: `depth_exhausted`. `#[serde(default)]`, so old rows load. |
| `ENDS_TURN_NOTICE` | `const &str` | Appended once to a flagged entry's `ToolDefinition.description` where the definition is built. Not counted by the declaration size check. |

**Turn-ending round** (computed, never stored): an `execute_python` tool response whose report has `ok == true` and at least one `tool_calls[i]` with `ok && ends_turn`.

State transitions of a turn: `running → (round with a turn-ending call succeeds) → ended` skipping the closing completion. A failed call (`record.ok == false`), a failed script (`report.ok == false`), or a `deferred` outcome leaves the turn `running`.

Resume: when a resumed turn's last recorded `execute_python` round is a turn-ending round, `chat_turn` finalises with `ended_by_tool` instead of requesting another completion.

## 2. Interface description (WebUI; carried in the call's `args`)

```text
GenerativeUiArgs
├─ title      string            short, for screen readers and the collapsed view
└─ spec       UiSpec            ≤ 256 KB serialised

UiSpec                           json-render flat format + two extensions
├─ format     1                  optional; version of this shape (default 1)
├─ root       string             id of the root element
├─ elements   map<id, UiElement> ≤ 1500
├─ state      object?            initial page state; keys starting "_" are derived (not saved)
└─ logic      map<name, Logic>?  ≤ 64 KB of source in total

UiElement
├─ type       string             a catalogue component name
├─ props      object             per the component's schema; values may be expressions
├─ children   id[]               per the component's child rule
├─ slots      map<string, id[]>? named child lists, for components that declare them
├─ visible    VisibilityCondition?  json-render: $state/$item/$index, eq/neq/gt/…, $and/$or
├─ repeat     { statePath | $item path, key? }?   one child set per array item
├─ on         map<event, ActionBinding | ActionBinding[]>?
└─ watch      map<statePath, ActionBinding | ActionBinding[]>?

ActionBinding
├─ action     "setState" | "pushState" | "removeState" | "validateForm" | "sendMessage" | "run"
├─ params     object             may contain expressions
└─ (confirm / preventDefault as json-render defines them are not enabled in v1)

Logic
├─ params     string[]           names, ≤ 8
└─ body       string             JS function body; pure; returns JSON-serialisable data

Expression   {$state} {$bindState} {$bindItem} {$item} {$index} {$cond,$then,$else}
             {$template} {$computed, args}                 (json-render; see ui-catalogue contract)
```

**Validation rules** (enforced in the tool handler, see research R4): component and action names exist in the catalogue; props satisfy the component's zod schema; every `children`/`slots`/`root` id exists; `visible`/`repeat`/`watch`/`on` are at element level, not in `props` (auto-fixed when misplaced); `run.fn` names an entry in `logic`; `$computed` names a registered function; size, depth, element-count and logic-size limits; `Image.src` starts with `data:image/`; `Heightfield.resolution ≤ 192`.

## 3. Page state

| Property | Rule |
|---|---|
| Shape | One JSON object, the json-render state model. |
| Initial value | `spec.state`, overlaid by the latest saved state if one exists. |
| Saved | Every change except under keys starting `_`, coalesced to one save per 500 ms through the existing `saveState` (spec 019 §6). ≤ 64 KB, rate-limited by the server as today. |
| Derived (`_`-prefixed) | Never saved, never sent to other tabs. Recomputed on every draw by the interface's own `Effect load` bindings. |
| Sync | Another tab's saved state arrives as a `client_call_state` frame and is merged into the store without firing `watch` actions. |
| Visibility to agent | Never. It reaches the agent only inside `sendMessage.data` when the interface puts it there. |

## 4. Interface event (`sendMessage`)

Uses the existing client event (spec 019): `{ text, data?, source_call }`. `data` is whatever the action's `params.data` resolved to (state values, an item, a FEN). Gating: the action chain must start at a trusted DOM event; rate ≤ 1/s per interface; `text` ≤ 4000 chars; `data` ≤ 64 KB. Frame and server-side rate limits are unchanged.

## 5. Logic runner protocol (WebUI-internal)

```text
host → runner   { id, fn, args: any[], defs? }    defs sent once on creation: { name: { params, body } }
runner → host   { id, ok: true,  result }         result: JSON, ≤ 256 KB
              | { id, ok: false, error }          message only (no stack from host objects)
host timer      250 ms per call; on expiry: destroy + recreate runner, reject { code: "logic_timeout" }
```

## 6. Reference documents (generated, WebUI-internal)

| Output | Produced from | Returned by |
|---|---|---|
| Component index | catalogue `description` + group | `ui_components()` |
| Component detail | zod schema → compact signature (name, type, required, enum, default, events emitted, child rule) + `example` | `ui_components(names=[…])` |
| Recipe | a stored `UiSpec` (validated in CI) + a one-line intent | `ui_components(recipe=…)` |
| Error report | `validateSpec` issues + zod issues + nearest-name suggestions | thrown by `ui_render`, seen by the script |

## 7. Entities carried over unchanged

`client_tool_call` row (arguments, outcome, saved state), `SessionHistoryContent::ClientToolCall`, `ClientEvent` request, `DeferredClaims`, the per-call and per-conversation `RateLimiter`s. The legacy `render_interactive_ui` rows keep their meaning and renderer.
