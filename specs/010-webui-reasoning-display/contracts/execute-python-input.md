# Contract: `execute_python` input

**Feature**: `010-webui-reasoning-display` · **Breaking change**

## Before

```rust
pub struct ExecutePythonInput {
    pub code: String,
}
```

## After

```rust
pub struct ExecutePythonInput {
    #[schemars(
        description = "Python source to run. The value of the last expression is returned as `result`."
    )]
    pub code: String,

    #[schemars(
        description = "One short sentence, in plain language, saying what this script is for. \
                       It is shown to the person watching in place of the code — write it for \
                       them, not for yourself. E.g. \"compute the 99th percentile from the last \
                       24h of latency samples\"."
    )]
    pub intent: String,
}
```

`intent` is `String`, not `Option<String>`: a call omitting it fails deserialization and the error
returns to the agent, which may retry (FR-025).

## Guarantees

| # | Guarantee | Requirement |
|---|---|---|
| C1 | `intent` appears under `Required:` in the tool's schema, alongside `code`. | FR-024 |
| C2 | A call with both fields runs exactly as it does today. Behaviour, limits and the returned report are unchanged. | FR-038 |
| C3 | A call omitting `intent` does **not** execute. The agent receives an error naming the missing field. | FR-025 |
| C4 | `intent` is never passed to the sandbox, never bound as a script variable, and never affects limits, tool availability or the script's result. | Assumptions |
| C5 | `ExecutionReport` is unchanged — no `intent` field. The intent is already persisted in the tool call's arguments. | research D6 |
| C6 | The description tells the agent the intent is for the person watching. | FR-026 |

## What C4 means concretely

The sandbox receives `args.code` and nothing else. `intent` is read only by whatever renders the call:
it is in the `ToolCall { arguments }` history entry and in the live `ToolChoice { args }` frame.

## Breaking-change surface

Any caller that constructs this input by hand starts failing until it adds the field:

- the HTTP API, if a client issues a tool call directly
- a stored prompt or skill that hand-writes an `execute_python` call
- the dummyplug sample-request flow — the sample JSON now includes `intent`, so an operator
  round-tripping it unchanged still works (dummyplug fills every required field)

Agents adapt without intervention, because the tool definition they are handed declares the field.

## Verification

- Dummyplug §2: send `execute_python`, confirm `intent` is listed under `Required:` and appears in the
  sample JSON block (C1).
- Dummyplug §3: send the sample JSON unchanged → runs (C2). Send it with `intent` removed → error
  naming the field, no execution (C3).
