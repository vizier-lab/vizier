# 2.2 Main Configuration

`.vizier.yaml` is optional. When present, all keys sit under a single top-level `vizier:` key.

## Full reference

```yaml
vizier:
  # ── Model catalogue (read every start) ──
  catalogue:
    refresh: true                     # default true; false = never contact models.dev
    ttl: 24h                          # default 24h

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

### `catalogue`

| Field | Type | Default | Description |
|-------|------|---------|-------------|
| `refresh` | bool | `true` | Let a person's visit to the provider pickers or Settings → Providers check [models.dev](https://models.dev) for an updated catalogue in the background. `false` keeps the snapshot built into the binary. |
| `ttl` | duration | `24h` | How often such a check may happen. |
| `url` | string | `https://models.dev/api.json` | Where the catalogue is fetched from. |

Agents, tasks and dreams never trigger a check, and startup never does. See [Providers](./providers.md#the-model-catalogue).

> **No `providers` section.** Provider credentials are no longer set here: use **Settings → Providers** or the provider's environment variable (e.g. `ANTHROPIC_API_KEY`). A leftover `providers:` section is ignored, with a warning at startup naming the variable to set. See [Providers](./providers.md).

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
| `--catalogue-refresh <true\|false>` | `catalogue.refresh` |
| `--catalogue-ttl <DURATION>` | `catalogue.ttl` |

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
| `providers` | Settings → Providers, or the provider's env var |

## Minimal examples

**Local only** (Ollama on its default address needs no configuration)

```yaml
vizier:
  storage:
    type: sqlite
  channels:
    http:
      port: 9999
      jwt_secret: "change-me"
```

**Cloud provider, key from env** (run with `ANTHROPIC_API_KEY` set)

```yaml
vizier:
  storage:
    type: sqlite
  channels:
    http:
      port: 8080
      jwt_secret: "${VIZIER_JWT_SECRET}"
      jwt_expiry_hours: 168
  worker_threads: 8
```
