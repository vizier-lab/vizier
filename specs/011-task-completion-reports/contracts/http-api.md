# Contract: HTTP API

**Feature**: `011-task-completion-reports`

Two new routes on the existing task router (`src/channels/http/api/v1/agents/task.rs`, nested at
`/agents/{agent_id}/tasks` in `agents/mod.rs:97`), plus changed fields on the existing ones.

They belong here rather than under `/channel` because that router derives its session from the
caller — `VizierChannelId::HTTP(user.username, channel_id)` (`channel.rs:170,126`) — so it can
only ever name the caller's own web conversations, never a task's. Under `/tasks/{slug}` the
permission check is the one tasks already use, `user_can_view_agent`, which is FR-016.

---

## `GET /api/v1/agents/{agent_id}/tasks/{slug}/runs`

Paginated, newest first. Carries **no response text** (FR-014, FR-020).

**Query**

| Param | Type | Notes |
|---|---|---|
| `before` | RFC3339 | `ran_at` of the oldest run on the previous page |
| `before_id` | integer | that run's `id`; completes the cursor so runs sharing a millisecond cannot straddle a page boundary. Ignored without `before` |
| `limit` | integer | default 20, capped |

**200**

```json
{
  "status": 200,
  "data": {
    "runs": [
      { "run_id": "2026-10-04T09:00:00Z", "id": 412, "ran_at": "2026-10-04T09:00:00Z",
        "finished_at": "2026-10-04T09:02:14Z", "state": "answered" },
      { "run_id": "2026-10-03T09:00:00Z", "id": 398, "ran_at": "2026-10-03T09:00:00Z",
        "finished_at": "2026-10-03T09:00:31Z", "state": "no_response" }
    ],
    "has_more": true
  }
}
```

`has_more` is FR-009 — the caller must not be offered a page that does not exist.
`run_id` is the run's address (its `ran_at`), which is what the detail routes take.

**403** not permitted to view the agent · **404** agent or task unknown

---

## `GET /api/v1/agents/{agent_id}/tasks/{slug}/runs/{run_id}/history`

That one run's full exchange — the trail a person expands in the task view (FR-010).

Builds `VizierSession(agent_id, VizierChannelId::Task(slug, run_id), None)` and calls
`list_session_history`, which already accepts any session. Returns `Vec<SessionHistory>`, the same
shape `get_topic_history` returns, so the WebUI renders it through `trail.ts`'s existing
`groupHistory` with no new rendering code.

**Query**: `before`, `before_seq`, `limit` — identical to `get_topic_history`'s `HistoryQuery`
(`channel.rs:53-60`).

**One WebUI caveat on reusing `groupHistory` for this.** A run's history opens with a `Request`
entry, and `groupHistory` adopts any `Request` as the turn's request (`lib/trail.ts:126`) — despite
the comment four lines above stating that an agent-initiated turn has none. That comment is true
only because no task history has ever reached the function. Rendering a run must open the turn
*without* adopting the entry, or every run's trail gains a nameless empty bubble and the
instruction appears twice on screen.

**404** when no run exists at that address.

---

## `GET /api/v1/agents/{agent_id}/tasks/{slug}` (changed)

`TaskResponse` gains the last run and the requester, and loses nothing:

```json
{
  "slug": "daily-report",
  "title": "Daily report",
  "instruction": "Summarise merged PRs and post to #eng",
  "is_active": true,
  "schedule": { "CronTask": "0 9 * * *" },
  "timestamp": "2026-09-01T12:00:00Z",

  "requester": { "user": "@dani (DiscordId: 182…)" },

  "last_run": {
    "run_id": "2026-10-04T09:00:00Z",
    "ran_at": "2026-10-04T09:00:00Z",
    "finished_at": "2026-10-04T09:02:14Z",
    "state": "answered",
    "response": "Posted to #eng. 4 PRs merged since yesterday.\n\nOne needed a note: **#412** …"
  }
}
```

- `requester` is `{"user": "<identity>"}` or `{"agent": "<agent_id>"}` (FR-032). A person who can
  no longer be resolved still renders as recorded (FR-033) — the string is what was stored, and
  nothing looks it up.
- `last_run` is `null` for a task that has never run — distinct from a run whose `state` is
  `no_response`, which has a `last_run` with `"response": null` (FR-005).
- `response` is the full text. Not truncated: the task view is where the response is read, and
  FR-015 renders it rather than hiding it.
- `last_executed_at` is retained for compatibility, now redundant with `last_run.ran_at`.

---

## `GET /api/v1/agents/{agent_id}/tasks` (changed)

Each entry gains `requester` and a `last_run` **without** `response` — the list shows when it ran
and the state it reached, nothing more (FR-014).

---

## `POST` / `PUT /api/v1/agents/{agent_id}/tasks` — **breaking**

`user` is removed from the request body. The requester is taken from the authenticated caller
(FR-028), who the handler already holds as `AuthenticatedUser` and currently ignores.

A body still carrying `user` is accepted and the field ignored, rather than rejected: it is
currently unvalidated and most often holds the literal string `"user"`, so refusing it would
break callers to no benefit. The change is breaking in behaviour, not in status code, and the
commit carries `[**breaking**]` per the changelog convention.

`DELETE` additionally removes the task's runs (FR-012).
