# 2.8 CLI

```
vizier <COMMAND>

Commands:
  run       Run vizier agents, servers and channels
  shutdown  Stop a running instance
  onboard   Interactive wizard that writes a seed .vizier.yaml
  storage   Move an embedded workspace to managed storage
  skill     Manage skills (install, list, uninstall, update)
  agent     Manage and inspect agents

`vizier --version` / `vizier <cmd> --help` work as usual.

## `vizier run`

Start the whole system: storage + migrations, every persisted agent, the scheduler (tasks + dream cycles), the HTTP channel (REST/WS/WebUI), per-agent Discord/Telegram bots, and the command socket.

```sh
vizier run                                  # ./.vizier.yaml, $VIZIER_CONFIG, or defaults
vizier run -c /etc/vizier/.vizier.yaml
vizier run -d                               # daemonize
vizier run --port 8080 --data-dir /srv/vizier --workers 8
```

| Flag | Description |
|------|-------------|
| `-c, --config <PATH>` | Path to `.vizier.yaml`. Optional; see resolution order below |
| `-d, --detached` | Daemonize. PID file `/tmp/vizier.pid`; stdout/stderr go to `<workspace>/.runtime/logs/<timestamp>.out|.err` |
| `--port <PORT>` | HTTP port (overrides config) |
| `--workspace <PATH>` | Workspace directory (overrides config and `VIZIER_DATA_DIR`) |
| `--data-dir <PATH>` | Alias of `--workspace`; wins if both are given |
| `--storage <STORAGE>` | Database: `sqlite` (default) or `postgres` (URL from `VIZIER_DATABASE_URL`); `filesystem` is rejected. See [Storage](./storage-shell.md#storage) |
| `--objects <OBJECTS>` | Files: `local` (default) or `s3` (`VIZIER_S3_*` variables) |
| `--workers <N>` | Tokio worker threads |
| `--ws-idle-timeout <SECS>` | WebSocket idle timeout |
| `--catalogue-refresh <true\|false>` | Allow the lazy models.dev catalogue check (default `true`) |
| `--catalogue-ttl <DURATION>` | How often that check may run (default `24h`) |
| `--role <ROLE[,ROLE]>` | Which server roles this process runs: `all` (default), or a comma list of `gateway-http`, `gateway-grpc`, `worker`, `channels`, `control`. `VIZIER_ROLE` does the same for containers. See [Roles](#roles) |

### Roles

Fleet runs the same binary as a cluster by splitting it into roles ([#159](https://github.com/vizier-lab/vizier/issues/159)).

| Role | Does | Singleton? |
|------|------|------------|
| `gateway-http` | REST, WebSocket, the WebUI, authentication and authorization | no, scale out |
| `gateway-grpc` | The same operations over gRPC, for other software | no, scale out |
| `worker` | Runs agents (supervisors and instances) | no; one owner worker per agent |
| `channels` | The Discord and Telegram readers | one reader per integration |
| `control` | The scheduler (cron, one-shot, dreams), retention purge, migrations, catalogue refresh | one leader plus standbys |
| `all` | Every role in one process (**default**) | |

`--role all` is exactly the single-node server described on this page. **Every other role is refused
for now**, with a message naming the sub-issue that wires it in: `control`
([#166](https://github.com/vizier-lab/vizier/issues/166)), `channels`
([#167](https://github.com/vizier-lab/vizier/issues/167)), `worker`
([#168](https://github.com/vizier-lab/vizier/issues/168)), `gateway-http`
([#169](https://github.com/vizier-lab/vizier/issues/169)) and `gateway-grpc`
([#170](https://github.com/vizier-lab/vizier/issues/170)). A role the binary was not built with is
refused with the flag to rebuild with.

```sh
vizier run --role all            # one node doing everything: today's server
vizier run --role worker,control # refused until #166 and #168 land
```

The process's roles show in `vizier agent ps` (`Roles  all`) and in `VizierRuntime::health`.

**Config resolution:** `-c` → `$VIZIER_CONFIG` → `./.vizier.yaml` → built-in defaults. With a file, the workspace is `<config dir>/.vizier/`; without one it's `$VIZIER_DATA_DIR` or `$HOME/.vizier`. See [Overview](./index.md#config-less-mode) — note `VIZIER_JWT_SECRET` must be set in config-less mode.

**Logging:** `tracing` with `RUST_LOG` (e.g. `RUST_LOG=vizier=debug`). Noisy dependencies (twilight, hyper, reqwest, bollard, rmcp, sqlite…) are quieted by default unless you name them explicitly.

**Tracing:** set `OTEL_EXPORTER_OTLP_ENDPOINT` to export OpenTelemetry traces and metrics. See [Observability](./observability.md).

## `vizier shutdown`

```sh
vizier shutdown [-c <PATH>]
```

Loads the same config to find the workspace, then sends `Exit` over the Unix socket `<workspace>/.runtime/.vizier.sock`. Errors if no instance is running there. Works for both foreground and detached instances.

## `vizier onboard`

```sh
vizier onboard [-p <PATH>]
```

| Flag | Description |
|------|-------------|
| `-p, --path <PATH>` | Workspace directory (prompted if omitted; `~` is expanded). `.vizier.yaml` is written there. |

Prompts for: HTTP port, JWT secret (random default), worker threads, WebSocket idle timeout. Shows a preview and asks to confirm before writing. Storage is always SQLite; providers are connected in the WebUI (Settings › Providers) or through their environment variables.

It does **not** create users, providers or agents — do that in the WebUI after `vizier run`. Embedding is configured per agent, not here.

## `vizier storage`

```sh
vizier storage migrate --workspace <SOURCE> [-c <DEST_CONFIG>] \
  [--storage postgres] [--objects local|s3] [--reset-destination]
