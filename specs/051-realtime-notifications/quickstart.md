# Quickstart: Realtime Notifications

End-to-end checks against a running binary with a `dummyplug` agent (no keys, no network). `scripts/inbox-e2e.mjs` scripts the steps marked **(script)**; the rest need a browser.

## Setup

```bash
just run
```

Create agent `alpha` on provider `dummyplug` from the WebUI or `POST /api/v1/agents`, and sign in. Open a socket to `/api/v1/inbox/stream?token=<token>` and log every frame.

## 1. Connecting (script)

1. Open the stream. **Expect** `hello` first, with `counts`, `latest_id` and `setups`.
2. Leave it open for 6 minutes. **Expect** it stays open, past the 5-minute chat idle timeout.

## 2. A reply notifies live (script)

1. In a new WebUI topic on `alpha`, send `tools`. **Expect** a reply listing tools.
2. **Expect** within 2 seconds a `notification` frame: `category: chat_reply`, the topic's `title` or "New conversation", an `excerpt` of the reply as plain text of at most 141 characters, and `counts.unread` up by one.
3. `GET /notifications?filter=unread`. **Expect** the same item.
4. If the title arrives after the reply, **expect** `resync`, and the reloaded row shows the title.

## 3. Read state crosses clients (script)

1. Open a second stream as the same person and a third as another user who can view `alpha`.
2. `PUT /notifications/{id}/read` with `{ "read": true }`. **Expect** `read` on the first two streams within 2 seconds, and nothing on the third.
3. **Expect** the third stream never received the chat reply's `notification`.

## 4. Task runs and failures (script)

1. Create a one-time task on `alpha` due now. **Expect** a `notification` with `category: task_run`, the task's title and an excerpt of its answer.
2. `GET /agents/alpha/tasks/{slug}/runs`. **Expect** the run closed as `answered`.

## 5. Setup is live (script for frames, browser for the row)

1. Create agent `beta` with a local embedding model that must download. *(Needs network for the model; skip offline and rely on the unit tests in `setup.rs`.)*
2. **Expect** `setup` frames from `starting` to `downloading`, no more than about 4 per second, then `ready`.
3. **Expect** no periodic `GET /utils/setup-state` or `GET /notifications` in the browser's network panel.
4. Drop the network while it downloads, then restore it. **Expect** `hello` with the current state.

## 6. Reconnect recovers (script)

1. Close the stream and note `lastSeen`. Send `alpha` two more messages from another connection.
2. Reconnect. **Expect** `hello.latest_id` above `lastSeen`. Fetching the page returns both rows once.
3. In the browser: **expect** one toast for the burst and a correct bell number.

## 7. Revocation and visibility (script)

1. Second user holds a stream and can view `alpha`. Remove their access.
2. Send `alpha` a task run. **Expect** nothing on that stream.
3. Delete their user. **Expect** the stream closes at the next event.
4. Create agent `gamma` as user A. **Expect** user B, with no all-agents permission, gets no `setup` frame for it.

## 8. Deleting a conversation (script)

1. Delete the topic from step 2. **Expect** `resync`, and the row is gone from `GET /notifications` with counts lowered.

## 9. External channels

Needs Discord or Telegram credentials, so marked **live**. Covered offline by unit tests on `chat_replied` and `job_ended`, and the projection no longer exists.

1. **(live)** Have the agent reply on Discord or Telegram. **Expect** no frame, then or after the hour closes.
2. **(live)** Launch a background job from that conversation. **Expect** no `notification` when it ends, and the report still posts in the channel.
3. **(live)** Change that integration in Settings. **Expect** a `notification` with `category: integration`.
4. Upgrade a workspace that holds `channel_replies` rows. **Expect** them gone and the counts correct.

## 10. Popup (browser)

At desktop and 375 px widths, in light and dark, and one non-default color theme:

1. Press the bell. **Expect** the popup anchored to it and page content moved by 0 pixels.
2. Close it with ×, Escape and a press outside. Press the bell twice. **Expect** it toggles.
3. **Expect** order: filters with icons, then setup when running, then rows, then "Load older". No title bar; filters have no counts; the bell shows the unread number.
4. **Expect** rows: sigil, title, excerpt, then agent and time; no dot; unread text heavier than read. The eye appears on hover and focus, and is reachable on touch.
5. Open the app on a page and select a toast. **Expect** the popup opens.
6. Select a row. **Expect** it is read, the popup closes and you land on its page.

## 11. Desktop app

```bash
just app-dev
```

1. **Expect** steps 2 and 3 work with no network port open, and `inbox_open` is the only stream.
2. Hide the window and let a task finish. **Expect** an operating system notification, and selecting it opens the row.
3. Restart the engine. **Expect** the stream reopens and `hello` arrives.

## Automated

```bash
cargo test -p vizier-core notifications
```

```bash
cargo test -p vizier-http --test inbox_stream
```

```bash
cargo clippy --workspace
```

```bash
cd webui && npm run typecheck && npm test
```
