# Quickstart: verifying the core extraction

Run these on every phase PR (phases 0–4). Steps marked **[baseline]** are run once on `master`
before phase 1 and saved under `target/037-baseline/` (gitignored) for later diffs. All agent
steps use the offline `dummyplug` provider: no keys, no network.

## 0. Setup

```sh
just checkout 037-core-extraction   # workspace + every submodule on the same branch
just install                        # submodules, npm i, cargo fetch
cd webui && npm run build && cd ..
```

## 1. Baselines [baseline]

```sh
mkdir -p target/037-baseline
cargo build --release
B=target/release/vizier
for c in "" run shutdown onboard skill agent "agent ps" "skill install"; do
  $B $c --help > "target/037-baseline/help-${c// /-}.txt" 2>&1
done
VIZIER_DATA_DIR=$(mktemp -d) $B run --port 19999 & sleep 5
curl -s localhost:19999/openapi.json > target/037-baseline/openapi.json
$B shutdown; wait
cargo metadata --format-version 1 --locked \
  | jq -r '.packages[] | "\(.name) \(.version) \(.source)"' | sort > target/037-baseline/deps.txt
```

Also keep a copy of a workspace created by the *previous release* binary with one dummyplug
agent, one conversation, one memory document and one task (`target/037-baseline/workspace/`).

## 2. In-process harness (core, no binary)

```sh
cargo test -p vizier-core --no-default-features --test dummyplug_e2e   # before phase 3: cargo test dummyplug_e2e
cargo test -p vizier-core --all-features --test dummyplug_e2e
```

Expected, in order (dummyplug protocol, `specs/008-dummyplug-provider/contracts/dummyplug-protocol.md`):

| Send | Expect |
|---|---|
| `tools` | a reply listing the sandbox tools (`think`, `execute_python`, `list_tool_functions`, `describe_tool_function`) |
| `execute_python` | a sample JSON request for that tool |
| the sample JSON request | a reply containing the tool's result; history holds the call and its result |
| `context` | the per-request context block the agent received |
| (forced subsystem stop) | `RuntimeExit::SubsystemStopped { .. }`, the test process still alive, `health()` shows the subsystem `Stopped` |
| second `VizierRuntime::start` on the same workspace while the first runs | `Err` naming the workspace as in use |

## 3. Headless example (US2)

```sh
cd "$(mktemp -d)"
cargo run --manifest-path ~/project/vizier/crates/vizier-core/Cargo.toml \
  --no-default-features --example headless -- --workspace ./ws
```

Expect: printed `Started` event, one dummyplug reply, session events for the turn, a clean
shutdown, exit 0. `cargo tree -p vizier-core -e normal --no-default-features` lists no
`axum`, `clap`, `inquire`, `termimad`, `daemonize`, `dotenvy`, `tower-http`.

## 4. Server unchanged (US1)

```sh
cargo build --release                      # default-members = crates/vizier
B=target/release/vizier
for c in "" run shutdown onboard skill agent "agent ps" "skill install"; do
  diff <($B $c --help 2>&1) "target/037-baseline/help-${c// /-}.txt"
done                                        # expect: no output
cp -r target/037-baseline/workspace /tmp/037-ws
VIZIER_DATA_DIR=/tmp/037-ws $B run --port 19999 & sleep 5
diff <(curl -s localhost:19999/openapi.json) target/037-baseline/openapi.json   # expect: no output
```

With that server still running:

1. `node scripts/dummyplug-vizier-e2e.mjs` against `http://localhost:19999` — passes.
2. Open `http://localhost:19999/` — WebUI loads, log in, the existing agent, conversation,
   memory document and task are all there; reload a deep link (`/agents/<id>/chat/...`) — SPA
   fallback still serves it; an integration icon loads on the Integrations page.
3. In the dummyplug agent's chat send `tools`, `execute_python`, the JSON request, `context` —
   same replies as step 2.
4. Start a **second** `VIZIER_DATA_DIR=/tmp/037-ws $B run --port 19998` — it refuses with
   "already in use" and exits 1 (the one deliberate change, research R14).
5. `$B agent ps` lists the agent; `$B shutdown` stops the server; `echo $?` of the `run` job is 0.
6. Bind a port with `nc -l 19997`, then `VIZIER_DATA_DIR=/tmp/037-ws $B run --port 19997` — the
   server logs that the HTTP subsystem stopped and exits 1 (#51 preserved).
7. `OTEL_EXPORTER_OTLP_ENDPOINT=http://localhost:4318 just run` with `just jaeger` up — traces
   for one dummyplug turn appear in Jaeger (`invoke_agent`, `chat`, `execute_tool` spans).
8. `cargo run -- run --config dev.vizier.yaml` (no `-p`) still starts the server.

## 5. Dependencies unchanged (FR-019)

```sh
cargo metadata --format-version 1 --locked \
  | jq -r '.packages[] | "\(.name) \(.version) \(.source)"' | sort \
  | grep -vE '^vizier(-core|-http|-derive)? ' > /tmp/deps.txt
grep -vE '^vizier(-derive)? ' target/037-baseline/deps.txt | diff - /tmp/deps.txt   # expect: no output
grep -E 'fastembed = "=4.9.1"|monty = "=1.0.0"|monty-types = "=1.0.0"' Cargo.toml   # three lines
```

## 6. Packaging (phase 4)

```sh
mkdir -p crates/vizier-http/webui-dist && cp -r webui/build/client/. crates/vizier-http/webui-dist/
cargo package --workspace --allow-dirty         # all four build from their .crate files
ls -la target/package/*.crate                   # each < 10 MB
DOCS_RS=1 cargo doc -p vizier-core --no-deps --features discord,telegram,documents,utoipa
cargo public-api -p vizier-core                 # only the stable tier (no __adapter items)
```

## 7. Cross targets and release dry run (required on the phase 3 and phase 4 PRs)

```sh
cross build --release --target x86_64-unknown-linux-musl -p vizier
python3 -I scripts/catalogue_update.py --keep-on-failure     # phase 3: writes crates/vizier-core/assets/catalogue
git status --short crates/vizier-core/assets/catalogue       # changes only there, nothing under ./assets
gh workflow run release-and-publish.yml --ref 037-core-extraction \
  -f version=<next> -f dry_run=true
```

The dry run must show (research R18):

- "Update Cargo.toml versions" bumps `[workspace.package] version` and the three internal
  dependency entries in the root `Cargo.toml`, and its verifying `grep`s pass;
- "Commit version bump" stages `Cargo.toml`, `Cargo.lock`, `CHANGELOG.md` and
  `crates/vizier-core/assets/catalogue` only;
- all four targets build and archive as `vizier-v<next>-<target>.tar.gz` with the `vizier`
  binary at the same path inside;
- the Docker image builds;
- `publish.yml`: the version check passes, the package listing contains
  `assets/catalogue/api.json` (core) and `webui-dist/index.html` (http), every `.crate` is
  under 10 MB, and `cargo publish --workspace --dry-run` lists `vizier-derive`, `vizier-core`,
  `vizier-http`, `vizier` (phase 4);
- the release notes no longer mention a `python` feature.
