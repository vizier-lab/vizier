# Contract: Scratchpad Agent Surface

**Feature**: `020-scratchpad-tools`

This is what an agent sees. Tool names and result shapes are the contract. Tool names are dispatch keys, so renaming one is a breaking change.

## Category

| Name | Description (as listed in `SANDBOX.md` and by `list_tool_functions`) |
|---|---|
| `scratchpad` | Temporary notes private to this conversation, deleted once it goes idle |

It is listed after `files`. Every agent has it, and it needs no configuration. It is also offered directly to the dream cycle.

## Shared rules (in every tool description)

- A scratch file is plain text, private to this conversation, and shared by every turn of it while turns keep coming back to back.
- All scratch files are deleted once the conversation has nothing running or queued, and on server restart. Use memory for anything that must last. Nothing written here reaches a later background-job report.
- Names: 1–128 characters of letters, digits, `-`, `_` and `.`. Limits: 1 MiB per file, 64 files per conversation.

## `write_tmp`

Input: `{ "name": string, "content": string }`. Creates the file or replaces all of its content.

## `append_tmp`

Input: `{ "name": string, "content": string }`. Adds `content` to the end exactly as given (no separator), creating the file if absent.

### Write/append result

```json
{ "status": "written", "name": "plan", "size": 42 }
```

```json
{ "status": "refused", "reason": "invalid_name", "name": "../x",
  "message": "Scratch file names are 1–128 characters of letters, digits, '-', '_' and '.'." }
{ "status": "refused", "reason": "too_large", "name": "log", "limit": 1048576, "current": 1048000,
  "message": "…would make 'log' larger than 1 MiB; it is unchanged." }
{ "status": "refused", "reason": "too_many", "name": "extra", "limit": 64, "existing": ["a", "b", "…"],
  "message": "This conversation already has 64 scratch files; write to an existing one." }
```

## `read_tmp`

Input: `{ "name": string }`.

```json
{ "status": "found", "name": "plan", "size": 42, "content": "…" }
{ "status": "not_found", "name": "notes", "existing": ["log", "plan"] }
```

An invalid name returns the `refused` / `invalid_name` shape above.

## Errors

Only a storage failure is an error: the script sees a catchable exception, and a dream sees a tool error. A refusal or a missing file is never an error (FR-012).

## From a script (code mode)

```python
write_tmp(name="plan", content="1. search\n")
append_tmp(name="plan", content="2. summarise\n")
r = read_tmp(name="plan")
if r["status"] == "found":
    print(r["content"])
```