```

Copies a stopped SQLite/local workspace into PostgreSQL and the configured file backend. The PostgreSQL URL and S3 credentials come from environment variables, never arguments. Progress and errors go to stderr; success prints one JSON report to stdout. Exit `0` means complete, `1` means failed but resumable, and `2` means refused without changing the destination. A completed source fingerprint is refused unless `--reset-destination` is passed. See [Moving an existing workspace](./storage-shell.md#moving-an-existing-workspace).

## `vizier agent`

```sh
vizier agent [-c <PATH>] ps
```

| Subcommand | Description |
|------------|-------------|
| `ps` | Lists every agent process with `online`/`offline` status, via the command socket of the running instance |

There is no `agent create`/`delete` — agents are managed in the WebUI or via `/api/v1/agents`.

## `vizier skill`

Manages skill packages on disk. Uses the config resolution above to find the workspace (`<workspace>/skills/` or `<workspace>/agents/<id>/skills/`). Requires `git` for registry and git sources. See [Skills](./skills.md).

```sh
vizier skill install <SOURCE> [-a <AGENT_ID>]
vizier skill list
vizier skill uninstall <SLUG> [-a <AGENT_ID>]
vizier skill update <SLUG>
```

| Command | Description |
|---------|-------------|
| `install <SOURCE>` | `SOURCE` detection: `http(s)://…` or `*.git` → git clone; `owner/repo` → `https://github.com/owner/repo.git`; `./path` or `/path` → local copy; bare `slug` → sparse-checkout of `skills/<slug>` from the registry `https://github.com/vizier-lab/vizier.git`. `-a` installs into an agent's private skills dir instead of the global one. |
| `list` | Global skills: name, description, version |
| `uninstall <SLUG>` | Remove the skill directory (`-a` for an agent-scoped skill) |
| `update <SLUG>` | Re-install from the registry. Only works for skills whose `.meta.json` says `source: registry` |

## Docker

The image (`blinfoldking/vizier`, also `ghcr.io/vizier-lab/vizier`) runs `vizier run` config-less by default. `docker-entrypoint.sh` translates env vars to flags and `exec`s the binary so signals propagate.

| Env var | Maps to | Default |
|---------|---------|---------|
| `VIZIER_CONFIG` | `-c` | unset (config-less) |
| `VIZIER_DATA_DIR` / `VIZIER_WORKSPACE` | `--data-dir` | `$HOME/.vizier` inside the container — mount a volume |
| `VIZIER_PORT` | `--port` | `9999` |
| `VIZIER_STORAGE` | `--storage` | `sqlite` |
| `VIZIER_OBJECTS` | `--objects` | `local` |
| `VIZIER_ROLE` | `--role` | `all` |
| `VIZIER_WORKERS` | `--workers` | `4` |
| `VIZIER_WS_IDLE_TIMEOUT` | `--ws-idle-timeout` | `300` |
| `VIZIER_CATALOGUE_REFRESH` | `--catalogue-refresh` | `true` |
| `VIZIER_CATALOGUE_TTL` | `--catalogue-ttl` | `24h` |
| `VIZIER_JWT_SECRET` | env read by the default config | `vizier-default-secret-change-me` — **change it** |
| `VIZIER_EXTRA_ARGS` | appended verbatim | unset |
| `RUST_LOG` | logging filter | unset |
| `OTEL_EXPORTER_OTLP_ENDPOINT` (and other `OTEL_*`) | read directly, see [Observability](./observability.md) | unset (export off) |

Any extra arguments after `run` are appended last (they win). Any other first argument (`shutdown`, `agent ps`, `skill …`) is passed straight through with no env translation.

```sh
# config-less, persisted
docker run -p 9999:9999 -v vizier-data:/data \
  -e VIZIER_DATA_DIR=/data -e VIZIER_JWT_SECRET=$(openssl rand -hex 32) \
  blinfoldking/vizier

# with a config file
docker run -p 9999:9999 -v $PWD/.vizier.yaml:/cfg.yaml -e VIZIER_CONFIG=/cfg.yaml blinfoldking/vizier

# passthrough
docker exec vizier vizier agent ps
```

A sample `docker-compose.yaml` ships in the repository.
