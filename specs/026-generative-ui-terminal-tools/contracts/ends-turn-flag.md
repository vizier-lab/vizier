# Contract: the `ends_turn` flag

**Feature**: `026-generative-ui-terminal-tools` · extends `docs/src/api-integration/client-tools.md` (protocol version stays 1: one optional field)

## 1. Declaring (client tools)

`ClientToolDeclaration` gains one optional field:

```json
{
  "name": "render_ui",
  "category": "ui",
  "description": "…the author's own text…",
  "input_schema": { "type": "object" },
  "ends_turn": true
}
```

| Rule | Behaviour |
|---|---|
| Absent or `false` | Exactly as before this feature. |
| Not a boolean | Refused with `client_error` `invalid_client_tools`, rule `ends_turn_type`. |
| Size check | The 2048-byte description limit applies to the author's `description` only. |
| With `deferrable: true` | Accepted, but a `{"status": "deferred"}` outcome never ends the turn (it is not a result). The WebUI's generative UI tool is not deferrable. |
| In `client_tools_resolved` | Each resolved tool echoes `ends_turn`, so a client can show it. Clients that ignore unknown fields are unaffected. |

## 2. What the agent sees

For every flagged tool, **wherever its description is shown** (`list_tool_functions` summary is the first sentence only, unchanged; `describe_tool_function`, the in-script `describe_tool()`, the dummyplug `tools` listing), the description ends with exactly one copy of:

> Calling this ends your turn: its result is your final answer, so finish all other work first and expect no further round. If the call fails you get the error and the turn continues.

Order for a client tool: `(Provided by the person's client.) <author text> <notice>`. The standing instructions (`SANDBOX.md`) never mention individual tools or the flag.

## 3. Semantics

A turn **ends** after a round when, in that round's `execute_python` report:

1. `report.ok == true` (the script finished without error), **and**
2. at least one `tool_calls[i]` has `ok == true` and `ends_turn == true`.

Then:

- The round's tool results are saved to history as usual.
- No further model completion is requested for the turn.
- The turn's reply is a message whose text is the text the model wrote in that step (may be empty); its stats carry `ended_by_tool` (the final function name).
- Any `print()` output and the script's other results are in the saved report, not in the reply text.

A turn **continues** (the agent gets the report and another round) when the flagged call failed, was refused, timed out, was cancelled, returned `deferred`, or when the script failed afterwards. If one script calls several flagged tools, the turn ends after the script (all calls complete).

Not affected: dream cycle, checkpoint handovers, scheduled runs, background pieces and reports, inter-agent turns: none has client tools; flagged **native** tools follow the same rule wherever they are offered. `think`, `list_tool_functions`, `describe_tool_function` and `execute_python` themselves are never flagged.

## 4. Native tools and integrations

`VizierTool::ends_turn() -> bool` (default `false`). An override makes the registered tool's `CatalogueEntry.ends_turn` true. Addon tools are the same. MCP tools cannot be flagged.

## 5. Observability

The `execute_tool` span of a flagged call gains `vizier.tool.ends_turn = true`; the `invoke_agent` span gains `vizier.turn.ended_by_tool = <name>` when the turn ended through one.

## 6. Documentation change

`docs/src/api-integration/client-tools.md` §1 (declaration table) and §3 (calls) gain the field, the notice and these semantics; the changelog notes "optional `ends_turn` on declarations (version 1)".
