# Contract: dummyplug additions for steering tests

Additive to `specs/008-dummyplug-provider/contracts/dummyplug-protocol.md`; every existing script keeps working.

## §3 addition — `delay_ms`

```json
{ "tool": "current_time", "arguments": {}, "delay_ms": 1500 }
```

- Optional integer, 0–10 000 (default 0). Dummyplug waits this long **before** answering with the tool call.
- Out of range or not an integer → `Invalid tool request: delay_ms must be an integer from 0 to 10000`.
- Purpose: a window in which a test sends a second message while the turn is busy.
- `tool` becomes optional **when `delay_ms` is present**: `{ "say": "done", "delay_ms": 2000 }` waits, then answers with `say` as the turn's final text and no tool call — a busy turn with no round boundary (the fallback test). Without `delay_ms`, a missing `tool` stays an `Invalid tool request` as today.

## §8 — `Read mid-turn` echo

When a request's messages after the **last tool result** contain user text (the turn was steered), the reply is:

```text
**Read mid-turn**:
<text of the first such user message, frontmatter removed>
<text of the next one>
```

instead of the §4 tool-result echo. A background delivery's text in the same user message is listed too, prefixed `background:`, and a silent read (frontmatter `addressed: false`) is prefixed `silent:`. Frontmatter is removed by cutting at the closing `---`; `via: mid_turn` lines are therefore not shown, but the §6 `context` command still echoes the context block.

With no user text after the last tool result, §4 applies unchanged.
