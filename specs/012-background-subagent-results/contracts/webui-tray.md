# Contract: WebUI Background Jobs

**Feature**: `012-background-subagent-results`

This contract covers the WebUI behaviour for User Story 5. The layout is the one agreed in conversation on 2026-10-09.

## Store: `app/hooks/backgroundJobStore.tsx`

There is one store for the open topic: `jobs: Map<id, BackgroundJobSnapshot>` plus `finishing: Map<id, 'done' | 'cancelled' | 'lost'>`.

| Input | Effect |
|---|---|
| topic opened / WebSocket (re)connected | `GET …/jobs` replaces `jobs`. Any job that was in the tray but is no longer listed is fetched by id: if it is `reported` or `cancelled`, it is dropped silently; if it is `interrupted` or `undelivered`, it is marked `lost` |
| `{background_job}` frame, `state: running` | upsert into `jobs` |
| `{background_job}` frame, `state: reporting` | upsert, mark `done`, remove after 3s |
| `{background_job}` frame, `state: cancelled` | upsert, mark `cancelled`, remove after 3s |
| `{background_job}` frame, `state` is `undelivered` or `interrupted` | mark `lost`, remove after 3s |

`connectionStore` routes a frame with a top-level `background_job` key to this store. It never goes to `lastMessage`, so the chat's response handler cannot mistake it for a response.

## Tray: `app/components/BackgroundJobTray.tsx`

- **Position**: rendered by `chat.tsx` directly above the message input, inside the same sticky container, so it moves with the input. It renders nothing when there are no jobs.
- **Collapsed row**: `⟳ Batch <id> ▓▓░ <done>/<total> done · <elapsed>  ✕`, or for a delegation `⟳ Delegated to <agent> · <elapsed>  ✕`.
- **Cancel (FR-028)**: `✕` opens an inline confirmation (`Cancel this job? [Keep] [Cancel job]`, not a browser `confirm()` dialog), then calls `POST …/jobs/{id}/cancel`. While the request is in flight the row shows `cancelling…`. A `409` shows the job's final state instead. The control is hidden once the job is `reporting`.
- **Expanded rows**: one per piece showing `state icon · prompt (1 line, ellipsized) · state · elapsed · ›`. `›` opens the piece panel.
- **Jump**: `↥ jump` scrolls to the trail row of the `tool_choice` that launched the job. That row is matched by the job id in its tool response.
- **Height**: the tray's maximum height is 40vh, and it scrolls internally beyond that.
- **Phone width** (`< 640px`): the tray is a single pill, `⟳ N running`, which opens the expanded list as a bottom sheet.
- **Finished jobs**: a finished job shows `✓ … 2 answered · 1 timed out`, `⊘ cancelled` or `⚠ lost` for 3 seconds, then is removed.
- **Live elapsed time**: one shared 1-second timer drives every elapsed label while `jobs` is non-empty.

## Piece panel

The piece panel reuses `SlideOver` and `ActivityTrail`. It loads `…/pieces/{ordinal}/history` and runs it through `groupHistory`. While the piece is `running`, it re-fetches whenever a `background_job` frame for that job arrives, because the piece's own frames are not streamed. The panel is read-only.

## Report entry in the conversation

A history `Request` with `background_report` content is rendered by a new `BackgroundReportItem`. It is shown as a divider-styled, collapsible row, not as a user bubble:

```
┄┄ ⚙ Background batch b-7f3a finished · 2 ✓ · 1 ⧖ timed out ▸ ┄┄
```

Expanding the row lists each entry's prompt, state and text, with `›` to open the piece panel. Live, the entry is appended when the `reporting` frame arrives. If a turn is in progress (`isThinking`), it goes into the existing queued-messages list, so it lands in the same order the server processes it.

## Cancel entry in the conversation

The history `Command` entry `cancelled background job <id>` renders through the existing command row as `⊘ You cancelled background job <id>`. When the agent cancels a job, no entry is added: the agent's own `cancel_background_job` tool call already appears in its activity trail.

## Topic list

A topic whose `running_jobs > 0` shows a `⟳N` badge. The badge refreshes with the topic list. It is not pushed.

## Typecheck

`webui/app/interfaces/types.ts` gains the following, and `cd webui && npm run typecheck` must pass:

- `BackgroundJobSnapshot`, `BackgroundPieceSnapshot` and `BackgroundReport`;
- `{ background_report: BackgroundReport }` in `VizierRequestContent`;
- `running_jobs` on the topic entry.
