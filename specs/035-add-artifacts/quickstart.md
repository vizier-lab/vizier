# Quickstart: Permanent Artifacts

**Feature**: `035-add-artifacts`

This checks the feature against a running binary with an offline `dummyplug` agent, per the constitution's e2e gate. The dummyplug protocol is `specs/008-dummyplug-provider/contracts/dummyplug-protocol.md`; agent tool calls go through `execute_python` (code mode). Steps marked **LIVE** need a real Discord/Telegram connection and are not part of the offline gate.

Run against a scratch workspace:

```sh
rm -rf /tmp/vizier-artifacts
cargo run -- run --workspace /tmp/vizier-artifacts --port 9999 2>&1 | tee /tmp/vizier-artifacts.log
```

On first run create the admin (or complete onboarding in the browser):

```sh
curl -s localhost:9999/api/v1/auth/setup \
  -H 'content-type: application/json' \
  -d '{"username":"admin","password":"admin"}'
```

In another shell:

```sh
TOKEN=$(curl -s localhost:9999/api/v1/auth/login \
  -H 'content-type: application/json' \
  -d '{"username":"admin","password":"admin"}' | jq -r .data.token)
API=localhost:9999/api/v1
auth=(-H "authorization: bearer $TOKEN" -H 'content-type: application/json')
DB=/tmp/vizier-artifacts/.runtime/vizier.db
TOPIC=artifact-e2e

# One dummyplug turn over the REST chat endpoint. The body is ChatRequest:
# { channel_id, topic_id, content } where content is the message text.
chat() {
  jq -cn --arg c "$1" --arg t "$TOPIC" \
    '{channel_id:"vizier-webui",topic_id:$t,content:$c}' \
    | curl -s "${auth[@]}" "$API/agents/dp-a/chat" -d @- | jq .data
}
```

Create a dummyplug agent `dp-a`:

```sh
curl -s "${auth[@]}" "$API/agents" -d '{
  "agent_id":"dp-a","name":"Dp A","description":"artifacts quickstart",
  "provider":"dummyplug","model":"dummyplug"
}' | jq .data.agent_id
```

Create the source files the agent will copy:

```sh
printf 'artifact v1\n' >/tmp/artifact-e2e.txt
printf 'artifact v2\n' >/tmp/artifact-e2e-v2.txt
```

## 1. Promote a session file (US1, FR-009–FR-012)

Ask the dummyplug agent to load a local path as a session file and promote it in one `execute_python` script:

```sh
chat '{"tool":"execute_python","arguments":{"code":"load_path_as_session_file(path=\"/tmp/artifact-e2e.txt\")\nprint(promote_artifact(filename=\"artifact-e2e.txt\", name=\"e2e-artifact.txt\"))"}}'
```

Expect the report to contain the artifact name `e2e-artifact.txt`, id, kind `text`, size 12, version 1. The conversation is `http__admin__vizier-webui` / topic `artifact-e2e`; open `/dp-a/chat/artifact-e2e` in the WebUI if needed.

Check the list and the filesystem:

```sh
curl -s "${auth[@]}" "$API/agents/dp-a/artifacts" | jq '.data'
find /tmp/vizier-artifacts/artifacts -type f -print
find /tmp/vizier-artifacts/uploads -type f -print
```

Expect:

- one list item with `name: "e2e-artifact.txt"`, `kind: "text"`, `version: 1`, `available: true`, `agent_total: 1`;
- one file under `artifacts/dp-a/<artifact-id>/content.v1`;
- the source still under `uploads/<file_id>/artifact-e2e.txt`;
- no artifact content under `uploads/`.

## 2. Replace the artifact (US1, FR-013–FR-016)

Run a second script that loads the v2 file and updates the artifact:

```sh
chat '{"tool":"execute_python","arguments":{"code":"load_path_as_session_file(path=\"/tmp/artifact-e2e-v2.txt\")\nprint(update_artifact(name=\"e2e-artifact.txt\", filename=\"artifact-e2e-v2.txt\"))"}}'
```

Get the artifact id and download both versions:

```sh
ART=$(curl -s "${auth[@]}" "$API/agents/dp-a/artifacts" | jq -r '.data.items[] | select(.name=="e2e-artifact.txt") | .id')
curl -s "${auth[@]}" "$API/agents/dp-a/artifacts/$ART/content"
ls -la /tmp/vizier-artifacts/artifacts/dp-a/$ART
```

Expect `artifact v2`, `version: 2`, and only `content.v2` in the directory (`content.v1` removed after the switch). The source v1/v2 session files may both remain; nothing marks them.

A failed replace must leave the old content:

