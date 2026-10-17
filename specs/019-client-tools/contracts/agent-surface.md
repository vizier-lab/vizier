# Contract: What the Agent Sees

**Feature**: `019-client-tools`

## SANDBOX.md category lines

Client categories follow the built-in, addon and MCP lines, in declaration order:

```
## Tool categories

- memory: Search, read, write and organise long-term memory
…
- github: Tools from the github MCP server
- ui: (provided by the person's client) Show interactive pages in the conversation
- palette: (provided by the person's client) Colour pickers shown to the person
```

A category with no declared description reads `(provided by the person's client) Tools provided by the person's client`. With no client tools, the list is exactly what it is today.

## Listing and describing

- `list_tool_functions(category="ui")` → `{ category: "ui", count: 1, functions: [{ name: "ui_render_interactive_ui", summary }] }`.
- `list_tool_functions()` includes client functions with their category.
- `describe_tool_function(name="ui_render_interactive_ui")` → the usual `ToolFunctionDoc`, built from the client's `input_schema` (and `output_schema` when declared), with the description prefixed `(Provided by the person's client.) `.
- In-script `list_tools(category=…)` and `describe_tool(…)` behave the same way.
- Calling a client function from a script that has none (any turn without client tools) is the usual unknown-function `NameError`. `list_tool_functions(category="ui")` returns the usual `{available: false, did_you_mean}`.

## Calling

`ui_render_interactive_ui(title="Task runs", html="<!doctype html>…")` returns `{"ui_id": "<call id>", "status": "rendered"}`. A client error, a disconnect (`client_disconnected`) or a timeout raises `RuntimeError("ui_render_interactive_ui: …")`, which the script may catch.

In the `execute_python` report the agent gets back, the call appears in `tool_calls` like any other nested call. The model's view already drops nested `arguments`, so the page HTML is not echoed back to the model. The stored report keeps a reference instead of the HTML (`{"client_call": "<call id>"}`), so a page is not stored twice.

## A client event, as the agent receives it

```
---
sender: alice
via: client_event
source_call: 6f1c0d6e-…
metadata: {}
---

Submitted filters

```json
{"quarter": 3}
```
```

## Background report framing

A `BackgroundReport` turn's text gains one closing line:

> Client tools (such as pages shown in the WebUI) are not available in this turn. If a visual would help, offer it and show it when the person next writes.
