# Contract: server edition compatibility

The `vizier` binary after the split must be indistinguishable from the release before it.
Each line is checked by a named gate (research R13, R15).

| Surface | Must stay identical | Gate |
|---|---|---|
| crates.io name / bin | `cargo install vizier`, `cargo binstall vizier`, binary named `vizier` | `package` job; release dry run |
| Release archives | `vizier-v{version}-{target}.tar.gz`, same four targets (`x86_64`/`aarch64` linux-gnu, `x86_64`/`aarch64` apple-darwin), same layout | release dry run |
| Docker image | `Dockerfile`, `docker-entrypoint.sh` env → flag mapping | docker build in release dry run |
| CLI | every subcommand, flag, help text, exit code (`run`, `run -d`, `shutdown`, `onboard`, `skill …`, `agent ps`) | `--help` snapshot diff |
| Config resolution | `-c` → `$VIZIER_CONFIG` → `./.vizier.yaml` → defaults; workspace `$VIZIER_DATA_DIR` → `$HOME/.vizier`; `${ENV}` expansion; overrides | harness + manual quickstart step |
| Env vars | `VIZIER_*`, provider key vars, `RUST_LOG`, `OTEL_*`, `VIZIER_ENCRYPTION_KEY(_FILE)` | manual quickstart step |
| HTTP/WebSocket API | every route, payload, status code, frame | `/openapi.json` byte diff + `scripts/dummyplug-vizier-e2e.mjs` |
| WebUI | same build served at `/`, SPA fallback, icons route | quickstart browser check |
| Storage | schema, data, document layout, migrations | run new binary on a copied previous-release workspace |
| Process semantics | any subsystem stop → exit 1; `vizier shutdown` → exit 0; command socket path | harness (core side) + quickstart (binary side) |
| Logging | default filter directives, compact fmt, OTLP export when `OTEL_EXPORTER_OTLP_ENDPOINT` set | manual quickstart step |

**One deliberate exception** (research R14): a second `vizier run` on a workspace that is
already in use now fails at startup with a clear message instead of starting and racing on
the same database.
