# Data Model: Workspace Channel

Applies to both backends through the shared `SqlStorage`: `init_workspace_schema` for SQLite, `0004_workspaces.sql` for PostgreSQL. Traits follow `storage/project.rs`: a new `WorkspaceStorage` on `VizierStorageProvider`.

Timestamps are epoch milliseconds, so the text-sort rule in spec 052 does not apply.

## Tables

### `workspace`

| Column | Type | Notes |
|---|---|---|
| `id` | text PK | UUID. Never changes |
| `name` | text | As shown. Rule below |
| `name_key` | text UNIQUE | Lower-cased `name`. Enforces FR-001 |
| `created_by` | text | Username, for display only |
| `stopped_at_seq` | integer | Default 0. Stop-all guard (research R7) |
| `created_at`, `updated_at` | integer | |

**Name rule** (Workspace and topic): 1 to 64 characters, `^[a-z0-9][a-z0-9-]*$`. Agents type these into tool calls.

### `workspace_topic`

| Column | Type | Notes |
|---|---|---|
| `id` | text PK | UUID. Is the session's topic |
| `workspace_id` | text FK | `ON DELETE CASCADE` |
| `name` | text | Unique per Workspace |
| `created_by` | text | A username or `agent:<id>` |
| `created_at` | integer | |

### `workspace_member`

| Column | Type | Notes |
|---|---|---|
| `workspace_id` | text FK | `ON DELETE CASCADE`. PK with `kind`, `handle` |
| `kind` | text | `person` or `agent` |
| `handle` | text | Username or agent id |
| `role` | text | `owner` or `member`. Agents are always `member` |
| `added_by` | text | |
| `added_at` | integer | |

A second PK-like unique index on `(workspace_id, handle)` makes `409 handle_in_use` (research R4) a storage rule.

### `workspace_entry`: what is shown as messages

Holds no tool calls or thoughts: those stay in the author's own record (research R3).

| Column | Type | Notes |
|---|---|---|
| `seq` | integer PK | Autoincrement. The one order |
| `workspace_id`, `topic_id` | text FK | `ON DELETE CASCADE`. Index on `(topic_id, seq)` |
| `kind` | text | `person`, `agent` or `note` (a command someone ran, never copied into a record) |
| `author_kind` | text | `person`, `agent` |
| `author` | text | Username or agent id |
| `author_name` | text | Snapshot, so history survives a deleted agent (US4 #5) |
| `body` | text | Text after redaction |
| `files` | text null | JSON list of `file_id` and name |
| `history_uid` | text null | Its `session_history` row. Null for `person`, and for entries from `mention_agent` or `send_message_to_workspace` |
| `source_session` | text null | Slug of the session that wrote it. Used by `catch_up` to skip an agent's own text |
| `origin` | text null | JSON, set when sent from outside: `{kind: "task"\|"chat"\|"discord"\|"telegram"\|"workspace", label}` (FR-032) |
| `is_final` | integer | 1 for a turn's final `Response`. Stats and reactions live on its history row |
| `activated_by_seq` | integer null | The entry that started this turn. Used by the stop guard |
| `at` | integer | |

Written from `save_session_history` for turns, and directly for people and tools (research R3, R6).

### `workspace_mention`: outbox, and the unavailable note

| Column | Type | Notes |
|---|---|---|
| `entry_seq` | integer FK | `ON DELETE CASCADE`. PK with `target` |
| `target_kind` | text | `agent` or `person` |
| `target` | text | Handle |
| `state` | text | See below |
| `updated_at` | integer | |

**Agent mention states**: `pending` → `woken` | `unavailable` | `dropped`. A `pending` row is re-dispatched at startup. Person mentions stay `resolved`, and only drive the highlight.

### `workspace_read`: Read Position

| Column | Type | Notes |
|---|---|---|
| `topic_id` | text FK | `ON DELETE CASCADE`. PK with `agent_id` |
| `agent_id` | text | |
| `read_seq` | integer | Highest entry copied to this agent's session |

Written in the same transaction as the copied rows, so a restart neither loses nor repeats a read. On joining, or on a topic's creation, it starts at the current head, so there is no backfill.

## Changes to existing data

| Item | Change |
|---|---|
| `VizierChannelId` | New variant `Workspace(String)`, slug `workspace__{id}`. Regenerate `public-api.txt` |
| `UsageActivity` | New variant `Workspace`, label **Workspaces** |
| `NotificationCategory` | New `workspace_message`, one row per message |
| `notification` | New nullable columns `workspace_id` and `author`, added with `add_column_if_missing` and `0004_workspaces.sql` |
| `ToolCategory` | New `Workspaces`, category name `workspaces` |
| `AVAILABLE_PERMISSIONS` | `workspaces:create`, `all_workspaces:manage` |
| `session_history` | New index on `(channel, topic, timestamp)` for the timeline read. Holds copied reads as `Request(SilentRead)`, with `metadata.workspace_entry` |

**Notifications visibility**: the list clause `agent_id IN (visible)` gains `OR (workspace_id IN (my Workspaces) AND author != me)`, since a member may be notified about an agent they can't view (research R20). `agent_id` for such a row is the author agent's id, or empty for a person. Nothing is deleted when a member leaves, because the clause reads current membership.

## Deletion

| Deleted | Effect |
|---|---|
| Topic or Workspace | Stops its turns, deletes each agent's sessions for it (history, files, scratchpads) by channel slug, including former members', and its notifications. Rows cascade. Agents' memory and recorded usage stay (FR-025) |
| Member agent | Member row goes, turns in it are aborted, and entries stay with `author_name` |
| Agent itself | `delete_agent` removes its `workspace_member` and `workspace_read` rows. Entries stay |
| Member person | Member row goes, so its notifications stop showing to them. Entries stay |

## State transitions

**Message from a person**: insert entry and mentions (one transaction) → publish `workspace_entry` → dispatch agent mentions → publish `workspace_mentions`.

**Agent text saved**: `save_session_history` inserts entry and resolved mentions → `after_save` publishes and dispatches, applying the stop guard.

**Activation**: `Chat` request arrives at `chat` → `catch_up` up to `metadata.workspace_entry - 1` → the normal turn runs. A resumed turn skips `catch_up`.

## Migration and compatibility

No existing rows change. Tracked on #82: `InterAgent`, `JobKind::Delegation`, `delegated_to`, the retired tool hints, `delegate_agent` in `JOB_LAUNCHING_TOOLS` and `MessageList`'s delegation label.
