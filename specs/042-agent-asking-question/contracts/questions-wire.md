# Contract — Question wire: WebSocket frames, the answer request, the history entry

**Feature**: `042-agent-asking-question`

This extends the topic WebSocket protocol (spec 019's contract, §9 versioning) **additively**.
A client MUST ignore frames it does not recognise, so an older WebUI keeps working: it simply
never draws a card, and a pending set is answered from a newer client or goes unanswered.
No new REST endpoint is introduced.

```
ws://<host>/api/v1/agents/{agent_id}/channel/{channel_id}/topic/{topic_id}/chat?token=<jwt>
```

## 1. Server → client: a question set appears

Published as a normal history entry, so it survives a reload and arrives by the same path as
every other turn step (`SessionFrame::HistoryEntry`):

```json
{ "history_entry": {
    "uid": "6f1c0d6e-…",
    "vizier_session": { … },
    "content": { "QuestionSet": {
        "set_id": "6f1c0d6e-…",
        "title": "Project setup",
        "description": "Three quick choices before I scaffold.",
        "questions": [ { "id": "db", "prompt": "Database", "type": "choice",
                          "options": [ { "value": "postgres", "label": "PostgreSQL" } ],
                          "optional": false } ],
        "submit_label": "Create",
        "dismiss_label": "Cancel",
        "state": "pending",
        "answers": null,
        "answerer": null } },
    "timestamp": "…",
    "seq": 1234 } }
```

`state` becomes `answered`, `declined` or `withdrawn` later, without the entry being rewritten
in storage: the read path fills it from `question_set` (data-model.md §6). Two more frames
carry the change live:

```json
{ "question_set": { "set_id": "6f1c0d6e-…", "state": "answered",
    "answers": [ { "question_id": "db", "state": "answered", "value": "sqlite" } ],
    "answerer": "alice" } }
```

`answered` is also sent to the same socket as its own confirmation.

## 2. Client → server: the answer (a normal request)

The card submits one ordinary request whose content is the structured answer. It goes through
`TopicConnection::handle_inbound` exactly like a `chat`: same principal, same queue, same
history, same attribution (FR-015).

```json
{ "timestamp": "2026-11-01T10:31:00Z",
  "user": "alice",
  "content": { "question_answer": {
    "set_id": "6f1c0d6e-…",
    "text": "Database: SQLite",
    "answers": [ { "question_id": "db", "state": "answered", "value": "sqlite" } ],
    "answerer": "alice",
    "declined": false } },
  "metadata": {} }
```

| Field | Required | Rules |
|-------|----------|-------|
| `set_id` | yes | a set in **this** conversation; otherwise refused |
| `answers` | yes unless `declined` | every question exactly once; validated against the set |
| `text` | no | server-generated summary when absent |
| `answerer` | no | `metadata.user` / the authenticated principal is authoritative |
| `declined` | no | `true` for a dismissal; `answers` must then be empty |

Refusals are `client_error` frames, sent to the submitting socket only and starting no turn:

```json
{ "client_error": { "code": "question_not_open", "set_id": "6f1c0d6e-…",
    "message": "this question set was already answered" } }
```

| `code` | Sent when |
|--------|-----------|
| `question_not_open` | the set is answered, declined, withdrawn, or belongs to another conversation |
| `invalid_answer` | a required question unanswered, a value not offered, bounds broken, text over its limit, unknown/duplicate question id; carries `question_id` and `rule` |
| `answer_too_large` | the answer payload exceeds 64 KiB |
| `rate_limited` | more than 10 answer submissions a minute in one conversation |

The refusal names the question, and the client keeps the field values it already had
(FR-012). The atomic claim means two tabs, or two channels, can never both succeed.

## 3. Server → client: a sensitive-input ask opens

```json
{ "sensitive_ask": { "ask_id": "9a2c…", "label": "GitHub token",
    "purpose": "A GitHub token with repo scope, to file the issue you asked for.",
    "protected": true, "min_bytes": 8, "max_bytes": 65536,
    "expires_at": "2026-11-01T10:33:00Z" } }
```

- Sent to **every** open socket on the conversation, so a second tab or another device shows
  the same dialog; also replayed by `TopicConnection::open` for every ask still `open`, so a
  reload does not lose it (FR-048).
- `protected: false` when the redactor's minimum is above what a person may submit: the dialog
  states plainly that a value this short cannot be scrubbed (FR-043, SC-019).

```json
{ "sensitive_ask_closed": { "ask_id": "9a2c…", "outcome": "provided" } }
```

`outcome` is `provided`, `declined`, `timeout` or `abandoned`. Every socket closes its dialog on
this frame, whichever tab or person resolved it.

## 4. Client → server: the value

```json
{ "sensitive_input_answer": { "ask_id": "9a2c…", "value": "ghp_…" } }
```

```json
{ "sensitive_input_answer": { "ask_id": "9a2c…", "decline": true } }
```

Exactly one of `value` or `decline`. Rules:

- The value is **not** trimmed, normalised or re-encoded (FR-037). An empty or whitespace-only
  value is refused as unanswered and the dialog stays open (FR-038).
- Refused with `client_error` and no delivery when the ask is not open, is not in this
  conversation, or the value is over `max_bytes`:
  `sensitive_ask_not_open`, `sensitive_value_empty`, `sensitive_value_too_large`.
- The frame is never echoed, never logged, and never reaches the agent's model: the tool
  returns it to the running script only (FR-041).

## 5. History

`GET …/history` returns question entries as `content.QuestionSet` (with the filled `state`,
`answers`, `answerer`) and the answer as an ordinary `Request` whose content is
`{ "question_answer": { … } }`, so the conversation shows the person's message with its
summary text. A sensitive ask appears as a `Request`-adjacent entry describing that a value was
requested and what came of it — never the value (FR-054, FR-061):

```json
{ "content": { "SensitiveAsk": { "ask_id": "9a2c…", "purpose": "…",
    "label": "GitHub token", "state": "provided" } } }
```

## 6. Frame index

| Direction | Frame | Section |
|-----------|-------|---------|
| S → C | `history_entry` with `content.QuestionSet` | §1 |
| S → C | `question_set` | §1 |
| C → S | request with `content.question_answer` | §2 |
| S → C | `client_error` (`question_not_open`, `invalid_answer`, `answer_too_large`, `rate_limited`) | §2 |
| S → C | `sensitive_ask` (live and on socket open) | §3 |
| S → C | `sensitive_ask_closed` | §3 |
| C → S | `sensitive_input_answer` | §4 |
| S → C | `client_error` (`sensitive_ask_not_open`, `sensitive_value_empty`, `sensitive_value_too_large`) | §4 |

## 7. The answer as the agent sees it

`VizierRequestContent::QuestionAnswer` renders as its `text`, then the JSON of `answers` in a
fenced block — the same shape `ClientEvent` uses, so the agent gets structure without the
person's summary being lost:

```text
Database: SQLite

```json
{ "set_id": "6f1c0d6e-…", "answerer": "alice", "declined": false,
  "answers": [ { "question_id": "db", "state": "answered", "value": "sqlite",
                 "label": "SQLite" } ] }
```
```

Every supported channel submits the same request shape, so the agent needs no channel-specific
handling (FR-030).
