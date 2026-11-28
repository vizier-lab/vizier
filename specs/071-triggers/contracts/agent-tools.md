# Contract: Agent tools (category `triggers`)

`list_trigger_kinds`, `create_trigger`, `update_trigger`, `delete_trigger`, `list_triggers`,
`test_trigger`, `list_trigger_runs`, `get_trigger_run_detail` (a run's detail includes its events and
verdicts).

- Every result has a `status`; a refusal is data.
- The agent id is supplied by the system.
- Reading tools need the person behind the turn to hold `view_triggers`, changing ones
  `manage_triggers` (FR-041). A missing permission is a `status`. In a Telegram direct
  message the person is the user linked to the sender's account (spec 069); not offered in groups,
  or when nobody has linked it.
- Every created trigger is inactive and `awaiting_approval`. The agent never sees a webhook's
  secret: approving sets a fresh one and returns it to the person approving.
- An agent has at most 20 triggers; more is a `status`.
- `create`/`update` cannot activate a trigger; changing one returns it to awaiting approval.
- An agent may write triage criteria but not choose its model; a destination only to the
  conversation it is in.
- An agent may lower a trigger's run limit, never raise it or switch it off.
- A new trigger's scope is the narrow default. An agent cannot grant a category it cannot call in
  this turn, and every widening notifies the owner.
- Not offered in trigger runs, dreams, workers or consultations, except the two run-reading tools
  in dreams (`DREAM_TOOL_NAMES`).
- Not offered in Discord yet (the turn does not carry the author id a DM lookup needs).

## In trigger runs only

`nothing_to_report {note}`: turn-ending; the run delivers nothing and the note goes into the next
summary. The only trigger tool offered in a trigger run.
- An agent never sets a monitor's `tools`: a new or rewritten script gets `memory_read`, an unchanged
  one keeps what was saved, and nothing exceeds the calling turn's own scope (FR-031).
- `test_trigger` refuses a trigger awaiting approval (`awaiting_approval`), and one whose script may
  call tools the calling turn may not (`forbidden`).
