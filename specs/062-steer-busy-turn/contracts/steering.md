# Contract: Steering a Running Turn

The engine rules, independent of channel.

## S1 — Eligibility

A request received while its session is busy (a turn running, or a queue not yet drained) is queued with a mode from `steer::mode` (data-model.md). `Steer` items (person messages) and `Absorb` items (silent reads, R12) enter a running turn; `Queue` items never do. Non-person sessions, `/queue`, skill commands, commands, voice messages, client events, question answers and transient requests are `Queue`.

## S2 — The boundary

The only place a waiting message enters a turn is the round boundary of a chat turn's loop: after every tool call of the round has a result, before the next completion. Order at a boundary:

0. in a Workspace session: copy the topic's unread entries (catch-up to head);
1. take every `Steer` and `Absorb` item of the session (in arrival order);
2. `steer_turns` in one transaction (R2); drop what it did not save;
3. for each saved message, in order: attachments to session files, project mentions, model text from `to_prompt` (`via: mid_turn`);
4. claim background deliveries (unchanged);
5. send **one** user message: delivery text first (if any), then each person message and silent read, in arrival order, as its own text part (a boundary with only silent reads still sends them);
6. push each saved entry as `SessionFrame::HistoryEntry`; send `Steered` on each message's own sender and drop it.

No boundary exists after a round that ended the turn (a successful turn-ending call), or during a handover/checkpoint job, a side chat, or the final completion. Items still waiting then are popped by the loop, in order, as own turns.

## S3 — Exactly once

- In memory: taking and popping both hold the session's queue lock.
- On disk: the `session_turn` delete and the history insert are one transaction; the delete is guarded on `state = 'queued'`.
- A message is never both steered and run: a steered item is no longer in the queue and has no row to re-send.

## S4 — Lifecycle

| Event | Waiting `Steer` items | Already steered |
|---|---|---|
| `/abort` | queue cleared, rows cancelled (as today) | stay in the aborted turn's history |
| Update / restart | re-sent in order behind the resumed turn; steered at its next boundary | replayed as part of the resumed turn |
| Crash | re-sent by `recover_agent` in row order, same as above | same |
| Agent delete | rejected as today | deleted with the history |
| Turn ends without a boundary | popped as own turns, in arrival order | n/a |

## S5 — What the model sees

```text
---
sender: dana
via: mid_turn
sent_at: 2026-11-15T10:42:07+07:00
metadata: {}
---

wait, keep the old token format
```

BOOT.md (fixed, every turn): *"A message with `via: mid_turn` was sent while you were working on this turn; take it into account before your next step, and answer it in your reply."*

Replay builds the same single user message from the stored entries (adjacent user-side entries after a round are merged).

## S5a — Silent reads

An absorbed silent read keeps its silent-read rendering plus `via: mid_turn` and `addressed: false` in its frontmatter, gets no initiative roll, no session-detail refresh and no notification, and never splits the WebUI trail. Once absorbed it is never processed again.

## S6 — The steered message's sender

Receives exactly one frame, `steered`, then closes. Renderers: Discord/Telegram stop their typing indicator and post nothing; the WebUI ignores it. The running turn's reply is posted once, by its own renderer.