```sh
chat '{"tool":"execute_python","arguments":{"code":"print(update_artifact(name=\"e2e-artifact.txt\", filename=\"no-such-session-file.txt\"))"}}'
curl -s "${auth[@]}" "$API/agents/dp-a/artifacts/$ART/content"
```

Expect an error naming the missing session file, and still `artifact v2`.

## 3. Clear session files without touching artifacts (US3, FR-033–FR-043)

The conversation the agent used is `channel_id = vizier-webui`, `topic_id = artifact-e2e`:

```sh
curl -s "${auth[@]}" -X DELETE "$API/agents/dp-a/channel/vizier-webui/topic/$TOPIC/files" | jq .data
```

Expect `{removed: N, bytes: ...}`. Check:

```sh
sqlite3 "$DB" "select count(*) from session_file
               where agent_id='dp-a'
                 and session_slug like '%vizier-webui__artifact-e2e';"
find /tmp/vizier-artifacts/uploads -type f -print
curl -s -o /dev/null -w '%{http_code}\n' "${auth[@]}" "$API/agents/dp-a/artifacts/$ART/content"
```

Expect `0` session-file rows for that conversation, no source upload directory, and `200` for the artifact. The artifact directory still contains `content.v2`.

Cancelled/empty clears are client/UI behavior; the endpoint on an empty conversation returns `{removed: 0, bytes: 0}`. Active-turn refusal is covered by the unit tests (`SessionFiles::has_active_turn`) and the side-panel disabled state; a live check can hold a long tool turn and call the endpoint, expecting `409`.

## 4. Attach an artifact to a reply (US4, FR-044–FR-053)

Send an artifact through `send_attachment`:

```sh
chat '{"tool":"execute_python","arguments":{"code":"print(send_attachment(artifact=\"e2e-artifact.txt\"))"}}'
```

The response `attachments` must contain one entry whose content is `{"artifact":{"agent_id":"dp-a","artifact_id":"<id>"}}` and whose filename is `e2e-artifact.txt`. Send a plain session file to `send_attachment` and expect the "promote it first" error. A person's request attachment that carries artifact content must be refused; only the agent/tool response path creates artifact references.

Read the topic history:

```sh
curl -s "${auth[@]}" "$API/agents/dp-a/channel/vizier-webui/topic/$TOPIC/history" \
  | jq '.data[] | select(.content.Response.attachments != null) | .content.Response.attachments'
```

Expect one saved response attachment with `available: true`; after deleting the artifact (step 6) the same history entry must read `available: false`.

## 5. Memory attachments (US4, FR-046–FR-050)

Write a memory and attach the artifact with dedicated calls:

```sh
chat '{"tool":"execute_python","arguments":{"code":"print(memory_write(title=\"Q3 planning\", content=\"Plan.\", path=\"q3-planning\"))"}}'
chat '{"tool":"execute_python","arguments":{"code":"print(attach_artifact_to_memory(path=\"q3-planning\", artifact=\"e2e-artifact.txt\"))"}}'
```

Expect the updated attachment list with `e2e-artifact.txt`. Repeat the attach and expect a no-op (one entry). Update the memory content without attachments:

```sh
chat '{"tool":"execute_python","arguments":{"code":"print(memory_write(title=\"Q3 planning\", content=\"Plan v2.\", path=\"q3-planning\"))"}}'
chat '{"tool":"execute_python","arguments":{"code":"print(memory_read(path=\"q3-planning\"))"}}'
```

The attachment is still there. Clear the source conversation (step 3 again if a new source file exists), then `memory_read` must still list/load the artifact. Detach:

```sh
chat '{"tool":"execute_python","arguments":{"code":"print(detach_attachment_from_memory(path=\"q3-planning\", name=\"e2e-artifact.txt\"))"}}'
```

Expect an empty attachment list; `memory_read` no longer lists it.

Try the old path: `memory_write(..., attachments=["artifact-e2e.txt"])` through a script and expect a deserialization/unknown-field error; call the memory create/update API with `attachments: [...]` and expect `422 use_attach_artifact`. The companion `attach_artifact_to_memory` is the only new path.

### 5b. Legacy memory attachment fixture (compatibility)

Create a memory through the API, copy its markdown path from `agents/dp-a/memory/default/`, and add a legacy attachment to its frontmatter using a real `file_id` from an uploaded file (run step 1 again if needed):

```yaml
attachments:
  - filename: legacy.txt
    content:
      local: /api/v1/files/<file_id>
attachment_count: 1
```

Then read the memory through the API and the agent. Expect the legacy attachment to appear with its old shape, `memory_read` to copy it into the session, and `detach_attachment_from_memory(path="<path>", name="legacy.txt")` to remove it. This is the read-only legacy path tracked in #82; no migration runs.

