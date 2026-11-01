# Data Model — Asking the Person (042)

**Feature**: `042-agent-asking-question` | **Date**: 2026-11-01

Everything here is additive. Two new tables, two new `SessionHistoryContent` variants
(`QuestionSet`, `SensitiveAsk`) and one new `VizierRequestContent` variant. No existing column or field
changes, so no migration code and no #82 entry (research R13).

## 1. Question set

The unit the agent asks. Immutable once presented; only its state and answer change.

| Field | Type | Rules |
|---|---|---|
| `set_id` | string (uuid) | server-assigned; it is also the `SessionHistoryContent::QuestionSet` entry's `uid` |
| `short_id` | string | 8 chars, unique; the only thing a platform control carries (`custom_id`, `callback_data`) |
| `title` | optional string | ≤ 200 bytes |
| `description` | optional string | ≤ 1 000 bytes |
| `questions` | array of Question | 1–10, ids unique (FR-004, FR-005) |
| `submit_label` / `dismiss_label` | optional string | ≤ 100 bytes; defaults come from the surface |
| `agent_id`, `channel`, `topic` | session triple | the conversation the set belongs to |
| `state` | enum | `pending` → `answered` \| `declined` \| `withdrawn` |
| `answers` | optional JSON | the recorded per-question answers; null while pending |
| `answerer` | optional string | who submitted, as the surface identifies them |
| `controls` | optional JSON | platform message references, so a reader can edit or disable them |
| `created_at`, `closed_at` | timestamps | `closed_at` set on any terminal state |

### State transitions

```
            ask_question (valid)                 submit (valid, claims the row)
   (none) ─────────────────────▶ pending ──────────────────────────────────▶ answered
                                   │  │
                    dismiss ───────┘  └─────── script failed after the call ─▶ withdrawn
                       │
                       └────────────────────────────────────────────────────▶ declined
```

- `pending → answered`: atomic `UPDATE … WHERE set_id = ? AND state = 'pending'`; zero rows
  means the set is already closed (FR-017). This is the only place a race is resolved.
- `pending → declined`: the person dismissed the set (FR-014).
- `pending → withdrawn`: the script that asked then failed (FR-010) — the set must not stay
  answerable. Cards show it as withdrawn; channel controls are removed.
- A set never expires (FR-019) and is never invalidated by a later set.

## 2. Question

| Field | Type | Rules |
|---|---|---|
| `id` | string | 1–64 bytes, `^[a-z0-9][a-z0-9_-]*$`, unique in the set |
| `prompt` | string | 1–500 bytes, shown as the label |
| `help` | optional string | ≤ 500 bytes, shown under the prompt |
| `optional` | bool | default `false`; a required question with a default counts as answered |
| `default` | optional answer value | must be valid for the type (inside the options, within bounds) |
| `type` | enum | `text` \| `choice` \| `multi_choice` \| `confirm` |

Type-specific content:

- `text`: `placeholder` (≤ 100 bytes), `max_length` (1–4 000, default 4 000), `multiline` (bool).
- `choice`: `options` (1–25).
- `multi_choice`: `options` (1–25), `min_selections` (default 0), `max_selections` (default 25);
  `min ≤ max ≤ options.len()`.
- `confirm`: `yes_label` / `no_label` (≤ 100 bytes each).

## 3. Option

| Field | Type | Rules |
|---|---|---|
| `value` | string | 1–100 bytes, unique in the question; what the agent receives |
| `label` | string | 1–100 bytes; what the person reads (may be emoji/RTL; rendered as text) |
| `description` | optional string | ≤ 200 bytes |

## 4. Answer

What the person submitted, per question.

| Field | Type | Rules |
|---|---|---|
| `question_id` | string | must name a question in this set |
| `state` | enum | `answered` \| `skipped` |
| `value` | per type | `text`: string ≤ `max_length`; `choice`: one option value; `multi_choice`: an array of distinct option values within bounds; `confirm`: `true`/`false` |

Validation (one function, `questions::submit`, used by every surface — FR-012, FR-026):

1. every required question `answered` (a `default` counts);
2. every choice/multi value is one of the offered values; no duplicates in a multi;
3. multi-selection count within `min`/`max`;
4. text within its length; a whitespace-only text is `skipped` (and fails a required question);
5. no unknown or duplicate `question_id`; every question present exactly once;
6. whole answer ≤ 64 KiB.

A refusal names the question and the rule and changes nothing (the card stays open, the
submission is not sent).

## 5. Answer turn

`VizierRequestContent::QuestionAnswer(QuestionAnswer)`.

| Field | Type | Notes |
|---|---|---|
| `set_id` | string | the set being answered |
| `text` | string | the human-readable summary shown as the person's message and to the agent |
| `answers` | array of Answer | structured, for the agent (FR-013, FR-062) |
| `answerer` | string | as the surface identifies them |
| `declined` | bool | `true` for a dismissal, in which case `answers` is empty |

`Display` renders `text`, then the JSON of `answers` in a fenced block — the same shape
`ClientEvent` already uses, so the agent sees structured data and can still read the summary.
`kind()` is `question_answer`.

The request carries the session stored with the set, so an answer to an old set in a channel
whose active topic moved on still lands in the conversation that asked (FR-019).

## 6. Question card (history)

