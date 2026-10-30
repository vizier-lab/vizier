# Implementation Plan: Extract vizier-core and split the workspace

**Branch**: `037-core-extraction` | **Date**: 2026-10-30 | **Spec**: [spec.md](spec.md)

**Input**: Feature specification from `specs/037-core-extraction/spec.md` (issue #102, phases 0–4)

## Summary

Turn today's single `vizier` crate into a virtual workspace of four publishable crates —
`vizier-derive`, `vizier-core` (engine), `vizier-http` (axum adapter + embedded WebUI) and
`vizier` (CLI/server binary) — with the server's observable behaviour unchanged. The work is
sequenced so mechanical risk and design risk never land together (research R1):

| Phase | What lands | Still one crate? |
|---|---|---|
| 0 Guardrails | CI matrix, in-process dummyplug harness, baselines (OpenAPI, `--help`, deps) | yes |
| 1 Library-ify | `src/lib.rs` + thin `main.rs`; stable facade vs `#[doc(hidden)]` adapter surface | yes |
| 2 Break inversions | `auth::Identity`, `RefreshStatus`/`RateLimiter` moved, client channel of JSON values, `VizierRuntime` that reports instead of exiting, config resolution in the binary, HTTP server started by the binary, feature flags, workspace lock | yes |
| 3 Split | `git mv` into `crates/{vizier-core,vizier-http,vizier,vizier-derive}`, workspace deps, build scripts split, Justfile/Docker/release paths | no |
| 4 Publishable | assets inside crates, `include`/docs.rs metadata, `examples/headless.rs`, `cargo package --workspace`, public-api/semver checks, publish order | no |

The desktop app (`vizier-app`, GitHub Releases only), the service layer and the WebUI
transport seam are later specs; this plan only keeps room for them (`AuthMethod` is
non-exhaustive, the client channel is transport-neutral, `vizier-http` exposes `serve` and
`router` for a second host).

## Technical Context

**Language/Version**: Rust, edition 2024 (`vizier-derive` stays 2021); toolchain 1.99 (needs ≥ 1.89 for `File::try_lock`, ≥ 1.90 for `cargo package --workspace`)

**Primary Dependencies**: unchanged set and versions (research R12). Moved between crates: `axum`, `axum-extra`, `tower-http`, `utoipa-axum`, `utoipa-swagger-ui`, `jsonwebtoken`, `argon2`, `rust-embed` (WebUI/icons) → `vizier-http`; `clap`, `inquire`, `termimad`, `daemonize`, `dotenvy`, `tracing-subscriber`, `opentelemetry-otlp` → `vizier`. Made optional in core: `twilight-*`, `teloxide`, `whisper-rs`, `any-tts`, `symphonia*`, `hound`, `bollard`, `indicatif`, office crates, `utoipa`, `opentelemetry*` (research R9). `rust-embed` gains its `interpolate-folder-path` feature (R10).

**Storage**: embedded SQLite + markdown documents — **no schema or data change**; the one new on-disk file is the lock `{workspace}/.runtime/vizier.lock` (R14)

**Testing**: `cargo test --workspace`, `cargo clippy --workspace`, `cd webui && npm run typecheck && npx vitest run`; new in-process harness `crates/vizier-core/tests/dummyplug_e2e.rs` (R11); before/after gates: `/openapi.json` byte diff, `--help` diffs, resolved-dependency diff (R12, R13); end-to-end: dummyplug steps in [quickstart.md](quickstart.md)

**Target Platform**: the four release targets (`x86_64`/`aarch64` linux-gnu, `x86_64`/`aarch64` apple-darwin), `x86_64-unknown-linux-musl` via `Cross.toml`, the Docker image; Windows remains out of scope

**Project Type**: Rust library (`vizier-core`, `vizier-http`) + CLI/server binary (`vizier`)

**Performance Goals**: none new; startup time and turn latency must not regress measurably (the split adds no runtime indirection)

**Constraints**: zero behaviour change for the server except R14; zero dependency version change; each `.crate` < 10 MB compressed; core's normal dependency graph excludes the web framework, CLI and terminal UI crates

**Scale/Scope**: ≈ 111k lines moved; ≈ 16k (`channels/http`) to `vizier-http`, ≈ 0.9k (`cli`, `command`) plus `main.rs`/`telemetry` init to `vizier`; 7 inversions fixed; 34 files get `cfg_attr` for `utoipa`; one new CI workflow; edits to `release.yml`, `publish.yml`, `Justfile`, `Cargo.toml`s, `CLAUDE.md`, `docs/src/`

## Constitution Check

*GATE: Must pass before Phase 0 research. Re-check after Phase 1 design.*

| Principle / gate | Assessment | Status |
|---|---|---|
| I. Lean by Default | Three new crates are the feature itself (spec FR-001). No new third-party crate: the lock uses `std` (R14), the client sink is a channel, not a trait (R5), `AuthService` stays where its only caller is (R4). Feature flags are required by FR-012 and limited to the issue's seven groups. | ✅ |
| II. Trait-based extensibility | Feature gating happens at registration in module constructors (`IntegrationKinds::builtin()`, shell backend, STT/TTS registries), never as `cfg`/`match` in dispatch (R9). `RateLimiter` is moved, not duplicated (R6). | ✅ |
| III. Self-contained runtime | The server binary still embeds the WebUI (through `vizier-http`), keeps embedded SQLite and config-less mode; `cargo build --release` at the root still yields the one binary (`default-members`, R2). | ✅ |
| IV. Portability | No new platform-specific code; `File::try_lock` is portable `std`. `Cross.toml` musl build is a quickstart/CI step; `Cross.toml` itself needs no change (no new system libs). | ✅ |
| V. Unified errors & observability | Stable API returns `vizier_core::Result`/`VizierError`; `anyhow` stays internal (pre-existing internal use is not widened). `print_logo` (the one non-CLI `println!`) moves to the binary. Logging init leaves the library. | ✅ |
| Conventional commits | All commits/PR titles conventional; the split PR is `refactor:` (not breaking for users; the library API is new). | ✅ |
| Matching submodule branches via `just` | `just checkout 037-core-extraction` creates the branch everywhere (R17); no submodule content change planned. | ✅ |
| `cargo clippy` + `cargo test` pass; WebUI typecheck | In CI from phase 0 (R15). | ✅ |
| Manual verification of build pipeline changes | `build.rs` split, `Cargo.toml`, release workflow: quickstart §4, §7 run the binary, `cross build` and a release dry run. | ✅ |
| E2E through dummyplug | In-process harness + quickstart dummyplug steps against the running binary. | ✅ |
| #82 tracking of compat code | No migration or shim is added. Existing migrations move with their modules, untouched. | ✅ |

**Post-design re-check** (after research/data-model/contracts): unchanged — all ✅. One
noted deviation from the spec's FR-018 (not from the constitution): refusing a second server
on the same workspace (R14), recorded in Complexity Tracking.

## Project Structure

### Documentation (this feature)

```text
specs/037-core-extraction/
├── plan.md              # this file
├── research.md          # R1–R17 decisions
├── data-model.md        # boundary types (no storage change)
├── quickstart.md        # baselines + verification steps (dummyplug)
├── contracts/
│   ├── vizier-core-api.md         # stable tier of the library
│   └── server-compatibility.md    # what the binary must keep identical
├── checklists/requirements.md
└── tasks.md             # /speckit-tasks
```

### Source Code (repository root, after phase 3)

```text
Cargo.toml                       # virtual workspace: members = ["crates/*"], default-members = ["crates/vizier"],
                                 # [workspace.package], [workspace.dependencies] (versions verbatim)
Cargo.lock                       # unchanged versions
crates/
├── vizier-derive/               # moved from ./vizier-derive, unchanged
├── vizier-core/
│   ├── Cargo.toml               # features: discord telegram media docker-shell documents utoipa telemetry full; default = []
│   ├── build.rs                 # catalogue seed gzip + media seed validation only (from ./build.rs)
│   ├── assets/catalogue/        # moved from ./assets/catalogue
│   ├── templates/               # moved from ./templates
│   ├── examples/headless.rs     # US2
│   ├── tests/
│   │   ├── dummyplug_e2e.rs     # R11
│   │   └── fixtures/office/     # moved from ./fixtures/office (excluded from package)
│   └── src/
│       ├── lib.rs               # facade + prelude; adapter modules #[doc(hidden)]
│       ├── runtime.rs           # VizierRuntime, RuntimeOptions, RuntimeEvent, RuntimeExit, HealthReport, workspace lock
│       ├── auth.rs              # Identity, AuthMethod (+ Role re-export)
│       ├── access.rs            # Caller: From<&Identity>
│       ├── utils/rate_limit.rs  # moved from channels/http/rate_limit.rs
│       ├── catalogue/           # + RefreshStatus
│       ├── channels/            # readers only: connections, readers, discord (feature), telegram (feature),
│       │                        # commands, reactions, side_chat, woken — no http/
│       └── agents/ artifacts/ config/ dependencies.rs embedding/ encryption/ error.rs file_manager/
│           image_generation/ indexer/ integrations/ llm/ notifications/ sandbox/ scheduler/ schema/
│           secrets/ session_files/ setup.rs skill/ storage/ stt/ telemetry.rs (spans/metrics only)
│           transport.rs tts/ usage/ provider_keys.rs provider_migration.rs constant.rs
├── vizier-http/
│   ├── Cargo.toml               # depends on vizier-core { features = ["utoipa"] }
│   ├── build.rs                 # npm build in ../../webui when node_modules exists; stage webui into $OUT_DIR
│   ├── assets/integrations/     # moved from ./assets/integrations
│   ├── webui-dist/              # gitignored; filled only by the publish workflow
│   └── src/
│       ├── lib.rs               # serve(&VizierRuntime, HTTPChannelConfig), router(..)
│       ├── api/ auth/ models/ state.rs webui.rs openapi   # from src/channels/http/*
└── vizier/
    ├── Cargo.toml               # bin "vizier"; [package.metadata.binstall] moved here; vizier-core { features = ["full"] }
    └── src/
        ├── main.rs              # dotenvy, tracing init, Telemetry::init (OTLP), rustls, exit codes
        ├── server.rs            # tokio runtime, VizierRuntime::start, vizier_http::serve, command server, exit policy (#51)
        ├── config_resolve.rs    # -c / $VIZIER_CONFIG / ./.vizier.yaml / defaults; $VIZIER_DATA_DIR / $HOME/.vizier
        ├── cli/                 # from src/cli (run, shutdown, onboard, skill, agent)
        ├── command/             # from src/command (unix socket)
        └── logo.rs              # from src/utils/logo.rs
.github/workflows/ci.yml         # new (R15)
.github/workflows/{release,publish}.yml   # paths, version bump, publish order (R16)
Justfile                         # paths only; `cargo run -- run` keeps working via default-members
webui/ design/ landing/ dummyplug/        # submodules, unchanged
docs/src/                        # contributor + architecture docs for the layout (FR-020)
```

**Structure Decision**: Rust virtual workspace with crates under `crates/`, exactly the
issue's layout minus `app/` (phase 7). Phase 0–2 changes happen in place under `src/`; phase 3
moves files with `git mv` so history follows. Docs, specs, scripts, submodules and the Docker
files stay at the root.

## Phase sequencing and PR plan

1. **PR 1 — `chore: add CI matrix and dummyplug harness`** (phase 0): `ci.yml`; in-crate
   harness module; baselines recorded per quickstart §1 and attached to the PR.
2. **PR 2 — `refactor: build vizier as a library with a thin main`** (phase 1): `src/lib.rs`,
   `main.rs` only calls `vizier::cli::start`; `pub`/`pub(crate)`/`#[doc(hidden)]` pass; harness
   becomes `tests/dummyplug_e2e.rs`.
3. **PR 3 — `refactor: own identity, refresh status and rate limiter in the engine`**
   (phase 2a): R4, R5, R6.
4. **PR 4 — `refactor: add VizierRuntime and move process concerns out of the engine`**
   (phase 2b): R7, R8, R14; `VizierChannels` → `ChannelReaders`; config resolution split.
5. **PR 5 — `refactor: gate optional engine features`** (phase 2c): R9; CI `core-minimal`
   becomes required.
6. **PR 6 — `refactor: split the workspace into vizier-core, vizier-http and vizier`**
   (phase 3): `git mv`, manifests, build scripts, Justfile, release/publish paths, CLAUDE.md
   architecture section.
7. **PR 7 — `chore: make vizier-core and vizier-http publishable`** (phase 4): R10 staging,
   R16 metadata, `examples/headless.rs`, `package` CI job, docs.

Each PR runs the full quickstart; none may change the OpenAPI document, `--help` output or
resolved dependencies.

## Complexity Tracking

| Violation | Why Needed | Simpler Alternative Rejected Because |
|---|---|---|
| Deviation from spec FR-018: a second server on an in-use workspace now refuses to start (R14) | Spec edge case 1 requires a clear failure instead of data corruption; two processes on one SQLite + document tree is unsafe today | Keeping today's behaviour satisfies FR-018's letter but contradicts the edge case and leaves a corruption path the embedded runtime would inherit |
| `#[doc(hidden)]` adapter surface exposed from a published crate (R3) | `vizier-http` needs ~19 engine modules until the phase-5 service layer exists | Making them stable freezes internals; making them private is impossible before services exist; an extra private crate adds a fifth package for no user value |
