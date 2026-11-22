# Contract: HTTP, frames and the WebUI

## Consulting side (the person in A's chat)

**No new routes.** The spec 059 routes already serve consultations, because a consultation is a
`subagent` row of the same asking conversation:

| Method | Path | Returns |
|---|---|---|
| GET | `/api/v1/agents/{id}/channel/{channel}/topic/{topic}/subagents` | the topic's workers **and** consultations: open ones first, then the last 50 final ones |
| GET | `…/subagents/{sid}` | one row, uncut |
| GET | `…/subagents/{sid}/history` | its own conversation; for a consultation, the consultee's consultation conversation |

A topic that is not the caller's, or an unknown id, answers `404`. The snapshot carries
`consultee` (the consulted agent's id; absent for a worker) and `follow_ups` (absent when 0).

Frame: `subagent` (the existing `SessionFrame::Subagent`). The WebUI splits rows and frames by
`consultee`: the Subagents item counts workers, the Consultations item counts consultations.

## Consulted side (B's people)

- B's conversation list includes `kind: "consultation"` items, one per consulting agent
  (`channel: consultation__{A}`, title "Asked by <A name>", place "N consultations", the newest
  consultation as `latest_topic`), shown only to `can_edit_agent(B)` callers; `counts.consultation`.
- The read routes (history, files) take `channel_id = consultation__{A}` and a topic, through
  `readable_channel`; `authorize_topic` refuses anyone who may not edit B.
- **Read-only by construction**: the topic socket builds only the caller's own web channel
  (`VizierChannelId::from_web`), so nothing a person sends can reach a `Consultation` session.
- The WebUI opens it in the read-only chat page (`/{B}/chat/read?channel=…&topic=…`).

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
