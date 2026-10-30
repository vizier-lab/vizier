---

description: "Task list for extracting vizier-core and splitting the workspace (issue #102, phases 0–4)"
---

# Tasks: Extract vizier-core and split the workspace

**Input**: Design documents from `specs/037-core-extraction/`

**Prerequisites**: plan.md, spec.md, research.md (R1–R18), data-model.md, contracts/, quickstart.md

**Tests**: Requested by the spec — FR-014 (in-process dummyplug harness), FR-015 (CI matrix), SC-001/SC-006 (before/after baselines), SC-008 (runtime never exits the host). Test tasks below are those; no others are added.

**Organization**: One phase per user story. The stories are **not** ordered by priority here because they have hard dependencies (plan "Phase sequencing"): the inversions (US5, P2) must be fixed before the engine can stand alone (US2, P1), which must exist before the split that US1 (P1) guards, and layering enforcement (US3) and publishing (US4) only make sense after the split. US1's "server unchanged" gates (quickstart §4) run at the end of **every** phase, not only in its own.

## Format: `[ID] [P?] [Story] Description`

- **[P]**: Can run in parallel (different files, no dependencies on incomplete tasks)
- **[Story]**: US1–US5 from spec.md
- Paths are `src/…` until T041 (the split); after it they are `crates/<crate>/…`

## PR map (plan "Phase sequencing")

| PR | Tasks | Title |
|---|---|---|
| 1 | T001–T010 | `chore: add CI matrix and dummyplug harness` |
| 2 | T011–T017 | `refactor: build vizier as a library with a thin main` |
| 3 | T018–T024 | `refactor: own identity, refresh status and rate limiter in the engine` |
| 4 | T025–T034 | `refactor: add VizierRuntime and move process concerns out of the engine` |
| 5 | T035–T040 | `refactor: gate optional engine features` |
| 6 | T041–T058 | `refactor: split the workspace into vizier-core, vizier-http and vizier` |
| 7 | T059–T074 | `chore: make vizier-core and vizier-http publishable` (+ US3 enforcement, polish) |

---

## Phase 1: Setup (Guardrails — phase 0 of the issue)

**Purpose**: Baselines and CI that every later phase is measured against. Nothing here changes product code.

- [X] T001 Run `just checkout 037-core-extraction` so the workspace and the `design`, `webui`, `landing`, `dummyplug` submodules are on the same branch (constitution v1.3.0); confirm with `just status`
- [X] T002 On `master` (`0eee255` or the current tip), build the release binary and record the baselines from quickstart §1 into `target/037-baseline/` (gitignored): every `--help` output (`help-*.txt`), `GET /openapi.json` (`openapi.json`), and the resolved dependency triples (`deps.txt` from `cargo metadata --format-version 1 --locked`); add `target/037-baseline/` to nothing — `target/` is already ignored
- [X] T003 Create a previous-release workspace for upgrade testing: with the current release binary and `VIZIER_DATA_DIR=target/037-baseline/workspace`, create one agent on the `dummyplug` provider, one WebUI conversation, one memory document and one scheduled task (quickstart §1)
- [X] T004 Add `scripts/037-baseline-check.sh` that rebuilds the binary and diffs `--help` outputs, `/openapi.json` and the dependency triples against `target/037-baseline/` (the quickstart §4 and §5 commands, with the four workspace crate names filtered out of the dependency diff); exits non-zero on any difference
- [X] T005 [P] Create `.github/workflows/ci.yml` (on `pull_request` and push to `master`) with jobs `server` (checkout with submodules, Node 22, `(cd design && npm ci)`, `(cd webui && npm ci && npm run build)`, `cargo clippy --all-targets -- -D warnings`, `cargo test`, `cargo build --release`) and `webui` (`cd webui && npm ci && npm run typecheck && npx vitest run`), per research R15
- [X] T006 [P] Add to `.github/workflows/ci.yml` jobs `core-minimal` and `core-full` that, until the split, run `cargo check --all-targets` and `cargo test` on the single crate (with `--no-default-features` and `--all-features` respectively); T040 and T053 retarget them to `-p vizier-core`
- [X] T007 [P] Add a `deps` job to `.github/workflows/ci.yml` that fetches `origin/master`, computes `cargo metadata --format-version 1 --locked` `(name, version, source)` triples on both, filters out `vizier`, `vizier-core`, `vizier-http`, `vizier-derive`, and fails on any version or source difference; entries present on only one side are printed for review, not failed (research R12)
- [X] T008 Write the in-process harness `src/e2e_tests.rs` (`#[cfg(test)] mod e2e_tests;` in `src/main.rs`): temp workspace via `tempfile`, build `VizierDependencies` from a `VizierConfig` with that workspace, start `VizierAgents` + the transport command handler on the test runtime, create a `dummyplug` agent with `AgentCommand::Create`, then send `Chat` requests through `VizierTransport::send_request` and assert, per `specs/008-dummyplug-provider/contracts/dummyplug-protocol.md`: `tools` lists `think`, `execute_python`, `list_tool_functions`, `describe_tool_function`; `execute_python` returns a sample JSON request; that JSON runs the tool and history holds the call and its result; `context` echoes the context block. Reuse helpers from `src/tests_support.rs`
- [X] T009 Run T008 locally (`cargo test e2e_tests`) and in CI (`core-minimal`, `core-full`, `server`); fix flakiness (timeouts, ports) before merging — this harness is the regression gate for every later phase
- [X] T010 Confirm `vizier-core` and `vizier-http` are still unclaimed on crates.io (`cargo search vizier-core`, `cargo search vizier-http`, and the crates.io page for each); record the result in the PR 1 description