## 6. Delete an artifact and see the tombstone (US2/US4)

```sh
curl -s "${auth[@]}" -X DELETE "$API/agents/dp-a/artifacts/$ART" | jq .data
curl -s -o /dev/null -w '%{http_code}\n' "${auth[@]}" "$API/agents/dp-a/artifacts/$ART/content"
```

Expect `{removed:true, references:N}`, then `404` for the content. Read the topic history: the saved attachment must have `available:false` (or the WebUI shows `artifact deleted`); the memory (if still attached) must report it deleted by name. The activity view has a `deleted` record.

## 7. Conversation deletion (US3)

Create a throwaway WebUI topic with a session file (repeat the load/promote script or upload through the API), then delete the topic:

```sh
curl -s "${auth[@]}" -X DELETE "$API/agents/dp-a/channel/vizier-webui/topic/<topic>" | jq .data
```

Expect the topic's `session_file` rows and upload blobs to be gone, the artifact promoted from it to remain downloadable, and a legacy memory referencing that topic's session file to show unavailable. Deleting a topic with a running/queued turn must return `409` (unit test; live optional).

## 8. Agent deletion (US3/FR-041)

```sh
curl -s "${auth[@]}" -X DELETE "$API/agents/dp-a" | jq .data
sqlite3 "$DB" "select count(*) from artifact where agent_id='dp-a';
               select count(*) from artifact_event where agent_id='dp-a';
               select count(*) from artifact_reference where agent_id='dp-a';
               select count(*) from session_file where agent_id='dp-a';"
find /tmp/vizier-artifacts/artifacts/dp-a /tmp/vizier-artifacts/uploads -maxdepth 1 -print 2>/dev/null
```

Expect four zeros and no `artifacts/dp-a` directory. Other agents' artifacts and uploads must be untouched.

## 9. Gallery, side panel and phone (US2/US3)

Manually (or with `cd webui && npm run e2e -- artifacts.spec.ts` when a browser is available):

1. With at least two artifacts, open the agent's **Artifacts** page. Check the sidebar entry, the total count/bytes, newest-first order, name search, kind/date filters, paging and the Activity tab.
2. Preview an image, a PDF, a CSV, markdown and a text artifact; check the CSV table/raw switch and the unsupported-type download message. Download one and compare bytes.
3. Open the detail: origin conversation, source filename, current version, change history and "Used by" message/memory links. Delete one as the owner; confirm the dialog names it and the reference count; the row disappears and references show the tombstone. Repeat as a view-only user: no delete action and a direct delete returns `403`.
4. In a chat, use **Promote** on a session file: name dialog, success toast, source still listed. Use **Clear session files**: confirm dialog names the count and warns about old references; the list empties; artifacts remain. The action is disabled while the turn is running.
5. At a phone viewport, the gallery uses stacked cards, the detail is full-screen, filters wrap, and the side panel opens as a sheet. Check keyboard focus/Escape and screen-reader names.
6. A reply with an artifact attachment shows the current file; replace the artifact and reload the message; delete it and confirm the named tombstone. A legacy session-file attachment still renders as before.

`npm run typecheck`, `npm test` and the Playwright spec cover the helper and store paths; record the manual checks if the browser cannot start.

## 10. Channel clear command — unit + LIVE

Offline:

```sh
cargo test clear_confirmations
cargo test discord_builtin_commands
cargo test telegram_builtin_commands
```

LIVE (a bot with a channel):

1. In a Discord channel, send `/clear-files`. Expect the count and the two-step prompt. Send `/clear-files confirm`; expect `Cleared N session files. Artifacts are untouched.` A second confirm without a request says no clear is waiting.
2. In Telegram, use `/clear_files` and `/clear_files confirm`; the command menu shows `/clear_files`.
3. Attach an artifact larger than the platform's limit and send it. The reply text must arrive and a short explicit attachment failure must name the file; the artifact remains in the gallery.

## 11. Final checks

```sh
cargo fmt --check
cargo clippy --all-targets
cargo test
cd design && npm run typecheck && npm run build-storybook
cd webui && npm test && npm run typecheck
```

All must pass. Verify:

```sh
grep -R "retention\|garbage collect\|GC" src/session_files src/artifacts || true   # no automatic collection code
find /tmp/vizier-artifacts/uploads -type f -path '*/artifacts/*' | wc -l           # 0
```

Confirm the legacy `MemoryAttachment::LegacySessionFile` read path and the `422 use_attach_artifact` refusal are named in the #82 comment (the comment URL is in the issue), and that no legacy data was migrated. Delete `/tmp/vizier-artifacts` when done.

## Recorded run

When the implementation lands, record the command, expected/observed values and any skipped LIVE steps here, as the other feature quickstarts do.
