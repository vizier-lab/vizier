# Contract: the generative UI tools (agent-facing)

**Feature**: `026-generative-ui-terminal-tools`

The WebUI declares one category and two tools on every request, through `registerClientCategory` / `registerClientTool`. They are client tools (spec 019), so the agent reaches them from `execute_python` scripts as `ui_components(…)` and `ui_render(…)`.

## 1. Discovery path (no skill)

```text
system prompt (SANDBOX.md, byte-stable)
  └─ category line:  ui — (provided by the person's client) Show an interface made of ready-made
                          components (stats, tables, charts, forms, 3D). Start with ui_components.
list_tool_functions(category="ui")
  ├─ ui_components — Look up the components, props and ready-made examples for ui_render.
  └─ ui_render — Show an interface in the person's reply. Ends your turn.
describe_tool_function("ui_render")       → cheat sheet (§3) + input schema (§4) + the ends-turn notice
ui_components()                           → index of every component (≈1.5 KB)
ui_components(names=["Table","Chart"])    → exact props, events, example per component
ui_components(recipe="board_game")        → a complete valid spec to adapt
ui_render(...) fails                      → every problem listed with the valid options
```

An agent that already knows the shape can call `ui_render` directly; the errors teach the rest.

## 2. `ui_components` (read-only, not turn-ending, not deferrable)

**Input**

```json
{
  "type": "object",
  "properties": {
    "names":  { "type": "array", "items": { "type": "string" }, "description": "Component names for full detail" },
    "group":  { "type": "string", "enum": ["layout","text","data","media","input","action","surface","control"] },
    "recipe": { "type": "string", "enum": ["dashboard","form","tabs_and_filter","board_game","scene_3d","canvas_2d"] }
  }
}
```

At most one of `names`, `group`, `recipe` is meaningful; none returns the index.

**Output**

| Call | Result |
|---|---|
| none | `{ "components": [{ "name", "group", "summary" }…], "actions": [{ "name", "summary" }…], "functions": ["add","sub",…], "next": "ui_components(names=[…]) for props; ui_components(recipe=…) for full examples" }` |
| `group` | the index narrowed to that group |
| `names` | `{ "components": [{ "name", "group", "summary", "props": { "<prop>": "<type> (required|optional) [= default] — <note>" }, "children": "none | any | slot names", "events": ["press","change",…], "example": { …a minimal valid element map… } }…], "unknown": [{ "name", "did_you_mean": [..] }] }` |
| `recipe` | `{ "recipe", "intent": "<one line>", "notes": ["<what to adapt>"…], "spec": { …a complete valid UiSpec… } }` |

