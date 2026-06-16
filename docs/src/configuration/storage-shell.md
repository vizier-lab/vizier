# 2.6 Storage & Shell

## `storage`

Configure data persistence backend:

```yaml
storage:
  type: filesystem
  indexer: in_mem
```

> **Note:** Storage config is **not** migrated to storage — it remains file-based and is read once at startup. You cannot change the storage backend after first run without resetting the workspace.

### Storage Types

| Type | Description |
|------|-------------|
| `filesystem` | Store data in `.vizier/` directory (default) |
| `sqlite` | Use SQLite for data storage |

### Indexer Types

| Type | Description |
|------|-------------|
| `in_mem` | In-memory indexer (default, fast, non-persistent) |
| `sqlite` | SQLite-based indexer (persistent, vector search) |

## `shell`

Configure the execution environment for shell commands:

```yaml
shell:
  environment: local
  path: "."
```

> **Note:** Shell config is auto-migrated to storage on first run. After migration, shell settings are managed via WebUI (Settings > Shell) or HTTP API (`PUT /api/v1/global-config/shell`). Changes hot-reload without restart.

### Local Environment

```yaml
shell:
  environment: local
  path: "/path/to/working/dir"  # Working directory for shell commands
  env:                           # Optional: environment variables
    KEY: "value"
```

### Docker Environment

```yaml
shell:
  environment: docker
  image:
    source: pull              # Use "pull" or "dockerfile"
    name: "ubuntu:latest"     # Image name (for pull) or "my-image"
  container_name: "vizier"    # Container name
```

For `dockerfile` source:

```yaml
shell:
  environment: docker
  image:
    source: dockerfile
    path: "./Dockerfile"      # Path to Dockerfile
    name: "my-custom-image"   # Image name to build
  container_name: "vizier"
```

## Managing Shell Config at Runtime

After the initial seed config is migrated, shell settings are managed via:

- **WebUI**: Settings > Shell
- **API**: `PUT /api/v1/global-config/shell`

Shell changes are hot-reloaded — the shell instance is atomically swapped without restarting the agent.