`SessionHistoryContent::QuestionSet { set_id, title, description, questions, submit_label,
dismiss_label, state, answers, answerer }` — the first six fields are written once and never
rewritten; the last three are filled from `question_set` at read time
(`fill_question_sets`, research R3), mirroring `ClientToolCall`'s `outcome`/`state`.

Rendering contract: pending → interactive card; answered → read-only record of every answer and
every skipped question; declined → "declined"; withdrawn → "withdrawn" with no controls
(FR-020).

## 7. Sensitive ask

A transcript entry too: `SessionHistoryContent::SensitiveAsk { ask_id, purpose, label, state }`,
written when the ask opens, with `state` filled at read time from `sensitive_ask` (the same fill
pass). It says that a value was requested and what came of it, never the value (FR-054, FR-061).

One request for a sensitive value. **Holds no value.**

| Field | Type | Rules |
|---|---|---|
| `ask_id` | string (uuid) | server-assigned |
| `purpose` | string | 1–1 000 bytes; the agent's reason, shown to the person, not secret |
| `label` | optional string | ≤ 100 bytes; the field's name, e.g. "GitHub token" |
| `agent_id`, `channel`, `topic` | session triple | the conversation and turn it belongs to |
| `state` | enum | `open` → `provided` \| `declined` \| `timed_out` \| `abandoned` |
| `requested_at`, `closed_at` | timestamps | |

```
(none) ── ask_sensitive_input ─▶ open ── submit ────────────▶ provided
                                  │ ├── reject / dismiss ───▶ declined
                                  │ ├── wait limit ─────────▶ timed_out
                                  │ └── abort / panic / restart ▶ abandoned
```

`provided` records **that** a value was supplied, never what it was (FR-044, FR-054). The
transcript entry is the ask row's state; the dialog is the only place the value exists outside
the running script.

## 8. Outcome handed to the script

`ask_sensitive_input` returns (`FR-033`, `FR-050`, `FR-051`):

```json
{ "status": "provided", "value": "…", "protected": true }
{ "status": "provided", "value": "…", "protected": false }   // shorter than MIN_VALUE_BYTES
{ "status": "declined" }
{ "status": "timeout" }
```

`protected` is `false` exactly when the value is shorter than `MIN_VALUE_BYTES`, i.e. when the
turn's redactor refused to arm it (research R10). A call in a conversation where the tool is not
offered is an `Err`, which the script sees as a catchable `RuntimeError` and the turn continues
(FR-047).

## 9. SQL

```sql
CREATE TABLE IF NOT EXISTS question_set (
    set_id     TEXT PRIMARY KEY,
    short_id   TEXT NOT NULL UNIQUE,
    agent_id   TEXT NOT NULL,
    channel    TEXT NOT NULL,
    topic      TEXT,
    state      TEXT NOT NULL,          -- pending | answered | declined | withdrawn
    questions  TEXT NOT NULL,          -- the immutable set, JSON
    answers    TEXT,                   -- JSON, filled once
    answerer   TEXT,
    controls   TEXT,                   -- platform message references, JSON
    created_at INTEGER NOT NULL,
    closed_at  INTEGER
);
CREATE INDEX IF NOT EXISTS idx_qs_session ON question_set(agent_id, channel, topic);
CREATE INDEX IF NOT EXISTS idx_qs_short ON question_set(short_id);

CREATE TABLE IF NOT EXISTS sensitive_ask (
    ask_id       TEXT PRIMARY KEY,
    agent_id     TEXT NOT NULL,
    channel      TEXT NOT NULL,
    topic        TEXT,
    purpose      TEXT NOT NULL,
    label        TEXT,
    state        TEXT NOT NULL,        -- open | provided | declined | timed_out | abandoned
    requested_at INTEGER NOT NULL,
    closed_at    INTEGER
);
CREATE INDEX IF NOT EXISTS idx_sa_session ON sensitive_ask(agent_id, channel, topic, state);
```

Both tables are deleted with their conversation in `SqliteStorage::delete_session`
(`storage/sqlite/session.rs:336-400`, the explicit-delete block) and with their agent in
`AgentStorage::delete_agent` — the conversation's rows already go there, and these are part of
that conversation (FR/US9 #9).

## 10. Storage trait surface

One new module, `storage/question.rs`, mirroring `storage/client_call.rs`:

| Method | Used by |
|---|---|
| `create_question_set(&QuestionSetRecord) -> Result<()>` | `ask_question` |
| `claim_question_set(set_id, &session, answerer, answers) -> Result<Claim>` | `questions::submit` |
| `decline_question_set(set_id, &session, answerer) -> Result<bool>` | dismissal |
| `withdraw_question_set(set_id) -> Result<bool>` | script failure (FR-010) |
| `close_question_set(set_id) -> Result<bool>` | conversation deleted mid-flight |
| `get_question_set(set_id) -> Result<Option<QuestionSetRecord>>` | control resolution, claim |
| `question_set_by_short_id(short) -> Result<Option<…>>` | channel presses |
| `open_asks(&session) -> Result<Vec<SensitiveAskRecord>>` | socket open, restart sweep |
| `open_sensitive_ask(&SensitiveAskRecord) -> Result<()>` | `ask_sensitive_input` |
| `close_sensitive_ask(ask_id, state) -> Result<bool>` | answer, decline, timeout, abandon |
| `abandon_open_asks() -> Result<usize>` | startup sweep |
| `abandon_session_asks(&session) -> Result<usize>` | abort / panic |
