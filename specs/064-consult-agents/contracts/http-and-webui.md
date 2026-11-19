# Contract: HTTP, frames and the WebUI

## Consulting side (the person in A's chat)

These routes mirror spec 059's subagent routes. A topic that is not the caller's, or an unknown
id, answers `404`.

| Method | Path | Returns |
|---|---|---|
| GET | `/api/v1/agents/{id}/channel/{channel}/topic/{topic}/consultations` | snapshots: open ones first, then the last 50 final ones |
| GET | `…/consultations/{cid}` | snapshot + `task` (the first request in full) + `result` in full |
| GET | `…/consultations/{cid}/history` | the consultee's consultation conversation, as history entries |

The existing `…/subagents` routes filter to `consultee == null`.

Frame: `subagent` (the existing `SessionFrame::Subagent`) on the consulting topic socket, now
carrying `consultee`. The WebUI routes frames with `consultee != null` to the Consultations store.

## Consulted side (B's people)

- B's conversation list includes `kind: "consultation"` rows, each with `from_agent: {id, name}`.
  Only `can_edit_agent(B)` callers see them, and other callers get a list without them.
- History reads go through the existing topic routes with `channel_id = consultation__{A}`.
- The topic socket refuses `chat`, `client_event` and `ui_page_state` frames on that channel with
  `{"type": "error", "code": "read_only_conversation"}`.

## Removed

Every `/api/v1/workspaces…` route, the `…/topics/{tid}/stream` Workspace socket and the
Workspace OpenAPI schemas are gone. Their conformance vectors and their `pending.txt` lines go
too.

## WebUI mockup (to be redrawn in FigCraft before code: `Chat` page, status strip + panel)

Strip, with a third item beside Background work and Subagents:

```
┌─ composer deck ─────────────────────────────────────────────────────────────┐
│  ⟳ Background work 1   ◎ Subagents 2   ⇄ Consultations 2 •                  │  ← amber dot: one asking
└─────────────────────────────────────────────────────────────────────────────┘
```

Panel (opens like the Subagents panel; read-only; ×, Escape and an outside press close it):

```
┌ Consultations ─────────────────────────────────────────────── × ┐
│ 10:42 · 2 consulted                                              │
│ ┌──────────────────────────────────────────────────────────────┐ │
│ │ (R) researcher   1 · "Find last week's incident reports…"    │ │
│ │     c_8k2m1x0q · 1m 12s · waiting for you ● amber            │ │
│ │     ❝ Which region — eu-west or us-east? ❞                    │ │
│ └──────────────────────────────────────────────────────────────┘ │
│ ┌──────────────────────────────────────────────────────────────┐ │
│ │ (B) billing      2 · "What did we invoice Acme in October?"  │ │
│ │     c_4n9p2a7r · 38s · answered · the agent read it          │ │
│ │  ▾  [trail of billing's steps, the chat's own Trail]         │ │
│ │     Answer: "€12,400 across 3 invoices…"                     │ │
│ └──────────────────────────────────────────────────────────────┘ │
└──────────────────────────────────────────────────────────────────┘
```

- Each row is the design `SubagentWorker`, with a new `agent` slot that draws the consultee's
  `Sigil` and name in place of the worker label. Follow-ups show as "· 2 follow-ups".
- Grouped by call, as the Subagents panel is (`lib/subagents.ts`'s grouping, made generic).
- The phone layout uses the same bottom sheet as the Subagents panel.

B's conversation list:

```
│ Conversations                         │
│  ⇄ Consultation from support · 10:42  │  ← read-only; opening shows no composer
│  💬 Pricing questions · 09:10          │
```

Rail after the removal: Home, then the agents. There is no Workspace group, and the `+` goes
straight to New agent (no `CreateChooser`).
