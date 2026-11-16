# Contract: the `ui_render` tool

Native tool. Category `ui` (23rd `ToolCategory`). Turn-ending (`ends_turn() == true`). It is offered only when the conversation is `HTTP` or `Project` and the context is not detached (research R9). It is reached from `execute_python` like every tool.

## Description (fixed, every turn)

> Draw an interactive HTML page in your reply: anything a web page can be (apps, games, simulations, dashboards). The page starts with the Vizier kit (theme, base styles, `<vz-*>` blocks, bundled libraries). Before your first page in a conversation, call `describe_tool_function("ui_render")` and read its `guide`. To change a page, send `edits` against its latest version instead of rewriting it. A successful call is your final answer.

The description is followed by `ENDS_TURN_NOTICE`, added where the definition is built.

## Input (exactly one shape)

```jsonc
// 1. New page
{ "title": "Chess", "html": "<!doctype html>…" }

// 2. Full replacement of a page you made in this conversation
{ "page": "pg_…", "base_version": 3, "html": "…", "title": "Chess (wooden)" /* optional */ }

// 3. Edit set
{ "page": "pg_…", "base_version": 3,
  "edits": [ { "find": "background: #eee", "replace": "background: var(--vz-wood, #a0703c)" },
             { "find": "</aside>", "replace": "<ol id=\"moves\"></ol></aside>" } ] }
```

- Edits apply in order, each to the text the previous one produced.
- Each `find` must occur exactly once at that point.

## Output on success (`Ok`, ends the turn)

```json
{ "status": "rendered", "page": "pg_…", "version": 4, "verified": true }
{ "status": "rendered", "page": "pg_…", "version": 4, "verified": false, "reason": "no_tab" }
```

## Errors (`Err`; the turn continues; the script may catch them)

The message is a JSON object. `code` is one of:

| code | When | Extra fields |
|---|---|---|
| `invalid_input` | not exactly one shape, missing or extra fields | `problems: [..]` |
| `too_large` | version > 1 MiB | `size`, `limit` |
| `unknown_page` | no such page for this agent | — |
| `other_conversation` | page made in another conversation | — |
| `stale_base` | `base_version` ≠ latest | `latest` |
| `edit_not_found` | an edit's `find` occurs 0 times | `edit` (index), `find_preview` |
| `edit_ambiguous` | an edit's `find` occurs > 1 times | `edit`, `count` |
| `problems` | **saved**, but verification found problems | `page`, `version`, `problems: [Problem]` |

Every error except `problems` saves nothing.

## Guide (`describe_tool_function("ui_render").guide`)

The guide is `crates/vizier-core/assets/ui-kit/REFERENCE.md`. It contains:
1. The page model: versions, edits, state across versions.
2. Rules: no network from script; `https` images, audio and video allowed; drive loops with `requestAnimationFrame`; put anything that can run longer than about 50 ms in `vizier.worker`; size limits.
3. Kit CSS: tokens and classes.
4. Every `<vz-*>` element with its attributes, events and an example.
5. Bridge v1 (`contracts/bridge.md`).
6. Libraries and their import names (`import * as THREE from "three"`).
7. Four short examples: a dashboard, a canvas game loop, a form that sends, and a game opponent searching in `vizier.worker`.

`scripts/ui-kit-reference.test.mjs` keeps the guide in step with the kit's manifest.

## Retired names

`ui_components` and `render_interactive_ui` are in `RETIRED_TOOL_NAMES`, with the answer: *"Retired. Use `ui_render`; its guide is in `describe_tool_function("ui_render")`."*
