# Artifacts

Artifacts are the **permanent** file tier beside a conversation's transient **session files**.

An agent or a person promotes a session file into a named, agent-owned artifact. Promotion
**copies** the bytes; the source stays in `uploads/` and is not marked, moved or linked. Clearing
the source conversation, deleting it, or deleting the agent therefore cannot affect the artifact.

## Storage

- Metadata, change events and references live in SQLite (`artifact`, `artifact_event`,
  `artifact_reference`).
- Content lives at `<workspace>/artifacts/<agent_id>/<artifact_id>/content.v<n>`.
- Session files stay under `<workspace>/uploads/<file_id>/`. No artifact content is ever written
  under `uploads/`, and no session-file content is written under `artifacts/`.
- The display name is metadata only; the content file name is server-generated, so Unicode or
  Windows-reserved characters in a name never become a filesystem path.

## Promoting and replacing

- An agent promotes a file itself, or you use the chat side panel's **Promote** action (you need the artifact **manage** permission).
- A name is unique per agent and case-sensitive. Promoting a duplicate name is refused with a
  message pointing to replacement.
- Replacing an artifact keeps its id and name, increments the version and
  writes a new `content.v<n>` before switching the row. The previous version file is removed
  after the switch. A failed source read or transaction leaves the old content and version
  intact.
- Artifacts have **no version history or restore**. The version counter and change log record
  what happened; only the current content is kept.

## Clearing session files and deletion

There is **no automatic garbage collection, retention period or background collector** in this
release. Session files are removed only by:

- the chat side panel's **Clear session files** action (`DELETE …/files`);
- `/clear-files` on Discord, `/clear_files` on Telegram, both two-step: request, then `confirm`
  within 60 seconds;
- deleting the conversation (`DELETE …/topic/{topic_id}`);
- deleting the agent.

A clear is refused while a turn is running or queued in that conversation. Blobs are reference
counted: a `file_id` still referenced by another conversation's session-file record is kept until
its last reference goes.

Deletion is reference-only in both directions:

- Deleting a conversation or memory leaves the artifact, its events and its content intact.
- Deleting an artifact removes its content, references and row, but leaves message and memory
  bodies intact. Messages keep the artifact id and read as **artifact unavailable**; memories
  lose the attachment link and record a revision.

## Permissions

Four permissions gate the gallery and its content. They are additive to the agent's other pages
and never grant them.

| Permission | Allows |
|---|---|
| `all_agents:view_artifacts` | See, preview and download every agent's artifacts |
| `owned_agents:view_artifacts` | The same for agents the person owns |
| `all_agents:manage_artifacts` | Promote and delete on every agent |
| `owned_agents:manage_artifacts` | Promote and delete on owned agents |

`manage` implies `view`; the system role bypasses both. People with view-only access may browse
and download but cannot promote or delete. A person without view permission gets no sidebar
entry, a `403` from every gallery/content route, and sees a message attachment as a named
unavailable file.

## Pointing a memory at an artifact

Memory attachments were removed: a memory cites an artifact with an **entity link** in its body
(`[[artifact:name]]`, see [Entity links](/docs/configuration/entity-links)) — one mechanism for
chat and memory alike. A memory document that still carries the old `attachments:` frontmatter
parses fine; the key is ignored and drops on the next rewrite.

## Dreams and source conversations

A dream cycle may read the files of the conversations it reflects on and promote one of them to an artifact. It cannot replace or delete artifacts, send attachments or clear files, and it never writes to the source files.

## API

```
GET    /api/v1/agents/{agent_id}/artifacts                     list (q, kind, from, to, cursor, limit)
GET    /api/v1/agents/{agent_id}/artifacts/activity            change log
GET    /api/v1/agents/{agent_id}/artifacts/{id}                detail
GET    /api/v1/agents/{agent_id}/artifacts/{id}/content        current bytes (Range supported)
DELETE /api/v1/agents/{agent_id}/artifacts/{id}                delete (manage)
POST   /api/v1/agents/{agent_id}/channel/{c}/topic/{t}/files/promote
DELETE /api/v1/agents/{agent_id}/channel/{c}/topic/{t}/files   clear session files
```

Message attachments use `VizierAttachmentContent::Artifact { agent_id, artifact_id, available }`;
`available` is filled at read time and never stored. Old `local`/`url`/`bytes`/`base64`
attachments keep working as legacy data.
