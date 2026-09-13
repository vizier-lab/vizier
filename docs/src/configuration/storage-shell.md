# 2.6 Storage & Shell

## Storage

```yaml
vizier:
  storage:
    type: sqlite
```

SQLite is the only runtime backend. The database lives at `<workspace>/.runtime/vizier.db` and holds agents, sessions, history, tasks, users/roles/API keys, providers, dream journals, session-file records, and the memory graph index (with `sqlite-vec` for vectors). No external service is needed.

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
4. YAML `providers` → providers table
5. Per-agent MCP / shell config backfill (from the old global config)
6. Default `CORE.md` for agents missing one

## Shell

Shell access is configured **per agent** under `tools.shell`. Each agent owns its own shell instance; there is no global shell. Setting `shell` to `null` (the default) removes the `shell_exec` tool from that agent entirely.

The tool runs `sh -c "<commands>"` (local) or `docker exec` (docker) and returns stdout. Each call is bounded by the agent's `tools.timeout` (default `30m`).

### Local

```json
{
  "tools": {
    "shell": {
      "environment": "local",
      "path": "/home/me/project",
      "env": { "RUST_LOG": "debug" }
    }
  }
}
```

| Field | Required | Description |
|-------|----------|-------------|
| `path` | yes | Working directory for every command |
| `env` | no | Extra environment variables |

The command runs with the same OS user as the Vizier process — treat `local` as fully trusted.

### Docker

```json
{
  "tools": {
    "shell": {
      "environment": "docker",
      "image": { "source": "pull", "name": "ubuntu:latest" },
      "container_name": "vizier-agent-1",
      "env": { "TZ": "UTC" }
    }
  }
}
```

Build from a Dockerfile instead of pulling:

```json
{
  "environment": "docker",
  "image": { "source": "dockerfile", "path": "./sandbox/Dockerfile", "name": "vizier-sandbox" },
  "container_name": "vizier-agent-1"
}
```

| Field | Default | Description |
|-------|---------|-------------|
| `image.source` | `pull` | `pull` (pull `name` from a registry) or `dockerfile` (build `path` and tag as `name`) |
| `container_name` | `vizier` | Name of the long-lived container |
| `env` | — | Environment for each `exec` |

Behavior:

- Connects to the local Docker daemon (`connect_with_local_defaults`, i.e. `/var/run/docker.sock` or `DOCKER_HOST`).
- If a container named `container_name` **already exists** it is reused as-is (image settings are ignored). Otherwise the image is pulled/built and a new TTY container is created and started.
- The container is kept running; every `shell_exec` is a `docker exec` inside it. Give each agent a distinct `container_name` if you want isolation between agents.

### Managing at runtime

- **WebUI**: agent Settings → Tools → Shell
- **API**: `PUT /api/v1/agents/{id}` with `tools.shell` (owner, or `all_agents:edit`)

Updating an agent restarts its process, which rebuilds the shell from the new config.
