# Contract: HTTP, Inbox Frame and UI

All routes sit under `/api/v1/agents/{agent_id}/projects/{project_id}`. Authorization is in
`service::project_lsp` through `projects::access`. Each route gets a vector in
`tests/conformance/groups/projects.rs` and an OpenAPI entry.

## Changed routes

| Route | Change |
|---|---|
| `POST …/projects` | Optional `lsp_enabled`, default `true` |
| `PATCH …/{project_id}` | Optional `lsp_enabled`. Requires `can_manage`. Off stops servers at once |
| Project summary and detail | Gain `lsp_enabled` |

## New routes

### `GET …/lsp`

Needs `can_use`. Returns `LspStatusSnapshot` (data-model §6), and for someone who can manage the project:

```json
{
  "...": "LspStatusSnapshot",
  "can_manage": true,
  "languages": [{ "id": "rust", "name": "Rust", "default_command": "rust-analyzer" }],
  "overrides": [
    {
      "language": "rust",
      "command": "/opt/rust/nightly/bin/rust-analyzer",
      "args": [],
      "env": { "RA_LOG": "warn" }
    }
  ]
}
```

Other viewers get `can_manage: false`, with no `overrides` and no `default_command`.

### `PUT …/lsp/overrides/{language}`

Needs `can_manage`. Body: `{ "command": "...", "args": [], "env": {} }`. Saves, then stops that
language's servers in the project, so the next use starts the new one. Returns the snapshot.

| Status | Code | When |
|---|---|---|
| 404 | `unknown_language` | Not in the built-in table |
| 422 | `invalid_override` | Fails the data-model §2 rules |

### `DELETE …/lsp/overrides/{language}`

Needs `can_manage`. Removes it and stops that language's servers. Returns the snapshot. Deleting an
absent override succeeds, so a retry is safe.

### `POST …/lsp/restart`

Needs `can_manage`. Body: `{ "language": "go", "root": "tools/gen" }`. `root` is optional; without it,
every root of that language restarts. This is both **Restart** and **Check again**: it clears the
restart count and any `failed` or `missing` entry, then starts again in the background. Only roots
already listed are accepted, so it never starts a server somewhere new. Returns the snapshot.

| Status | Code | When |
|---|---|---|
| 409 | `lsp_disabled` | The switch is off |

## Inbox stream

Additive to `specs/051-realtime-notifications/contracts/inbox-stream.md`.

- **Event**: `InboxEvent::ProjectLsp { agent_id, project_id, snapshot }`.
- **Frame**: `{ "type": "lsp", "project_id": "...", "snapshot": LspStatusSnapshot }`.
- **Who**: `judge_with` delivers it to people who can use the project, reusing the rule behind
  `can_use`. The snapshot holds no override values.
- **When**: on every state change, plus progress throttled to 250 ms per project.
- **Recovery**: on `hello` or `resync`, the client re-reads `GET …/lsp` for the project on screen.

## UI

Reviewed mockup: [`../mockups/lsp-status-popup.html`](../mockups/lsp-status-popup.html). Changes follow
the design → webui → vizier order.

**`design/` (with stories)**

- `StatusStripItemId` gains `lsp`.
- `StatusStripItem` gains `dot?: "live" | "pulse" | "warn" | "fail"`. `busy` stays and means `live`.
- `StatusStripItem` gains `display?: string`, so the item can read `off` instead of a number.
- `StatusPanel` gains `headerAction?: ReactNode`, between `meta` and the ×, for the switch.

**`webui/`**

- `screens/chat/LspPanel.tsx`, `lib/lsp.ts` (pure: grouping by language, worst dot, meta line, with
  vitest tests), and project LSP calls in `api/endpoints/projects.ts`.
- `ChatStatusStrip` gets an `lsp` prop, used only in project chats.
- `lib/inboxFrames.ts` handles the `lsp` frame.
- `schema.d.ts` regenerated.

**Behaviour**

| Item | Rule |
|---|---|
| Strip number | Ready servers |
| Strip dot | Red if any failed, otherwise amber if any missing, otherwise pulsing green if any starting, otherwise none |
| Grouping | By language, since an override applies to all its roots |
| Rows | Only roots that have been used |
| Viewers | No switch, no **Configure**, no **Add override**, no **Restart**, no **Check again** |
| Warning | None. **By decision, no text about running code appears in the panel or the open-project dialog** |
| Dismissal | ×, Escape, press outside (`StatusStrip` already does this) |
