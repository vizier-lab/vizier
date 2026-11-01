# Contract — Agent surface: `ask_question` and `ask_sensitive_input`

**Feature**: `042-agent-asking-question`

This is what the agent sees and what a script gets back. The two functions are ordinary
script-callable tools (`list_tool_functions`, `describe_tool_function`), so they need no new
discovery mechanism. Both are **incomplete in an unattended turn**: they are absent from the
catalogue, from the category lines, and a direct call fails (FR-007, FR-046, FR-047).

## 1. `ask_question`

Category `questions`. Ends the turn (`ends_turn() == true`), so `ENDS_TURN_NOTICE` is appended
exactly once by `with_ends_turn_notice`, and the notice does not count against the description
budget (FR-009).

### Input

```json
{
  "title": "Project setup",
  "description": "Three quick choices before I scaffold.",
  "questions": [
    { "id": "name", "prompt": "Project name", "type": "text", "max_length": 60 },
    { "id": "db", "prompt": "Database", "type": "choice",
      "options": [ { "value": "postgres", "label": "PostgreSQL" },
                   { "value": "sqlite",   "label": "SQLite" } ],
      "default": "sqlite" },
    { "id": "features", "prompt": "Features", "type": "multi_choice",
      "options": [ { "value": "auth", "label": "Auth" }, { "value": "ci", "label": "CI" } ],
      "min_selections": 0, "max_selections": 2, "optional": true },
    { "id": "confirm", "prompt": "Proceed?", "type": "confirm" }
  ],
  "submit_label": "Create",
  "dismiss_label": "Cancel"
}
```

| Field | Required | Rules (FR-002, FR-003, FR-060) |
|---|---|---|
| `title` | no | ≤ 200 bytes |
| `description` | no | ≤ 1 000 bytes |
| `questions` | yes | 1–10 |
| `submit_label`, `dismiss_label` | no | ≤ 100 bytes; surfaces substitute their own default |
| `questions[].id` | yes | 1–64 bytes, `^[a-z0-9][a-z0-9_-]*$`, unique in the set |
| `questions[].prompt` | yes | 1–500 bytes |
| `questions[].help` | no | ≤ 500 bytes |
| `questions[].optional` | no | default `false` |
| `questions[].default` | no | valid for the type |
| `questions[].type` | yes | `text`, `choice`, `multi_choice`, `confirm` |
| `options[].value` | yes | 1–100 bytes, unique per question |
| `options[].label` | yes | 1–100 bytes |
| `options[].description` | no | ≤ 200 bytes |

A set is refused **before anything is shown**, naming every problem at once (FR-005):
`no_questions`, `too_many_questions`, `duplicate_question_id`, `empty_prompt`,
`prompt_too_long`, `no_options`, `too_many_options`, `duplicate_option_value`,
`default_not_an_option`, `selection_bounds`, `text_limit`, `payload_too_large`,
`invalid_question_id`, `unsupported_type`. A refusal is an `Err`, the script can catch it, and
the turn **does not end**.

### Output

```json
{ "status": "pending", "question_set_id": "6f1c0d6e-…",
  "questions": [ { "id": "name", "type": "text" }, { "id": "db", "type": "choice" } ] }
```

The output says what was asked and nothing else — the answers arrive as the person's next
message (see `questions-wire.md` §4). Nothing in it is secret.

### What its description must say (FR-009)

Purpose; the four types; the 10-question limit; that success ends the turn; the shape of the
answer the agent will receive (`status`, `question_set_id`, then a `question_answer` request);
the byte limits of §1; that it is unavailable in unattended turns; that credentials and other
sensitive values must go through `ask_sensitive_input` instead (FR-066); and that a script that
fails after asking withdraws the question (FR-010), so ask last.

## 2. `ask_sensitive_input`

Category `secrets`, **not** turn-ending. Available only when the conversation is a WebUI topic
or a project chat (`VizierChannelId::web_user().is_some()`), and only in an interactive turn.

### Input

```json
{ "purpose": "A GitHub token with `repo` scope, to file the issue you asked for.",
  "label": "GitHub token" }
```

| Field | Required | Rules |
|---|---|---|
| `purpose` | yes | 1–1 000 bytes; shown to the person; **not** secret |
| `label` | no | ≤ 100 bytes; field name |

### Output

```json
{ "status": "provided", "value": "<the value>", "protected": true }
{ "status": "declined" }
{ "status": "timeout" }
```

`status` is one of `provided`, `declined`, `timeout` — never an exception for those three, so a
script branches instead of failing (FR-050, FR-051, SC-016). A value shorter than
`MIN_VALUE_BYTES` (8) is delivered with `protected: false` (FR-043, R10 of research).

### Refusals (an `Err` the script can catch, the turn continues)

| Situation | Message names |
|---|---|
| not a WebUI conversation | "this conversation is on {platform}; a secure input is only available in the WebUI" |
| unattended turn | "nobody can answer here: this turn is unattended" |
| empty `purpose` | the field and the limit |
| another ask is open | waits for it to close (FR-053), never a second dialog |

### What its description must say (FR-040)

The value goes to the script only; the agent never sees it; the call does not end the turn; the
person may reject, dismiss or let it time out; the 8–65 536-byte limits; redaction is
exact-string only, so a transformed form is outside the guarantee; a value written to a file,
memory or an outbound body is outside the guarantee; where it is unavailable; that it does not
store anything (use `set_secret` for that, FR-045); and that a masked dialog is not the place
for ordinary text (FR-066).

## 3. Shared rules

- **One set / one ask per turn**: a second successful `ask_question` in the same turn is
  refused (FR-008); a second `ask_sensitive_input` in the same turn is allowed once the first
  ask closed (FR-052).
- **The turn is the boundary**: `ask_question` ends the turn only when the whole script
  completed and the call itself succeeded (`ExecutionReport::ended_turn`); `ask_sensitive_input`
  never ends it.
- **Calling them directly**: neither is a sandbox tool, so the model reaches them only from
  `execute_python`; a direct call gets `… is not called directly; call it from an
  execute_python script` (existing behaviour).
