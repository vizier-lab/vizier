# 2.6 Storage & Shell

## `storage`

Configure data persistence backend:

```yaml
storage:
  type: filesystem
  indexer: in_mem
```

### Storage Types

| Type | Description |
|------|-------------|
| `filesystem` | Store data in `.vizier/` directory (default) |
| `surreal` | Use SurrealDB for data storage |

### Indexer Types

| Type | Description |
|------|-------------|
| `in_mem` | In-memory indexer (default, fast, non-persistent) |
| `surreal` | SurrealDB-based indexer (persistent, slower) |

## `shell`

Configure the execution environment for shell commands:

```yaml
shell:
  environment: local
  path: "."
```

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
