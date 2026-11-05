# Research: Realtime Notifications

No unknowns remained in the Technical Context; these are the design decisions, each checked against the code as it stands on `develop` (2076fead).

## R1. Delivery: one WebSocket per client, server to client only

**Decision**: `GET /api/v1/inbox/stream`, a WebSocket that only pushes. Reads stay on the REST routes; marking read stays `PUT /notifications/{id}/read` and `POST /notifications/read-all`.

**Rationale**: The WebUI already has `TopicSocket` (backoff, connection states, retry), `require_auth` already accepts the token as a query parameter for sockets, and `Message::Ping` keepalive already exists. Keeping writes on REST means no second write path, and every REST call stays in the conformance suite.

**Alternatives**: Server-sent events fit a read-only stream better, have built-in reconnection, and could authenticate with the same query token. Rejected because of the browser's connection limit: `vizier-http` serves plain HTTP through `axum::serve` with no TLS, so browsers use HTTP/1.1 unless a proxy adds HTTP/2, and HTTP/1.1 allows about 6 connections per host across all tabs. Each SSE stream holds one open, so six tabs would hang every REST call, while WebSockets don't count against that limit. Closing the stream in hidden tabs would mitigate this, but that is extra logic to guard against a failure WebSocket doesn't have. A per-user multiplex over each chat socket: a person with no chat open would get nothing. Putting read/write frames on the socket: duplicates authorization and validation already in `service::notifications`.

## R2. Desktop: the same stream through a Tauri Channel

**Decision**: `vizier_http::inbox::InboxConnection` is driven by both the WebSocket handler and the app's IPC (`inbox_open`/`inbox_close`), and `Dispatcher::open_inbox` mints the desktop principal, exactly as `TopicConnection` and `Dispatcher::open_topic` do. `Transport` gains `openInbox`.

**Rationale**: The app opens no port by default (spec 038), so a WebSocket is not available there. This follows the seam that already exists, and `boundary.test.ts` keeps socket code inside `webui/app/api/transport/`.

## R3. Fan-out across processes: the Fleet bus, through the existing pump

**Decision**: A new fan-out subject `inbox.events`. `transport/events.rs` gains `Outgoing::Inbox(InboxEvent)` and a third `select!` arm feeding a `broadcast` channel, with `publish_inbox_event` and `subscribe_inbox_events`. Each connection subscribes before it reads its snapshot.

**Rationale**: This is the established pattern for `session.>` and `integration.changed`. Events published on a worker reach a gateway on another node (FR-009) with no new mechanism. Events are small: a notification's excerpt is capped, well under the 1 MiB envelope limit.

**Alternatives**: A subject per person: a notification's audience is usually "everyone who may view this agent", which only the gateway can evaluate. Reusing `session.{id}.events`: setup and agent-wide rows have no session.

## R4. Visibility: evaluated per delivery, at the gateway

**Decision**: Events on the bus carry their audience facts, and each connection decides per event: re-resolve the principal (`Services::resolve`, which re-reads role and permissions), then apply the same rules as the REST path (`visible_agents`, `audience_user_id`, the setup rules in `service::setup_state`). A person who no longer resolves has their connection closed. Share those checks with `service::setup_state::states` and `service::notifications` rather than copying them.

**Rationale**: Revocation must apply at once (FR-007, SC-009), and a principal captured at connect would not notice. The cost is bounded because progress is throttled (R6) and events are otherwise infrequent.

**Note**: `SetupState.owner_id` is `#[serde(skip)]`, so it does not survive the bus. It decides who sees the setup of an agent still being created, so `InboxEvent::Setup` carries it as a separate field, and the frame sent to clients omits it.

## R5. Recovery: a snapshot on connect, then a high-water mark

**Decision**: On open, the connection subscribes, then sends `hello { counts, setups, latest_id }`. The client compares `latest_id` with its `lastSeenId`; if it is behind, it fetches the newest page over REST and runs the same "fresh" handling used today (auto-read of the conversation on screen, one toast for a burst). `resync` tells the client to do the same when the server may have missed something: a lagged `broadcast`, a deleted conversation or agent, or a changed title.

**Rationale**: Ids are monotonic and the store already dedupes by id, so missed rows appear once and doubled rows are impossible (SC-004). This keeps `start()`'s existing logic and removes only its timer. No server-side replay buffer is needed, because the table is the buffer.

**Alternatives**: Client-supplied `since` with server replay: more code for the same result, and it needs a retention rule of its own.

## R6. Setup progress: throttle only the noisy call

**Decision**: `SetupBoard` publishes an `InboxEvent::Setup` from its private `publish`, gated per agent to one every 250 ms **only for `download_progress`**. Every other transition (started, finished, failed, a download starting or ending) is sent immediately, so a final state can never be swallowed. When the entry has been pruned (a lone download finished, so `snapshot()` no longer returns it), it publishes `SetupCleared`.

**Rationale**: `download_file` reports per MiB, which is many per second on a fast link. FR-014 asks for a few per second. Terminal states matter most and arrive at most a handful of times.

**Client side**: The board's 5 s `TERMINAL_TTL` runs only on read, so a client cannot wait for a server message when a terminal entry expires. The store removes a `ready`/`failed` entry itself after 5 s. ETA samples move from `refreshSetups` into the frame handler unchanged.

## R7. Excerpt: built at write time, from the saved message

