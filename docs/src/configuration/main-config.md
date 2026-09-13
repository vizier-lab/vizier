# 2.2 Main Configuration

`.vizier.yaml` is optional. When present, all keys sit under a single top-level `vizier:` key.

## Full reference

```yaml
vizier:
  # ── Providers (seed values, migrated into storage on first run) ──
  providers:
    ollama:
      base_url: "http://localhost:11434"
    llama_cpp:
      base_url: "http://localhost:8080"
    openai:
      api_key: "${OPENAI_API_KEY}"
    anthropic:
      api_key: "${ANTHROPIC_API_KEY}"
    openrouter:
      api_key: "${OPENROUTER_API_KEY}"
    # …any of the variants listed in Providers

  # ── Storage (read every start) ──
  storage:
    type: sqlite            # the only supported backend

  # ── Channels (read every start) ──
  channels:
    http:
      port: 9999
      jwt_secret: "${VIZIER_JWT_SECRET}"
      jwt_expiry_hours: 720           # default 720 (30 days)
      ws_idle_timeout_secs: 300       # default 300 (5 minutes)

  # ── Runtime ──
  worker_threads: 4                   # tokio worker threads, default 4
```

### `providers`

A map of provider variant → credentials. Only the providers you list are seeded; each variant has its own fields (`api_key`, `base_url`, `endpoint`, `access_token`/`account_id`). See [Providers](./providers.md) for the full table.

Providers are migrated into the providers table on first run. After that, edit them in **Settings → Providers** or via `PUT /api/v1/providers/{variant}` — changes to the YAML for an already-migrated variant are not re-applied.

### `storage`

| Field | Values | Default |
|-------|--------|---------|
| `type` | `sqlite` | `sqlite` |

`filesystem` still *parses* (so old files keep loading) but is no longer a runtime backend: on startup the data is migrated into SQLite with a warning. See [Storage & Shell](./storage-shell.md).

### `channels.http`

| Field | Type | Default | Description |
|-------|------|---------|-------------|
| `port` | number | `9999` | Port the REST API, WebSocket, Swagger UI, and WebUI listen on (binds `0.0.0.0`) |
| `jwt_secret` | string | `"${VIZIER_JWT_SECRET}"` | HMAC secret for signing JWTs. **Set a strong value.** |
| `jwt_expiry_hours` | number | `720` | JWT lifetime |
| `ws_idle_timeout_secs` | number | `300` | A WebSocket chat connection with no traffic for this long is closed |

`channels.http` is an `Option` — omitting it disables the HTTP channel entirely (no WebUI, no API). Passing `--port` or `--ws-idle-timeout` on the CLI re-enables it with defaults.

> Discord and Telegram are **not** configured here. Each agent carries its own `discord_token` / `telegram_token`; see [Channels](./channels.md).

### `worker_threads`

Number of Tokio worker threads for the whole process. Override with `--workers`.

## Environment variable expansion

Every string in the file goes through `${VAR}` expansion before parsing, so secrets can live in the environment:

```yaml
vizier:
  providers:
    openrouter:
      api_key: "${OPENROUTER_API_KEY}"
  channels:
    http:
      jwt_secret: "${VIZIER_JWT_SECRET}"
```

An unset variable is an error at load time.

## Overrides

CLI flags win over the file (`VizierConfig::apply_overrides`):

| Flag | Overrides |
|------|-----------|
| `--port <PORT>` | `channels.http.port` |
| `--workspace <PATH>` / `--data-dir <PATH>` | workspace directory (`--data-dir` wins if both given) |
| `--storage sqlite` | `storage.type` |
| `--workers <N>` | `worker_threads` |
| `--ws-idle-timeout <SECS>` | `channels.http.ws_idle_timeout_secs` |

## Ignored legacy keys

Older versions accepted `embedding`, `shell`, `tools.brave_search`, `tools.mcp_servers`, `channels.discord`, and `channels.telegram` at this level. The parser ignores unknown keys, so an old file still loads — but those values have **no effect**. Their replacements:

| Legacy key | Now |
|------------|-----|
| `embedding` | per-agent `embedding` (WebUI agent settings) |
| `shell` | per-agent `tools.shell` |
| `tools.mcp_servers` | per-agent `tools.mcp_servers` |
| `tools.brave_search` | per-agent `tools.brave_search_settings` |
| `channels.discord.<agent>.token` | per-agent `discord_token` |
| `channels.telegram.<agent>.token` | per-agent `telegram_token` |

## Minimal examples

**Local only**

```yaml
vizier:
  providers:
    ollama:
      base_url: "http://localhost:11434"
  storage:
    type: sqlite
  channels:
    http:
      port: 9999
      jwt_secret: "change-me"
```

**Cloud provider, secrets from env**

```yaml
vizier:
  providers:
    anthropic:
      api_key: "${ANTHROPIC_API_KEY}"
  storage:
    type: sqlite
  channels:
    http:
      port: 8080
      jwt_secret: "${VIZIER_JWT_SECRET}"
      jwt_expiry_hours: 168
  worker_threads: 8
```
