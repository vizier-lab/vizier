# Contract: Agent Secret Tools

**Feature**: `027-agent-secrets`

This is what an agent sees. Tool names and result shapes are the contract; tool names are dispatch keys, so renaming one is a breaking change.

## Category

| Name | Description (as listed in `SANDBOX.md` and by `list_tool_functions`) |
|---|---|
| `secrets` | Credentials stored for this agent only; values are usable in scripts but never shown to the model |

It is the last built-in category, after `integrations`. Every agent has it and it needs no configuration. It is **not** offered to the dream cycle.

## Shared rules (in both tool descriptions)

- Secrets belong to this agent only. Another agent's name behaves exactly like a name that does not exist.
- Names are 1–128 characters of letters, digits, `-`, `_` and `.`, case-sensitive.
- Values are 8–65,536 bytes of text, stored exactly, including spaces and line breaks.
- Setting an existing name replaces its value; the old value is gone.
- `get_secret` returns the value **to the running script only**. It is never shown to the model or in the report: the report shows `[redacted]`. Use the returned variable by passing it to another tool, not by printing it.
- There is no list tool. The agent only needs names it was told.

## `set_secret`

Input:

```json
{ "name": "LINEAR_TOKEN", "value": "lin_api_…" }
```

Created:

```json
{ "status": "created", "name": "LINEAR_TOKEN" }
```

Replaced:

```json
{ "status": "replaced", "name": "LINEAR_TOKEN" }
```

Refused (nothing written):

```json
{ "status": "refused", "reason": "invalid_name", "name": "a/b",
  "message": "Secret names are 1–128 characters of letters, digits, '-', '_' and '.'." }
{ "status": "refused", "reason": "invalid_value", "name": "TOKEN",
  "message": "Secret values are 8–65,536 bytes of text." }
```

The tool registers a successfully stored value with the turn redactor before returning, so later prints/calls in the same script are scrubbed.

## `get_secret`

Input:

```json
{ "name": "LINEAR_TOKEN" }
```

Found (the script receives the real `value`; the saved/displayed report has `"[redacted]"`):

```json
{ "status": "found", "name": "LINEAR_TOKEN", "value": "lin_api_…" }
```

Not found (nothing created, no cross-agent hint):

```json
{ "status": "not_found", "name": "LINEAR_TOKEN" }
```

Refused name:

```json
{ "status": "refused", "reason": "invalid_name", "name": "a/b",
  "message": "Secret names are 1–128 characters of letters, digits, '-', '_' and '.'." }
```

The tool registers the value with the turn redactor before returning it to the script.

## Errors

Only a storage failure is an `Err` (a catchable Python exception). Invalid input, a missing name and a refused write are data results, so an unguarded script can continue and inspect `status`.

## Redaction contract

After `set_secret` or `get_secret` has armed the turn redactor:

- nested tool arguments recorded in `ExecutionReport.tool_calls[].arguments` show `[redacted]`;
- nested tool errors show `[redacted]`;
- `ExecutionReport.result`, `stdout`, error message and traceback show `[redacted]`;
- the bridge's log line and the `execute_tool` telemetry show `[redacted]`;
- the saved report/history and the WebUI show `[redacted]`.

The following are outside the guarantee: the outer `execute_python` code that called `set_secret` (the supplying input), and any transformed form of the value the script creates (base64, URL-encoding, hashing).

## From a script (code mode)

```python
token = get_secret(name="LINEAR_TOKEN")
if token["status"] == "found":
    answer = http_client(
        url="https://api.linear.app/graphql",
        method="POST",
        headers={"Authorization": f"Bearer {token['value']}"},
        body='{"query":"{ viewer { name } }"}',
    )
    print(answer["status"])
```

The HTTP request uses the real token. The report shows `"value": "[redacted]"`, the `http_client` arguments show `"Authorization": "Bearer [redacted]"`, and nothing in the report names the token.

## Category table / dream test

`src/agents/tools/mod.rs::every_tool_is_in_its_category` must include both names under `Secrets`. A test must assert neither name is in `VizierTools::DREAM_TOOL_NAMES`.