**Decision**: A new pure function in `notifications/excerpt.rs` renders Markdown to plain text using `pulldown-cmark` (already a `vizier-core` dependency), drops code block bodies to a short marker, collapses whitespace and cuts on a character boundary at 140 characters with an ellipsis. Stored in a new nullable `excerpt` column.

- **Chat reply**: from the saved history entry addressed by `history_uid`, not the in-flight response, so only what has been through saving can appear (FR-022). `storage/history.rs` has no by-uid read today; add one. No text: "Sent a page" (a turn ended by `ui_render`), "Sent N files", "Sent a voice reply".
- **Task run**: `storage::task_run::task_run_response` already returns a run's answer and is `None` unless answered, so `task_run_ended` gains the excerpt without changing its callers.
- **Background job**: the outcome text it already builds. No model output.

**Rationale**: Writing once keeps listing cheap and makes the live event self-contained. Retention already limits how long it stays. Excerpts are stored as plain text, as conversation history is (`docs/src/configuration/encryption.md` lists both as readable), and that page needs the word "excerpts" added.

**Note**: `summary` keeps its rule ("never model output"), and the existing `a_chat_reply_notification_carries_no_content` test still holds. The excerpt is a separate field, so a client that ignores it is unaffected.

## R8. Title: filled on read, not stored, for conversations

**Decision**: A new `source_session` column holds the session as JSON. `service::notifications` fills `title` from the session's current detail on every read, once per distinct session on the page, the way `fill_reactions` and `fill_client_calls` fill theirs. A task's title has no session, so it is stored in `source_title`, written when the run ends.

**Rationale**: `session_detail` keeps the title inside a JSON `data` blob, so a SQL join would need a per-dialect `json_extract`. Filling on read shows renames at once and needs no write when a title changes. When a title first arrives (`SessionFrame::Title`), the person's connections get `resync`, so "New conversation" is replaced live.

**Visibility of a title** (FR-023): an agent-wide row (a background job) can point at one person's conversation. The title is filled only when the session's `web_user()` is `None` or equals the viewer. Anyone else sees the row's text alone. `job_ended` takes the origin session in place of the agent id.

## R9. External channels: one rule, at creation

**Decision**: `VizierChannelId::platform().is_some()` means external. `job_ended` returns early on it, and `chat_replied` already requires `web_user()`. Remove the projection outright: `project_replies`, `channel_reply_batches`, `ReplyBatch`, `REPLIES_THROUGH`, `text::channel_replies`, `channel_href` if nothing else uses it, the `project()` calls in `service::notifications`, and the `ChannelReplies` variant.

**Existing rows**: delete them in the schema path. SQLite: an idempotent `DELETE FROM notification WHERE category = 'channel_replies'` in `init_notification_schema`. PostgreSQL: the same in migration `0003`. Read rows cascade. Add the SQLite statement and the leftover `notifications.replies_through` state key to #82.

**Rationale**: `platform()` is already the single answer to this question, so a future platform added there is excluded with no inbox change (FR-028). Deleting beats filtering: `list_notifications` drops unparseable categories in Rust while `notification_counts` counts them in SQL, so merely removing the variant would leave phantom unread counts.

**Removing the projection also fixes** a cost: every list and count call currently runs a write before reading.

## R10. Socket lifetime

**Decision**: The inbox loop has no idle deadline. It pings every 30 s like `handle_socket`, and a failed write ends it. On a network connection it also closes at the token's expiry: `require_auth` has the claims, so it passes the expiry to the handler as an extension. Reconnection then gets `401` on the upgrade. After `OFFLINE_AFTER` failures, the client makes one `GET /notifications/count` so a real sign-out goes through the existing `settle` 401 handling.

**Rationale**: `handle_socket` closes after `ws_idle_timeout_secs` (default 300) with no client frames. A push-only socket never sends any, so reusing that loop would drop every quiet inbox after five minutes. A socket's upgrade status is not visible to browser code, hence the count probe.

## R11. Popup: one instance, anchored to the mounted bell

**Decision**: `AppShell` renders one `NotificationsPopup` using design's `Floating` (`align="end"`, width `min(360, viewport − 16)`) and `useDismiss`, with the bell as `ignore` so pressing it toggles. The bell registers its element in the store on mount. Remove `NotificationsAside`, `NotificationsPanel` and the `aside` usage in `AppShell`. If nothing is registered, such as when a toast or the desktop app's `/notifications` link opens the inbox on a page with no bell, the popup anchors to a fixed spot at the top right.

**Rationale**: Bells mount in four places (`AppShell`, `home.tsx`, `ConversationHeader`, `PageFrame`), while toasts, `AgentStateBanner` and `root.tsx` all call `openPanel`. One popup avoids per-bell copies. `Floating` is already portalled, fixed-position and flips when there is no room, so the page does not move (SC-005), and its `maxHeight` lets the list scroll inside.

**Read filter**: the store's `filter` type narrows to `unread | all` in the WebUI. The REST `read` filter stays for API clients.

**Bell**: `IconButton` keeps `count` (the number, as decided) and its existing `busy` indicator while an agent sets up.

## R12. New design components

**Decision**: `NotificationItem` and `SetupItem` are new presentational components in `design/` with stories, using `Sigil` and `ProgressBar`, and the WebUI maps store data onto them. This follows the rule that a new visual goes design → webui → vizier, after the FigCraft drawing. Check that `ProgressBar` draws an indeterminate state for an unknown size; add it to the component if not.
