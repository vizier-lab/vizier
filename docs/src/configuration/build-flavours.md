# Build Flavours

Vizier ships one opinionated default build: the release binaries, the Docker image and the desktop
app turn every optional piece on. If you build from source you can leave pieces out and get a
smaller, faster-to-compile binary. Every optional piece is a Cargo feature of `vizier-core`; a flag
never changes the API shape, it only adds or removes a capability.

## Flags

| Flag | Adds | Optional dependencies pulled in | Runtime capability name |
|---|---|---|---|
| `channel-discord` | The Discord reader and its tools | `twilight-gateway`, `twilight-http`, `twilight-model`, `twilight-util` | `channel/discord` |
| `channel-telegram` | The Telegram reader and its tools | `teloxide` | `channel/telegram` |
| `provider-media` | Speech-to-text and text-to-speech | `whisper-rs`, `any-tts`, `symphonia`, `symphonia-adapter-opus-rs`, `hound` | `provider/media` |
| `addon-docker` | The Docker shell backend | `bollard`, `indicatif` | `addon/docker` |
| `addon-documents` | The Office addon (PDF, Word, Excel) | `pdf-extract`, `docx-rs`, `calamine`, `rust_xlsxwriter`, `ironcalc` | `addon/documents` |
| `utoipa` | OpenAPI schemas on the API types | `utoipa` | `utoipa` |
| `telemetry` | OpenTelemetry trace export | `opentelemetry`, `tracing-opentelemetry` | `telemetry` |
| `full` | Every flag above | | |

The capability names are served to the WebUI by the Fleet work ([#173](https://github.com/vizier-lab/vizier/issues/173)); the table gives the names they will carry. Cargo does not allow `/` in a feature name, so the flag is `addon-docker` and the runtime
capability name is `addon/docker`: the first `-` after the group (`channel`, `addon`, `provider`,
`gateway`, `role`) becomes `/`. `utoipa` and `telemetry` belong to no group and keep their names.
Names are grouped as `channel-*`, `addon-*`, `provider-*`, `gateway-*`, `bus-nats` and `role-*`.

`default = []` for `vizier-core`, so an app that embeds the engine pays only for what it asks for.
The `vizier` binary and the desktop app depend on `vizier-core` with `full`.

### The binary's role flags

The `vizier` binary carries one flag per Fleet role ([#161](https://github.com/vizier-lab/vizier/issues/161)):

| Flag | Adds |
|---|---|
| `role-gateway-http` | The HTTP gateway (REST, WebSocket, the WebUI) |
| `role-gateway-grpc` | The gRPC gateway |
| `role-worker` | The workers that run agents |
| `role-channels` | The Discord and Telegram readers |
| `role-control` | The scheduler and the other housekeeping |
| `role-all` | The five above: **the default**, and today's single node |

They decide which roles a build *carries*. `vizier run --role <role>` refuses a role the binary does
not have, naming the flag to rebuild with. They do not remove any code yet: the role crates land with
[#166](https://github.com/vizier-lab/vizier/issues/166)–[#170](https://github.com/vizier-lab/vizier/issues/170),
and until then **`--role all` is the only role that runs** — every other one is refused with the
sub-issue that wires it in.

## What the default build includes

All of the flags above (`full`).

## Building a lean binary

`vizier` always enables `full` on the engine, so for a leaner server depend on `vizier-core`
directly and pick flags:

```sh
cargo build -p vizier-core --no-default-features --features channel-discord,addon-documents
```

The binary's roles are flags of their own, and each one must build alone (CI checks every one):

```sh
cargo build -p vizier --no-default-features --features role-worker
```

Check one flag on its own the way CI does:

```sh
cargo test -p vizier-core --no-default-features --features addon-docker
```

## Renamed flags

Builds from source that name a flag directly (`--features`, or `features = [...]` in a
`Cargo.toml`) must use the new names:

| Old | New |
|---|---|
| `discord` | `channel-discord` |
| `telegram` | `channel-telegram` |
| `media` | `provider-media` |
| `docker-shell` | `addon-docker` |
| `documents` | `addon-documents` |
| `utoipa`, `telemetry`, `full` | unchanged |
