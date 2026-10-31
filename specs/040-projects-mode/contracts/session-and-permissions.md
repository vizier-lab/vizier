# Contract: Project Session Identity and Permissions

**Feature**: `040-projects-mode`

This contract fixes the session identity, the listing rules and the
authorization matrix. It is the single reference for the core, HTTP and WebUI
layers.

---

## 1. Channel namespace

A project session is an ordinary HTTP session whose channel id is namespaced:

```text
channel_id = "project__" + project_id
VizierSession = (agent_id, HTTP(owner_username, channel_id), Some(topic))
channel slug  = "http__" + owner_username + "__" + channel_id
              = "http__<owner>__project__<project_id>"
```

`projects::channel` exposes the only implementation:

| Helper | Contract |
|--------|----------|
| `PREFIX: &str = "project__"` | Shared literal (the WebUI mirrors it). |
| `channel_id(project_id) -> String` | `"project__" + id`. |
| `project_id(channel_id) -> Option<&str>` | `Some` only for the exact prefix and a non-empty id. |
| `is_project_channel(channel_id) -> bool` | Prefix test. |
| `project_of_slug(slug) -> Option<&str>` | Parses `http__<owner>__project__<id>` for listing filters. |

**Rules**

- The project id is 10 alphanumeric characters; no `:` or path separators
  (the slug becomes a directory name under `uploads/`).
- The owner username is always supplied by the authenticated principal. No
  route or request body may set it.
- Two people use two different channels for the same project and therefore see
  only their own sessions (FR-024).
- The project id must resolve to a row of the same agent and be non-removed for
  a turn or history read to proceed.

---

## 2. Session listing separation

`GET /agents/{id}/conversations` (and its search/counts/cursor) MUST NOT
include project sessions (FR-006).

Implementation:

- `service::conversations::kind_of` returns `None` for a channel slug whose
  channel id starts with `project__`.
- `storage::sqlite::session::conversation_rows` skips the same slugs before
  building rows.

The sidebar's Projects section uses only
`GET /agents/{id}/projects` and `GET /agents/{id}/projects/{pid}/sessions`.
The conversation picker and session search therefore stay clean with no schema
change.

`session_detail`, `session_history`, `session_turn`, session files and scratch
files are keyed as for any HTTP session; nothing is renamed or migrated.

---

## 3. Authorization matrix

`P` = `all_agents:projects` OR (`owned_agents:projects` AND the person owns the
agent). `V` = `can_view_agent`. `O` = the project's `owner_id` equals the
caller's `user_id`.

| Action | Required |
|--------|----------|
| Projects section / `GET /projects` | `V` and `P` |
| `GET /projects/{id}` | `V`, `P`, project belongs to agent, not removed |
| `POST /projects` (create) | `V`, `P`, `AuthMethod::Desktop` |
| `PATCH /projects/{id}` | `V`, `P`, and (`O` or `all_agents:projects`) |
| `DELETE /projects/{id}` | `V`, `P`, and (`O` or `all_agents:projects`), no pending turn |
| `GET /projects/{id}/sessions` | `V`, `P`, project not removed |
| `POST /projects/{id}/sessions` | `V`, `P`, project not removed |
| Open the topic WebSocket | `V`, `P`, project belongs to agent, not removed |
| Read history/files/detail of a project topic | `V`, `P`, not removed |
| Delete a project topic | `V`, `P`, not removed, and the session is the caller's (channel username) |
| Send a turn on an open project socket | re-check `P` on every inbound request |
| See another person's session | never (different channel username) |

The check is applied at every entry point that can start or read a turn with a
caller-supplied channel id: the WebSocket open and inbound path,
`POST /agents/{id}/chat`, `POST /agents/{id}/btw`, the read-only history/files/
detail/delete routes, and the topic-jobs routes. A REST chat naming a project
channel without `P` is refused exactly like the socket path; there is no route
that bypasses the matrix.

Notes:

- `V` is the existing `authorize_view`; `P` and `O` are new checks in
  `projects/access.rs`.
- The desktop principal's `user_id` is stable, so `O` is a simple comparison.
- The project owner is the person who created it. Ownership transfer is out of
  scope.
- A person with `all_agents:projects` can manage any project they can view; a
  browser user with only `owned_agents:projects` can manage their own projects
  but cannot create one.

---

## 4. WebSocket and revocation

`TopicConnection::open` currently checks only `authorize_view`. For a project
channel it additionally:

1. resolves the project by `(agent_id, project_id)`;
2. rejects `404` when absent or removed;
3. checks the matrix above for the opening principal.

`TopicConnection::handle_inbound` re-checks the project permission before
`transport.send_request` for a chat/turn frame. Consequences:

- Revoking `all_agents:projects` or `owned_agents:projects` refuses the next
  turn immediately (FR-018 edge case).
- A turn already running is allowed to finish.
- Reading history from an already-open socket is not a new turn; the next send
  is refused.
- A browser without the permission cannot reach a project by a bookmarked URL:
  `open` fails and the route renders the refused state, never read-only
  content.

The existing agent visibility rule still applies first, so a user who cannot
see the agent gets the same refusal as for any other conversation.

---

## 5. Notifications and usage

- **Notification audience**: unchanged. `chat_replied` already resolves the
  owner from `HTTP(username, _)` and uses `event_key = chat:<history_uid>`.
- **Notification href**: for a project channel, build
  `/{agent_id}/projects/{project_id}/chat/{topic}`. The ordinary route remains
  `/{agent_id}/chat/{topic}`.
- **Usage**: unchanged schema. `channel` is the channel slug
  (`http__<owner>__project__<id>`) and `place` is
  `http:<channel_slug>:<topic>`, so both the person and the project are
  recoverable. No aggregation change in this foundation.
- **Background report**: woken in the origin project session, so it inherits
  the project context and the shell; this is the one background path allowed
  to (spec Assumptions).

---

## 6. Turn context resolution

`projects::context::for_session(deps, &session)` returns
`Option<ProjectTurnContext>`:

| Field | Source |
|-------|--------|
| `project_id`, `name`, `folder` | project row |
| `available` | `folder.is_dir()` |
| `shell_enabled` | project row |
| `instructions`, `instructions_truncated` | `projects::instructions` |

It returns `None` for every non-project channel (Discord, Telegram, task,
dream, subagent, inter-agent, system) and for an ordinary WebUI channel. It is
called in `chat_turn`, side chat and checkpoint prompt rebuilds; the result is
also stored on `ToolContext.project` for the tool.

A project row that has been removed still resolves for history reads, but not
for new turns; the `removed` check is part of the turn authorization, not the
context builder. The resolver re-reads the session owner's current project
permission on every turn: if it was revoked, the shell and the project
instructions are omitted (history remains readable), so even an internally
woken background report cannot use the shell after revocation.