Size of any one result ≤ 24 KB (the answer limit is 1 MB; this keeps the agent's context small). The text is generated from the catalogue definitions (spec FR-003a).

## 3. `ui_render` description (the cheat sheet; budget 1.8 KB, hard cap 2048)

Draft, to be finalised against the byte cap in a test:

```text
Show an interface in your reply: ui_render(title=…, spec=…). It is your final answer (see below).
Only catalogue components work; look them up with ui_components(), ui_components(names=[…]) for props, or ui_components(recipe=…) for full examples.
spec = {"root": "id", "elements": {"id": {"type": "Card", "props": {…}, "children": ["id2"]}}, "state": {…}, "logic": {…}}.
Props can be values or expressions: {"$state":"/a/b"} read, {"$bindState":"/a/b"} read+write (inputs), {"$template":"Hi ${/name}"}, {"$cond":{"$state":"/x","eq":1},"$then":…,"$else":…}, {"$computed":"add","args":{"a":…,"b":…}}. Inside a repeat: {"$item":"field"}, {"$index":true}.
Put visible, repeat, on, watch beside props on the element, never inside props.
Events: "on": {"press": {"action":"setState","params":{"statePath":"/tab","value":"b"}}}. Actions: setState, pushState, removeState, validateForm, sendMessage {text,data} (sends a message as the person), run {fn,args,into} (calls a function from "logic").
"logic" holds small pure JS functions {"name": {"params":["a"],"body":"return …"}} for rules and maths; they run locally and cannot use the network or the page. Keys in state starting with "_" are derived and not saved.
Show files of this conversation with Image/Audio/Video/FileCard/Gallery by file name (see list_session_files); make the file first. Link takes https/http/mailto only.
Limits: 256 KB, 1500 elements, depth 16. A mistake returns every problem with the valid options; fix and call again.
Returns {"ui_id","status":"rendered"}. The person sees it after your turn; do all other work first.
```

(Then the standard ends-turn notice, added by vizier.)

## 4. `ui_render` input schema (≈1 KB, well under 16 KB)

```json
{
  "type": "object",
  "required": ["title", "spec"],
  "properties": {
    "title": { "type": "string", "maxLength": 120, "description": "Short title for screen readers and the collapsed view" },
    "spec": {
      "type": "object",
      "required": ["root", "elements"],
      "properties": {
        "root": { "type": "string" },
        "elements": {
          "type": "object",
          "additionalProperties": {
            "type": "object",
            "required": ["type"],
            "properties": {
              "type": { "enum": ["Stack", "Row", "Grid", "…every catalogue name…"] },
              "props": { "type": "object" },
              "children": { "type": "array", "items": { "type": "string" } },
              "visible": {}, "repeat": { "type": "object" }, "on": { "type": "object" }, "watch": { "type": "object" }
            }
          }
        },
        "state": { "type": "object" },
        "logic": { "type": "object" }
      }
    }
  }
}
```

The `type` enum is generated from the catalogue, so adding a component updates it. Per-component props are *not* in the schema: they are in `ui_components` (R5).

## 5. Results and errors

**Success** (turn ends): `{ "ui_id": "<call id>", "status": "rendered", "fixed": ["moved 'visible' out of props on 'row-3'"]? }`. The `ui_id` is the history uid of the call; the interface is drawn from the call's arguments.

**Failure** (turn continues): the tool answers with an error; the script gets a catchable failure whose message is:

```text
ui_render refused the interface (4 problems):
1. elements.chart-1.type: "PieChart" is not a component. Valid: Chart, … Did you mean "Chart" (props.kind "pie")?
2. elements.btn.props.variant: "big" is not allowed. Valid: primary, secondary, ghost.
3. elements.root.children[2]: "stats-2" has no definition in elements.
4. elements.photo.props.file: "cat.png" is not a file of this conversation. Available: forecast.png (image/png), report.pdf (application/pdf). Generate or load the file first (see list_session_files).
Fix these and call ui_render again. ui_components(names=["Chart","Button"]) shows exact props.
```

Rules: all issues in one message, ≤ 20 shown (then "and N more"), each with a JSON path, the problem, valid options when finite, the nearest names when it is a typo. Never echo the whole spec back.

Other failures: `too_large` (names the limit and the size), `client_disconnected`/timeout (standard client tool errors; the turn continues, so the agent can answer in text).

## 6. Rendering rules (what the person gets)

- Drawn inside the reply bubble of the turn, above the reply text, once the turn has ended; the last successful `ui_render` of a turn wins (a turn can only contain one, since the first success ends it).
- Light/dark and density follow the app; a re-theme applies live.
- A component that throws while drawing shows an inline error chip for that component only.
- Keyboard operable; inputs labelled by their `label` prop; the title is the region's accessible name.
- Presses/changes run local actions immediately; `sendMessage` posts as the person's own message after trust and rate checks, and shows the standard short "sent" notice (spec 019 FR-028a).
- Expand-to-chat-area and download controls from 019 are kept; the download exports the spec as `.json`.

## 7. Retired

`ui_render_interactive_ui` is no longer declared. A script that calls it gets the retired-name hint: `render_interactive_ui was replaced by ui_render. Start with ui_components().` Saved pages from it still draw (US6).
