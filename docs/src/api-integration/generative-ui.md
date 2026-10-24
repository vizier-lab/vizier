# Generative UI

In the WebUI, an agent can answer with an **interface** instead of (or beside) text: a dashboard, a form, a chart, a chess board, a 3D terrain you can orbit. It is built from a catalogue of ready-made components, drawn in the WebUI's own look, and it stays in the conversation.

This is a [client tool](./client-tools.md) the WebUI registers: the agent calls `ui_render` from a Python script, and the call **ends its turn**: the interface is the reply. Other clients do not offer it, so an agent talking to you from Discord or Telegram never sees it.

## What an agent can build

| Group | Components |
|---|---|
| Layout | `Stack`, `Row`, `Grid`, `Card`, `Tabs`, `Fold`, `Divider` |
| Text | `Heading`, `Text` (a markdown subset), `Badge`, `Callout`, `Code` |
| Data | `Stat`, `Table`, `KeyValue`, `Progress`, `Chart` (bar, line, area, pie) |
| Links and media | `Link`, `Image`, `Audio`, `Video`, `FileCard`, `Gallery` |
| Input | `TextInput`, `NumberInput`, `Slider`, `Select`, `Checkbox`, `Switch`, `RadioGroup` |
| Action | `Button` |
| Surface | `Canvas2D`, `Scene3D` with `Box`, `Sphere`, `Cylinder`, `Plane`, `Group3D`, lights, `OrbitControls`, `Heightfield`, `Mesh` |
| Control | `Effect` (runs actions on load and on a timer) |

There is no free-form HTML, CSS or script in the interface itself: props are closed lists (`gap: sm`, `tone: ok`, colours from the theme). That keeps every interface in the WebUI's look, in light and dark, and keeps agent text from restyling or scripting the page.

## How an agent learns it

Without a skill and without anything in its standing instructions. The agent sees a one-line `ui` category, then:

1. `list_tool_functions(category="ui")` shows `ui_components` and `ui_render`.
2. `describe_tool_function("ui_render")` is a short cheat sheet of the spec's shape.
3. `ui_components()` is an index of every component; `ui_components(names=["Table", "Chart"])` gives exact props, events and an example for each; `ui_components(recipe="board_game")` returns a complete interface to adapt. The text is generated from the same definitions that validate and draw, so it cannot drift.
4. A mistake in `ui_render` returns **every** problem at once, each with its JSON path, the valid options and the nearest name, and the turn continues so the agent can fix it.

Recipes: `dashboard`, `form`, `tabs_and_filter`, `board_game`, `scene_3d`, `canvas_2d`.

## An interface is alive

The spec holds elements, an initial `state` and, optionally, `logic`.

- **Local interaction.** Inputs write to state, tabs and toggles change what is visible, a `Button` runs actions (`setState`, `pushState`, `removeState`, `validateForm`). None of it costs an agent turn.
- **Expressions.** Any prop can be a value or an expression: `{"$state": "/a/b"}`, `{"$bindState": "/a/b"}` (two-way, for inputs), `{"$template": "Hi ${/name}"}`, `{"$cond": …}`, `{"$computed": "add", "args": {…}}`, and inside a `repeat` `{"$item": "field"}` and `{"$index": true}`. The pure `$computed` functions are `add sub mul div mod min max abs round floor eq not and or len at concat join slice includes range`.
- **Logic.** `logic` holds small pure JavaScript functions (`{"step": {"params": ["game", "index"], "body": "…"}}`) for rules and maths: legal chess moves, terrain noise. An action `run {fn, args, into}` calls one and writes the result to state. Functions run locally in a sandbox with no network, no page access and a 250 ms limit; a function that never returns is stopped and shown as an inline error while the page stays responsive.
- **Watching.** `watch` runs actions when a state path changes (a slider regenerating terrain); `Effect` runs them on load and on a timer (paused off screen and in a hidden tab, stopped after ten minutes without the person touching anything).
- **Asking the agent.** `sendMessage {text, data}` posts a message as the person and starts a turn. It only works from an action chain that began at the person's own click or key press, at most once a second; timers, watches and logic functions can never send.
- **Saved state.** State is saved with the conversation (top-level keys starting with `_` are derived and not saved), so the interface survives a reload and shows the same in another tab.

## Files and links

Media components refer to the conversation's **session files by name** (what `list_session_files` returns): `{"type": "Image", "props": {"file": "forecast.png", "alt": "Forecast"}}`. The WebUI resolves the name when it draws, so a file made later appears, and a deleted one shows "File not available" in that spot only. `ui_render` checks the names when it is called and answers with the names and types of the files that exist. Audio and video never play by themselves. `Image` also takes a small `data:image/` graphic; external addresses are not loaded.

Links (`Link`, and links in `Text`) accept `https`, `http` and `mailto` only. They open in a new tab without an opener, and the real host is shown beside a label that does not say it, so a label cannot hide the destination.

## Limits

| What | Limit |
|---|---|
| Interface size, elements, depth | 256 KB, 1500, 16 |
| Items drawn by one `repeat` | 2048 |
| Logic source, functions, params per function | 64 KB, 32, 8 |
| One logic call | 250 ms, 256 KB in and out |
| `Heightfield` resolution, `Canvas2D` shapes | 192, 5000 |
| Saved state | 64 KB (excluding `_` keys) |
| `sendMessage` | once a second; text 4000 characters; data 64 KB |

## For client authors

`ui_render` is declared with `ends_turn: true`: see [Tools that end the turn](./client-tools.md#3a-tools-that-end-the-turn). A client that wants the same behaviour for its own tool sets that flag; nothing about generative UI is special to the server.

Interfaces drawn by the earlier `render_interactive_ui` (a hand-written HTML page in a sandboxed frame) still open from history with their saved state. That tool is no longer offered to agents.
