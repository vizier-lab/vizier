# 2.6 Storage & Shell

## Storage

```yaml
vizier:
  storage:
    type: sqlite
```

SQLite is the only runtime backend. The database lives at `<workspace>/.runtime/vizier.db` and holds agents, sessions, history, tasks, users/roles/API keys, providers, dream journals, session-file records, and the memory graph index (with `sqlite-vec` for vectors). No external service is needed. Credentials and secret values inside it are encrypted at rest with a workspace key at `<workspace>/.runtime/encryption.key` (or `VIZIER_ENCRYPTION_KEY`/`VIZIER_ENCRYPTION_KEY_FILE`); back up the key separately from the database — see [Encryption at Rest](./encryption.md).

Memory **documents** are the one thing not stored in SQLite: they are markdown files under `<workspace>/agents/<agent_id>/memory/`. See [Memory](./memory.md).

| `type` | Status |
|--------|--------|
| `sqlite` | Default and only supported backend |
| `filesystem` | Legacy. Still parses so old configs load; on startup Vizier logs a warning and **migrates** everything from the old flat files into SQLite (one-time, idempotent). `--storage filesystem` / `VIZIER_STORAGE=filesystem` on the CLI is rejected outright. |

The `indexer` key that older docs mentioned at this level no longer exists — vector indexing is a per-agent setting (see [Tools & Embedding](./tools-embedding.md#indexer-indexer)).

### Startup migrations

`VizierDependencies::new` runs these once, guarded by markers in the `state` table:

1. Flat memory files → bundles (pre-bundle layouts)
2. `filesystem` backend → SQLite (if the config still says `filesystem`)
3. Seed users → `superadmin` system role
4. Provider credentials saved under pre-catalogue names → catalogue ids (`gemini` → `google`, …), once
5. Per-agent MCP / shell config backfill (from the old global config)
6. Default `CORE.md` for agents missing one

## Shell

Shell access is the **Shell addon**, added per agent from its Addons page. Each agent owns its own shell instance; there is no global shell. Without the addon, the agent has no `shell_exec` tool.

The tool runs `sh -c "<commands>"` (PowerShell on a Windows host) and returns stdout. Each call is bounded by the agent's `tools.timeout` (default `30m`).

### Local

Install the addon with:

```json
{
  "kind": "shell",
  "name": "Shell",
  "settings": {
    "path": "/home/me/project",
    "env": "RUST_LOG=debug\nTZ=UTC"
  }
}
```

| Setting | Required | Description |
|---------|----------|-------------|
| `path` | yes | Working directory for every command |
| `env` | no | Extra environment variables, one `KEY=value` per line (stored as a secret and never returned by the API) |

The command runs with the same OS user as the Vizier process — treat a local shell as fully trusted.

### Docker

Not offered yet. Docker shells are being rewritten; the Shell addon is local-only for now.

### Managing at runtime

- **WebUI**: agent → **Addons** → Shell → Manage
- **API**: `POST /api/v1/agents/{id}/integrations` to add it, or `PUT /api/v1/integrations/{id}` with new settings

Changing an addon's settings takes effect from the agent's next turn; it does not restart the agent.
