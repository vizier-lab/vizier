# Quickstart: Triggers (dummyplug)

Run `just run`, create an agent on the **dummyplug** provider.

A trigger run's prompt is not a dummyplug command, so steps 6, 8, 9, 14, 18, 20 and 23 cannot be
steered by dummyplug alone: put a stand-in in front of it that turns a marker in the instruction
into the command (a delay, a tool call, a refusal). Triage (17, 18) needs a stand-in model that
answers with verdicts. `create_trigger` takes `settings` even for a webhook (`{}`).

| # | Do | Expect |
|---|---|---|
| 1 | Create a webhook through the API | address and secret returned once |
| 2 | `POST` with the secret, body `{"hello":"world"}` | `202`, `accepted`, one run |
| 3 | Open the run's history | trigger framing, payload marked untrusted, a reply |
| 4 | `POST` with a wrong secret; to an unknown id | the same `404` |
| 5 | Repeat 2 with the same delivery id | `duplicate`, same event id |
| 6 | Send while a run is busy (dummyplug `delay_ms`) | events wait; one next run takes them |
| 7 | Send past the hourly limit | events wait, then one run when allowed |
| 8 | In the run, send `tools` | only the scoped categories |
| 9 | Spawn a worker from a scoped run with `subagents` allowed | worker has the same scope |
| 10 | Create a monitor returning `{"state": 1}` | state saved, no run |
| 11 | Change the watched value so the script returns `fire` | exactly one run |
| 12 | Make the script raise | error shown, no run, longer wait |
| 13 | Ask the agent (chat) to `create_trigger` a webhook | inactive, owner notified |
| 14 | Restart during a run | resumed or `interrupted`; waiting events are handled |
| 15 | (needs a real bot) Telegram direct message from a linked account (`/connect`), create a trigger | that user's permissions apply |
| 16 | (needs a real bot) The same from a group, or an account nobody linked | trigger tools not offered |
| 17 | Triage on (scripted verdicts): send a mix | only important events run; reasons recorded |
| 18 | Make triage answer in the wrong form | event runs, marked not triaged, owner told |
| 19 | Set a destination and storm the webhook | at most the limit delivered in the hour |
| 20 | Run ends with `nothing_to_report` | nothing delivered; note in the next summary |
| 21 | Shorten the summary window and wait | one summary to the setter only |
| 22 | Fill a monitor's queue | new event refused, state unchanged, reported again later |
| 23 | Make a run fail twice | its events end `failed`, setter told |
| 24 | Replay a signed request outside the tolerance | `404` |

## Results (2026-11-28)

Steps 1 to 14 and 17 to 24 passed over HTTP on SQLite, with a driver script whose model stand-in ran dummyplug's
`protocol.mjs` in-process, and a temporary host serving `vizier-core` and `vizier-http` (the `vizier` binary
needs `cmake`, which was not installed). Substitutions: step 7 raised the limit instead of waiting
an hour; step 21 backdated the trigger two hours in the database; step 22 filled the queue in the
database. Steps 15 and 16 need a real bot and were **not run**. The driver script was not kept.