**Checkpoint**: CI green on `master` + this branch; baselines recorded; harness passes.

---

## Phase 2: Foundational (Library-ify in place — phase 1 of the issue)

**Purpose**: The crate becomes a library with a thin binary, still one package. Blocks every user story.

**⚠️ CRITICAL**: No user story work can begin until this phase is complete.

- [X] T011 Create `src/lib.rs` holding every `mod` declaration now in `src/main.rs` (keep `#![allow(unused)]`, `#[macro_use] extern crate tracing;` and `pub type Result<T>`), add `[lib] path = "src/lib.rs"` and `[[bin]] name = "vizier" path = "src/main.rs"` to `Cargo.toml`
- [X] T012 Reduce `src/main.rs` to the bootstrap only (dotenvy, filter, telemetry, subscriber, rustls install, `vizier::cli::start()`, exit code), calling into the library as `vizier::…`; move `default_filter` with it unchanged
- [X] T013 Make `cli` and `command` `pub` in `src/lib.rs` (they move to the binary crate in T045); everything the binary calls is reachable as `vizier::…`
- [X] T014 Visibility pass over `src/lib.rs` per research R3: the stable tier (`config`, `error`, `schema` request/response types, `transport::VizierTransport`) is `pub`; every module that `src/channels/http/` or `src/cli/` imports is `#[doc(hidden)] pub mod`; the rest becomes `pub(crate) mod`. Add the crate-level doc comment explaining the two tiers and that the hidden tier is replaced by the service layer (phase 5 of #102)
- [X] T015 Move the harness from `src/e2e_tests.rs` to `tests/dummyplug_e2e.rs` as an integration test against the library's public paths (only `#[doc(hidden)]` items it needs: `dependencies`, `agents`); remove the `mod e2e_tests` line
- [X] T016 Fix every warning/error `cargo clippy --all-targets` reports from the visibility change (dead-code now visible as `pub(crate)`, `unreachable_pub`); do not delete migration or legacy code (#82 rule in CLAUDE.md)
- [X] T017 Run `scripts/037-baseline-check.sh` and `cargo test`; both pass with zero diff (US1 gate)

**Checkpoint**: Library + thin binary; behaviour identical; harness is an integration test.

---

## Phase 3: User Story 5 — The engine does not depend on the web layer's identity types (Priority: P2)

**Goal**: Remove every engine → HTTP import (spec FR-006, FR-007, FR-008).

**Independent Test**: `grep -rn "channels::http" src --include='*.rs' | grep -v '^src/channels/http'` prints nothing, and `grep -rln axum src --include='*.rs' | grep -v '^src/channels/http'` prints only test files; client tools (`scripts/client-tools-e2e.mjs`) and catalogue refresh (`GET /api/v1/catalogue/status`) behave as before.

- [X] T018 [P] [US5] Create `src/auth.rs` with `Identity { user_id, username, role: Role, permissions: Vec<String>, method: AuthMethod }` and `#[non_exhaustive] pub enum AuthMethod { Jwt, ApiKey }` (moved from `src/channels/http/auth/mod.rs:114-126`, `AuthenticatedUser` renamed to `Identity`, field `auth_method` → `method`), re-export `storage::user::Role`; register `pub mod auth` in `src/lib.rs` (stable tier)
- [X] T019 [US5] Update `src/channels/http/auth/mod.rs` and `src/channels/http/auth/middleware.rs` to produce `crate::auth::Identity` in request extensions; replace every `AuthenticatedUser` use under `src/channels/http/` with `Identity` (`Extension<Identity>`); keep `AuthService`, `Claims`, JWT and argon2 in `src/channels/http/auth/` (research R4)
- [X] T020 [P] [US5] Change `src/access.rs:17` to `impl<'a> From<&'a crate::auth::Identity> for Caller<'a>`; keep its tests passing
- [X] T021 [P] [US5] Change `src/schema/revision.rs:11-14` to import `crate::auth::{AuthMethod, Identity}` and build `RevisionOrigin` from `&Identity`; persisted columns and JSON unchanged (check the revision tests and the `/history` OpenAPI schema stay identical)
- [X] T022 [P] [US5] Move `RefreshStatus` from `src/channels/http/api/v1/catalogue.rs:63-70` to `src/catalogue/mod.rs`; update the imports in `src/catalogue/media.rs:19`, `src/catalogue/refresh.rs:19` and `src/channels/http/api/v1/catalogue.rs`; keep the `utoipa::ToSchema` derive (gated in T038)
- [X] T023 [P] [US5] Move `src/channels/http/rate_limit.rs` to `src/utils/rate_limit.rs` (`git mv`); update `src/dependencies.rs:75,282` and every `src/channels/http/` user
- [X] T024 [US5] Replace `axum::extract::ws::Message` in `src/agents/client/connection.rs:17,32,38,87,135` with `serde_json::Value` (`mpsc::Sender<serde_json::Value>`, send the frame value itself); convert to `Message::Text(value.to_string().into())` in the socket writer task created next to `ClientConnection::new` in `src/channels/http/api/v1/agents/channel.rs:1022`; fix the test helper in `src/agents/client/mod.rs:593`; run `node scripts/client-tools-e2e.mjs` against a dummyplug agent (research R5)

**Checkpoint**: Independent test passes; `scripts/037-baseline-check.sh` zero diff; harness green.

---

## Phase 4: User Story 2 — A third party can build their own interface on the engine (Priority: P1) 🎯 MVP

**Goal**: The engine runs inside any host with an explicit workspace, reports failures instead of exiting, and has optional feature groups (FR-002, FR-003, FR-005, FR-009, FR-012, FR-013).

**Independent Test**: the harness's runtime tests (T033) pass with `--no-default-features`; `examples/headless.rs` (T034) runs a dummyplug turn and exits 0; the server still passes `scripts/037-baseline-check.sh`.

### Runtime and process concerns (PR 4)

- [ ] T025 [US2] Create `src/runtime.rs` with `VizierRuntime`, `RuntimeOptions { config }`, `#[non_exhaustive] RuntimeEvent { Started, SubsystemStopped { name, outcome }, ExitRequested }`, `RuntimeExit { Requested, SubsystemStopped { name, outcome } }`, `HealthReport` and `SubsystemState` exactly as in data-model.md; move the `Subsystems` JoinSet helper from `src/cli/run.rs:155-185` into it
- [ ] T026 [US2] Implement `VizierRuntime::start` in `src/runtime.rs`: require a non-empty `config.workspace`; take the workspace lock (T027); `VizierDependencies::new`; `VizierAgents::new` then `deps.background_jobs.recover()` (same order as `src/cli/run.rs:121-127`); spawn agent manager, scheduler, channel readers (T029) and the transport command handler on the *current* tokio runtime; broadcast `Started`. Implement `transport()`, `#[doc(hidden)] deps()`, `events()`, `session_events()`, `health()` (subsystem states + `transport.agent_health()`), `wait()` (select on `exit_signal` → `Requested`, first stopped subsystem → `SubsystemStopped`, each also broadcast), `shutdown()` and `Drop` (abort subsystems, release lock). No `process::exit`, no runtime builder
- [ ] T027 [US2] Implement the workspace lock in `src/runtime.rs`: create `{workspace}/.runtime/`, open `vizier.lock`, `File::try_lock()`; on `TryLockError::WouldBlock` return `VizierError("workspace {path} is already in use by another vizier process")`; hold the `File` in `VizierRuntime` (research R14)
- [ ] T028 [US2] Split config resolution: in `src/config/mod.rs` replace `VizierConfig::load` with `VizierConfig::from_path(&Path)` (file read + `${ENV}` expansion, unchanged) and `VizierConfig::with_workspace(PathBuf)` (built-in defaults); move `default_config_candidate` and `resolve_default_workspace` (`src/config/mod.rs:250-268`) and the `$VIZIER_CONFIG` lookup into a new `src/cli/config_resolve.rs` that reproduces today's order exactly (`-c` → `$VIZIER_CONFIG` → `./.vizier.yaml` → defaults; workspace `$VIZIER_DATA_DIR` → `$HOME/.vizier`, created if missing); update every caller in `src/cli/`
- [ ] T029 [US2] Turn `VizierChannels` in `src/channels/mod.rs` into `ChannelReaders` (reconcile + watch, then `futures::future::pending()` — never returns while healthy); remove its `HTTPChannel` start and the `http_config` field
- [ ] T030 [US2] Add `pub async fn serve(runtime: &VizierRuntime, config: HTTPChannelConfig) -> crate::Result<()>` and `pub fn router(..) -> axum::Router` to `src/channels/http/mod.rs`, built from `runtime.deps()`; `serve` returns only if the server stops (research R8)
- [ ] T031 [US2] Rewrite `run_server` in `src/cli/run.rs` as `src/cli/server.rs`: build the tokio runtime with `config.worker_threads`, `VizierRuntime::start`, spawn `channels::http::serve` and `VizierCommandServer::run` as binary-owned subsystems, then select over `runtime.wait()` and those two: `Requested` → `exit(0)`; any stop → `tracing::error!("{name} stopped ({outcome}); exiting so vizier can be restarted")` + `exit(1)` (log text unchanged, #51); keep the command-socket bind-failure tolerance
- [ ] T032 [US2] Move `src/utils/logo.rs` to `src/cli/logo.rs` and `Telemetry::init` (OTLP exporter setup, `opentelemetry-otlp` use) from `src/telemetry.rs` to `src/cli/telemetry_init.rs`; `src/telemetry.rs` keeps only span/trace types (`RequestTrace`, `capture_content` users). Confirm `grep -rn "process::exit\|println!" src --include='*.rs' | grep -v '^src/cli/\|^src/main.rs\|test'` prints nothing
- [ ] T033 [US2] Extend `tests/dummyplug_e2e.rs` to go through `VizierRuntime` only (`start` → `transport().send_agent_command(Create)` → chats → `shutdown`), and add: (a) a forced subsystem stop via a `#[cfg(test)]`/`#[doc(hidden)]` hook on `VizierRuntime` asserting `wait()` returns `SubsystemStopped`, `health()` shows `Stopped`, an event was broadcast and the test process is still running (SC-008); (b) a second `VizierRuntime::start` on the same workspace returning the "already in use" error; (c) `start` with an empty workspace returning an error
- [ ] T034 [US2] Add `examples/headless.rs`: parse `--workspace <path>`, `VizierConfig::with_workspace`, `VizierRuntime::start`, print every `RuntimeEvent`, create a `dummyplug` agent, send one chat, print the reply and the turn's `session_events`, `shutdown`, exit 0 — using only stable-tier items plus `transport()` (contracts/vizier-core-api.md)

### Optional feature groups (PR 5)

- [ ] T035 [P] [US2] In `Cargo.toml` add `[features]` `discord`, `telegram`, `media`, `docker-shell`, `documents`, `utoipa`, `telemetry`, `full` and mark their dependencies `optional = true` exactly as research R9's table (versions untouched); keep `default = ["full"]` **for now** so the single-crate binary is unchanged (T046 sets core's `default = []`)
- [ ] T036 [US2] Gate Discord/Telegram: `#[cfg(feature = "discord")]` on `pub mod discord` in `src/channels/mod.rs`, on `kinds::discord` and its `Arc::new(kinds::discord::DiscordKind)` line in `src/integrations/mod.rs:35`, on `src/agents/tools/discord/` and `src/utils/discord.rs`; same for `telegram` (`src/channels/telegram/`, `kinds::telegram`, `src/agents/tools/telegram/`, `src/utils/telegram.rs`, `channels/side_chat` Telegram ledger); a stored integration of a missing kind takes the existing unknown-kind path
- [ ] T037 [US2] Gate `media` (`src/stt/whisper.rs`, `src/tts/kokoro.rs` and their registration in `src/stt/mod.rs`/`src/tts/mod.rs`, symphonia/hound decoding users), `docker-shell` (`src/agents/shell/docker/` and the `ShellConfig::Docker` arm in `src/agents/shell/mod.rs:27`, which without the feature returns `VizierError("docker shell support is not compiled into this build")`), and `documents` (`src/agents/tools/office/`, `kinds::office` registration in `src/integrations/mod.rs:37`, PDF/DOCX extraction in `src/agents/tools/session_files/mod.rs:138`, which without the feature returns a "not supported in this build" result)
- [ ] T038 [US2] Gate `utoipa`: replace `utoipa::ToSchema`/`IntoParams` derives and `#[schema(..)]`/`#[param(..)]` attributes in the 34 non-HTTP files (`grep -rln utoipa src --include='*.rs' | grep -v '^src/channels/http'`) with `#[cfg_attr(feature = "utoipa", …)]`; the HTTP module keeps plain derives
- [ ] T039 [US2] Gate `telemetry`: `src/llm/telemetry.rs` histograms and the `tracing_opentelemetry` context propagation in `src/telemetry.rs`/`RequestTrace` behind `#[cfg(feature = "telemetry")]` with no-op fallbacks that keep the same function signatures; `opentelemetry-otlp` stays a binary-only dependency (T032)
- [ ] T040 [US2] Make `cargo check --all-targets --no-default-features` and `cargo test --no-default-features --test dummyplug_e2e` pass; make the `core-minimal` CI job required; run `cargo run --no-default-features --example headless -- --workspace $(mktemp -d)/ws` (quickstart §3)

**Checkpoint**: US2 independent test passes; server zero-diff; harness green in both feature sets.

---

## Phase 5: User Story 1 — The server edition keeps working exactly as before (Priority: P1)

**Goal**: The physical split into four crates with every install, release and runtime path unchanged (FR-001, FR-010, FR-011, FR-017, FR-018, FR-021; research R2, R10, R18).

**Independent Test**: quickstart §4 (help/OpenAPI zero diff on a copied previous-release workspace, dummyplug session, WebUI, second-instance refusal, exit codes, traces), §5 (deps), §7 (cross build, release dry run) all pass.

### Move the files

- [ ] T041 [US1] Create the virtual workspace root `Cargo.toml`: `[workspace] members = ["crates/*"]`, `default-members = ["crates/vizier"]`, `resolver = "3"`, `[workspace.package]` (version `0.13.5`, edition 2024, authors, license, repository), `[workspace.dependencies]` with every version string copied verbatim from today's `Cargo.toml` (keep `fastembed = "=4.9.1"`, `monty = "=1.0.0"`, `monty-types = "=1.0.0"` and their comments) plus `vizier-derive`, `vizier-core`, `vizier-http` as `{ path = "crates/…", version = "0.13.5" }` (research R18)
- [ ] T042 [US1] `git mv vizier-derive crates/vizier-derive`; set its manifest to `version.workspace = true` etc. (keep `edition = "2021"`)
- [ ] T043 [US1] `git mv src crates/vizier-core/src`, `git mv assets/catalogue crates/vizier-core/assets/catalogue`, `git mv templates crates/vizier-core/templates`, `git mv fixtures/office crates/vizier-core/tests/fixtures/office`, `git mv tests/dummyplug_e2e.rs crates/vizier-core/tests/`, `git mv examples crates/vizier-core/examples`; fix `include_str!`/`include_bytes!`/`#[folder]` paths that are relative (`src/constant.rs`, `src/catalogue/seed.rs`, `src/skill/builtin.rs`) and fixture paths in `src/agents/tools/office/fixture_tests.rs`
- [ ] T044 [US1] `git mv crates/vizier-core/src/channels/http crates/vizier-http/src` (contents become `crates/vizier-http/src/{api,auth,models,state.rs,webui,…}` with `mod.rs` → `lib.rs`), `git mv assets/integrations crates/vizier-http/assets/integrations`; rewrite `crate::` paths that point into core as `vizier_core::` (the `#[doc(hidden)]` tier) and paths within the adapter as `crate::`
- [ ] T045 [US1] `git mv crates/vizier-core/src/cli crates/vizier/src/cli`, `git mv crates/vizier-core/src/command crates/vizier/src/command`, `git mv crates/vizier-core/src/main.rs crates/vizier/src/main.rs`; split `cli/server.rs`, `cli/config_resolve.rs`, `cli/logo.rs`, `cli/telemetry_init.rs` into `crates/vizier/src/` per plan's tree; rewrite paths to `vizier_core::` / `vizier_http::`
- [ ] T046 [US1] Write `crates/vizier-core/Cargo.toml` (`[features]` from T035 with `default = []`, `full` = all groups; `axum` and the HTTP mocks only under `[dev-dependencies]` for `src/llm/*_tests.rs`), `crates/vizier-http/Cargo.toml` (`vizier-core = { workspace = true, features = ["utoipa"] }`, axum, axum-extra, tower-http, utoipa, utoipa-axum, utoipa-swagger-ui, rust-embed with `interpolate-folder-path`, jsonwebtoken, argon2, plus what its code imports), `crates/vizier/Cargo.toml` (`[[bin]] name = "vizier"`, `vizier-core = { workspace = true, features = ["full"] }`, `vizier-http`, clap, inquire, termimad, daemonize, dotenvy, tracing-subscriber, opentelemetry-otlp, `[package.metadata.binstall]` moved verbatim from the old root)
- [ ] T047 [US1] Split `build.rs`: `crates/vizier-core/build.rs` keeps `embed_catalogue_seed` + `validate_media_seed` with paths relative to the crate; `crates/vizier-http/build.rs` runs `npm run build` in `../../webui` when `../../webui/node_modules` exists, copies `../../webui/build/client` (else the packaged `webui-dist/`) into `$OUT_DIR/webui`, panics with today's message if neither exists, and keeps the `rerun-if-changed` lines (`../../webui/app`, `../../webui/package.json`, `../../design/src`); change `#[folder]` in `crates/vizier-http/src/webui/mod.rs` to `"$OUT_DIR/webui"` (research R10); add `crates/vizier-http/webui-dist/` to `.gitignore`
- [ ] T048 [US1] `cargo build`, `cargo build --release`, `cargo test --workspace`, `cargo clippy --workspace --all-targets -- -D warnings` until clean; verify `Cargo.lock` changes only the workspace crates' own entries (`deps` CI job, T007)

### Paths, scripts and workflows (research R18)

- [ ] T049 [P] [US1] Update `Justfile`: recipes that call `cargo run`/`cargo build`/`cargo watch` keep working through `default-members` (add `-p vizier` where a recipe builds explicitly); fix `catalogue-update` comments; verify `just run`, `just dev`, `just release`, `just run-otel`
- [ ] T050 [P] [US1] Update `scripts/catalogue_update.py` `OUT_DIR` (line 41) and its docstring to `crates/vizier-core/assets/catalogue`, and the comment paths to `src/catalogue/…` → `crates/vizier-core/src/catalogue/…`; run it with `--keep-on-failure` and confirm only that directory changes
- [ ] T051 [US1] Update `.github/workflows/release.yml` per research R18: "Update Cargo.toml versions" seds `[workspace.package] version` and the three internal `{ path = "crates/…", version = "…" }` entries in the root `Cargo.toml` and greps those four lines; "Commit version bump" stages `Cargo.toml Cargo.lock CHANGELOG.md crates/vizier-core/assets/catalogue`; "Build binary" uses `cargo build --release -p vizier --target …`; delete the "Need Python Support?" block (lines 298-302)
- [ ] T052 [US1] Update `.github/workflows/publish.yml` per research R18 (phase-3 part): "Verify version in Cargo.toml" reads the version with `cargo metadata --no-deps --format-version 1 | jq -r '.packages[] | select(.name=="vizier") | .version'`; "Verify package" lists `cargo package -p vizier-core --list` and checks `assets/catalogue/api.json`; publish steps temporarily publish `vizier-derive`, `vizier-core`, `vizier-http`, `vizier` in that order with the existing `sleep 30` between them (T062 replaces this with `--workspace`)
- [ ] T053 [US1] Retarget `.github/workflows/ci.yml`: `core-minimal`/`core-full` run `-p vizier-core` (`--no-default-features` / `--all-features`, including `--test dummyplug_e2e` and `--example headless`); `server` runs `--workspace`
- [ ] T054 [P] [US1] Update path references in `CLAUDE.md` (Commands, "Build gotcha" now about `crates/vizier-http/build.rs`, Architecture paths `src/…` → `crates/vizier-core/src/…`, `src/channels/http` → `crates/vizier-http/src`, `src/cli` → `crates/vizier/src/cli`, a new "Workspace layout" paragraph with the one-way dependency rule)
- [ ] T055 [P] [US1] Update `docs/src/getting-started/contributing.md` (and any other `docs/src/` page naming `src/`, `build.rs` or `assets/`) for the new layout; leave `docs/src/SUMMARY.md` structure unchanged unless a page is added

### Verify

- [ ] T056 [US1] Run quickstart §4 in full on a copy of `target/037-baseline/workspace`: help and OpenAPI zero diff, `scripts/dummyplug-vizier-e2e.mjs`, WebUI login + deep-link reload + integration icon, dummyplug chat (`tools`, `execute_python`, JSON request, `context`), second instance refused with exit 1, `vizier agent ps`, `vizier shutdown` exit 0, busy-port exit 1, Jaeger traces, `cargo run -- run --config dev.vizier.yaml`
- [ ] T057 [US1] Run quickstart §5 (deps zero diff, three `=` pins present) and `cross build --release --target x86_64-unknown-linux-musl -p vizier` (quickstart §7)
- [ ] T058 [US1] Run `gh workflow run release-and-publish.yml --ref 037-core-extraction -f version=<next> -f dry_run=true` and check every item in quickstart §7's list (version bump lines, staged files, four archives, Docker image, publish dry run of the crates that exist, no `python` text); this is the PR 6 merge gate

**Checkpoint**: Four crates, server indistinguishable from the baseline, release dry run green.

---

## Phase 6: User Story 3 — Contributors can tell the layers apart and keep them apart (Priority: P2)

**Goal**: CI enforces the layering and the dependency freeze (FR-015, SC-003, SC-004).

**Independent Test**: on a scratch branch, adding `axum` as a normal dependency of `vizier-core` (or `use clap` in it) makes the `layering` job fail; the clean branch passes every CI job.

- [ ] T059 [US3] Add the `layering` job to `.github/workflows/ci.yml`: `cargo tree -p vizier-core -e normal --all-features --prefix none --format '{p}'` must not match `^(axum|axum-extra|tower-http|utoipa-axum|utoipa-swagger-ui|clap|inquire|termimad|daemonize|dotenvy|opentelemetry-otlp) ` and `cargo tree -p vizier-http -e normal` must not match `^(clap|inquire|termimad|daemonize) `; print the offending path (`cargo tree -i <crate>`) on failure
- [ ] T060 [US3] Make `server`, `webui`, `core-minimal`, `core-full`, `layering`, `deps` required checks on `master` (document the branch-protection change in the PR description; applying it is a maintainer action)
- [ ] T061 [US3] Prove the gate: on a throwaway branch add `axum = { workspace = true }` under `[dependencies]` of `crates/vizier-core/Cargo.toml`, push, confirm `layering` fails with the path printed; delete the branch; link the failed run in the PR description

**Checkpoint**: Violations are rejected automatically.

---

## Phase 7: User Story 4 — The packages can be published to the public registry (Priority: P2)

**Goal**: Each crate packages, documents and dry-run publishes in order (FR-004, FR-016, FR-017; research R16, R18).

**Independent Test**: quickstart §6 passes, and the release dry run's publish step lists all four crates.

- [ ] T062 [US4] Finish `.github/workflows/publish.yml` per research R18: add the "Stage WebUI" step (`mkdir -p crates/vizier-http/webui-dist && cp -r webui/build/client/. crates/vizier-http/webui-dist/`) after "Build WebUI"; "Verify package" also checks `cargo package -p vizier-http --list` contains `webui-dist/index.html` and `assets/integrations/`, then runs `cargo package --workspace --allow-dirty` and fails if any `target/package/*.crate` exceeds 10 MB; replace the four publish steps with `cargo publish --workspace --allow-dirty` (keep the four-step order as a commented fallback); dry run uses `cargo publish --workspace --allow-dirty --dry-run` and lists all four packages in the summary; add `cargo add vizier-core` to the post-publish summary
- [ ] T063 [P] [US4] Add package metadata to `crates/vizier-core/Cargo.toml`: `description`, `keywords`, `categories`, `readme = "README.md"`, `include = ["/src/**/*", "/build.rs", "/Cargo.toml", "/README.md", "/assets/catalogue/**/*", "/templates/**/*", "/examples/**/*"]` (tests and fixtures excluded), `[package.metadata.docs.rs] features = ["discord", "telegram", "documents", "utoipa"]`
- [ ] T064 [P] [US4] Add package metadata to `crates/vizier-http/Cargo.toml`: `description`, `keywords`, `readme`, `include = ["/src/**/*", "/build.rs", "/Cargo.toml", "/README.md", "/assets/integrations/**/*", "/webui-dist/**/*"]`, `[package.metadata.docs.rs]`
- [ ] T065 [P] [US4] Set `crates/vizier/Cargo.toml` `description`, `keywords`, `readme = "../../Readme.md"` (or a copied `README.md` if cargo refuses an out-of-crate readme), `include` for `src/**`; confirm `[package.metadata.binstall]` is byte-identical to the old root's
- [ ] T066 [P] [US4] Write `crates/vizier-core/README.md`: what the engine is, the feature table (research R9), the headless example as the quick start (`cargo add vizier-core`, `examples/headless.rs`), the stable vs hidden tier note, pre-1.0 versioning
- [ ] T067 [P] [US4] Write `crates/vizier-http/README.md`: `serve`/`router`, the WebUI embedding and how a source build finds `webui/` (R10)
- [ ] T068 [US4] Document the stable tier in `crates/vizier-core/src/lib.rs` (crate docs with the contracts/vizier-core-api.md example as a doctest marked `no_run`), add a `prelude` module re-exporting exactly the items in research R3, and `#[doc(hidden)]` the remaining `VizierTransport` methods (everything except `send_request`, `send_agent_command`, `subscribe_session_events`)
- [ ] T069 [US4] Run quickstart §6: stage WebUI, `cargo package --workspace --allow-dirty`, sizes < 10 MB, `DOCS_RS=1 cargo doc -p vizier-core --no-deps --features discord,telegram,documents,utoipa`, `cargo public-api -p vizier-core` shows only stable-tier items; save the `cargo public-api` output as `crates/vizier-core/public-api.txt` for future diffs
- [ ] T070 [US4] Add the `package` job to `.github/workflows/ci.yml`: build WebUI, stage it, `cargo package --workspace --allow-dirty`, size report, `DOCS_RS=1 cargo doc -p vizier-core --no-deps`, `cargo public-api -p vizier-core` diffed against `crates/vizier-core/public-api.txt` (a change requires updating the file in the same PR), and a `cargo semver-checks -p vizier-core` step that is skipped until the first `vizier-core` release exists
- [ ] T071 [US4] Re-run the release dry run (T058 command) and confirm the publish step lists `vizier-derive`, `vizier-core`, `vizier-http`, `vizier`; this is the PR 7 merge gate

**Checkpoint**: All four crates package and dry-run publish; docs build; public API recorded.

---

## Phase 8: Polish & Cross-Cutting Concerns

- [ ] T072 [P] Add a "Workspace layout" section to `docs/src/getting-started/contributing.md` (or a new `docs/src/architecture/crates.md` linked from `docs/src/SUMMARY.md`) with the package table from spec.md, the dependency rule, which crate a new tool/route/CLI flag goes in, and that `vizier-app` will be GitHub-Releases-only
- [ ] T073 [P] Draft the release-notes entry (PR 6/7 description and the next `CHANGELOG.md` via `git-cliff`): no user action needed; new `vizier-core`/`vizier-http` crates; the one behaviour change (second instance on one workspace refused, research R14)
- [ ] T074 Final full run: `scripts/037-baseline-check.sh`, quickstart §2–§7, `cargo clippy --workspace --all-targets -- -D warnings`, `cargo test --workspace`, `cd webui && npm run typecheck`; confirm no migration or legacy code was removed (`git diff master --stat -- '*migrat*' '*legacy*'` shows only moves) and that nothing new needs a #82 entry

---

## Dependencies & Execution Order

### Phase dependencies

- **Setup (Phase 1)** → **Foundational (Phase 2)** → **US5 (Phase 3)** → **US2 (Phase 4)** → **US1 (Phase 5)** → **US3 (Phase 6)** and **US4 (Phase 7)** (these two in parallel) → **Polish (Phase 8)**
- Order is by technical dependency, not priority: US5's inversions block the engine standing alone (US2); US2's runtime and features block the split (US1); the split blocks layering checks (US3) and packaging (US4)
- US1's gate (`scripts/037-baseline-check.sh` + harness) runs at **every** checkpoint, so US1 is protected from Phase 2 on even though its own tasks (the split) come in Phase 5

### Within phases

- T008 before T009; T011 → T012 → T013 → T014 → T015 → T016 → T017
- T018 before T019, T020, T021; T022, T023, T024 independent of each other
- T025 → T026 → (T027, T029, T030 in any order) → T031 → T032 → T033 → T034; T028 before T031
- T035 before T036–T039; T040 last in Phase 4
- T041 → T042 → T043 → T044 → T045 → T046 → T047 → T048; then T049–T055; then T056 → T057 → T058
- T059 → T061; T060 after T059
- T062–T068 before T069 → T070 → T071

### Parallel Opportunities

- Phase 1: T005, T006, T007 (separate jobs in one new file — split across commits or one author)
- Phase 3: T020, T021, T022, T023 after T018/T019
- Phase 4: T036, T037, T038, T039 after T035 (different modules)
- Phase 5: T049, T050, T054, T055 after T048
- Phase 6 and Phase 7 run in parallel; inside Phase 7, T063–T067

## Parallel Example: User Story 5

```bash
# after T018 (auth.rs) and T019 (HTTP middleware) land:
Task: "T020 Caller from &Identity in src/access.rs"
Task: "T021 RevisionOrigin from &Identity in src/schema/revision.rs"
Task: "T022 Move RefreshStatus to src/catalogue/mod.rs"
Task: "T023 Move RateLimiter to src/utils/rate_limit.rs"
```

## Parallel Example: User Story 2 (feature groups)

```bash
# after T035 (Cargo.toml features):
Task: "T036 Gate discord/telegram"
Task: "T037 Gate media, docker-shell, documents"
Task: "T038 cfg_attr utoipa in 34 files"
Task: "T039 Gate telemetry"
```

## Implementation Strategy

### MVP

Phases 1–4 (PRs 1–5): the engine runs headless inside any host, reports instead of exiting, and builds with no optional features — the third-party story (US2) works from a git dependency on the still-single crate, and the server is unchanged. Stop and validate with quickstart §2–§5.

### Incremental delivery

1. PR 1 (Setup) → CI + baselines
2. PR 2 (Foundational) → library
3. PR 3 (US5) → no engine → HTTP imports
4. PRs 4–5 (US2) → runtime, headless example, feature groups — **MVP**
5. PR 6 (US1) → four crates, release dry run green
6. PR 7 (US3 + US4 + Polish) → enforcement, packaging, docs

Each PR is conventional-commit titled (table above), passes CI, and must show zero diff in `scripts/037-baseline-check.sh`.

## Notes

- [P] tasks = different files, no dependencies on incomplete tasks
- Commit after each task or logical group in conventional-commit format; use `just commit "<message>"` (no submodule content changes are expected, but the recipe keeps branches aligned)
- Use `git mv` for every move so history follows
- Never bump a dependency version; the `deps` job and T057 check it
- Don't delete migrations, legacy shims or `legacy`/`migrated_from` fields while moving code (#82)

## Implementation notes

- **T002/T003**: baselines were recorded from the debug build of `0eee255` (the `--help` and
  `/openapi.json` output don't depend on the profile; `PROFILE=release scripts/037-baseline-check.sh`
  checks a release build). The upgrade workspace was seeded with that build (v0.13.5, the
  current release); `VIZIER_JWT_SECRET` must be set for `vizier run`/`shutdown` without a config.
- **T008 (deviation from R11)**: `dummyplug` is no longer an in-process Rust model; it is the
  Node server in the `dummyplug` submodule (spec 030). The harness starts it on a free port
  and stores that URL as the provider's `base_url`. It is skipped locally without Node or the
  submodule and fails under `CI`. `agent_config` leaves `prompt_timeout`/`tools.timeout` at
  zero, so the harness sets the HTTP API's defaults (60m/30m).
- **T005 (deviation)**: the tree had ~110 clippy style/complexity/perf warnings before CI
  existed, so CI runs `-D warnings -A clippy::style -A clippy::complexity -A clippy::perf`
  (`CLIPPY_LINTS` in `ci.yml`): rustc warnings and correctness/suspicious lints fail. Later
  tasks' "`-D warnings`" means these flags.
- **T010**: `cargo search` finds neither name and `crates.io/api/v1/crates/vizier-{core,http}`
  returns 404 for both (2026-10-30).
- **T016**: the only new diagnostics were the three `src/utils/mod.rs` doc examples, which never
  ran while the crate was a binary; they now import their function and pass as doctests.
- **T019**: `channels/http/auth` re-exports `crate::auth::{AuthMethod, Identity}` so handlers
  keep importing them from their own `auth` module (the path still works after the split).
- **T022**: `RefreshStatus` carries a plain `//` comment, not a doc comment: utoipa would turn a
  doc comment into the schema description and change `/openapi.json`.
- **T024**: the socket keeps its `Message` channel for pings, pongs and closes; the connection
  gets a second channel of JSON values, and the writer task `select!`s over both.
  `scripts/client-tools-e2e.mjs` passed against a dummyplug agent; `GET /api/v1/catalogue`
  returns the same `refresh` block.
