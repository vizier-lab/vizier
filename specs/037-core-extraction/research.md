# Research: Extract vizier-core and split the workspace

Each entry is **Decision / Rationale / Alternatives considered**. Facts about the current tree
were read from the code on 2026-10-30 (`master` at `0eee255`): one crate, `src/` ≈ 111k lines,
of which `src/channels/http/` ≈ 16k and `src/cli/` + `src/command/` ≈ 0.9k.

---

## R1 — Order of work: library-ify in place, then move files

**Decision**: Do phases 0–2 inside the existing single crate (add `src/lib.rs`, shrink
`main.rs`, fix the inversions), then do the physical split in phase 3 with `git mv` into
`crates/`, then phase 4 packaging. Each phase is one or more PRs that leave `master` shippable.

**Rationale**: The issue's own sequencing: mechanical risk (moving 111k lines) is kept apart
from design risk (where a type lives). While still one crate, every existing test and the
dummyplug harness catch regressions with no path churn. `git mv` keeps `git log --follow`
and blame usable.

**Alternatives considered**: Split first and fix inversions across crate boundaries — every
inversion becomes a compile error in a crate that cannot build yet, so nothing is testable
until all are fixed at once. Rejected.

---

## R2 — Crate layout and the workspace root

**Decision**: Virtual workspace at the repo root, `members = ["crates/*"]`,
`default-members = ["crates/vizier"]`, and a shared `[workspace.package]` (version, edition,
license, repository, authors) plus `[workspace.dependencies]` holding **the exact version
strings from today's `Cargo.toml`, copied verbatim**. Each crate takes `dep.workspace = true`.
`Cargo.lock` stays at the root and must not change any resolved version (R12).

| Crate | Path | Publish |
|---|---|---|
| `vizier-derive` | `crates/vizier-derive` (moved, unchanged, keeps edition 2021) | crates.io |
| `vizier-core` | `crates/vizier-core` | crates.io |
| `vizier-http` | `crates/vizier-http` | crates.io |
| `vizier` | `crates/vizier` (keeps `[package.metadata.binstall]`, the `vizier` bin) | crates.io |

**Rationale**: `default-members` keeps `cargo run -- run --config dev.vizier.yaml`,
`cargo build --release` and `cargo install --path .`-style muscle memory working, so the
Justfile and contributor docs need only path changes (FR-017/FR-018). One version for all
four crates matches today's lock-step `vizier`/`vizier-derive` bump in `release.yml`.

**Alternatives considered**: Keep `vizier` as the root package with `crates/*` members —
leaves the root `src/` ambiguous and makes `include` rules awkward. Independent versions per
crate — adds release bookkeeping with no current consumer asking for it (Principle I).

---

## R3 — The public facade of `vizier-core`

**Decision**: Two tiers.

1. **Stable facade** (documented, checked by `cargo public-api` / `cargo-semver-checks`):
   `VizierRuntime`, `RuntimeOptions`, `RuntimeEvent`, `RuntimeExit`, `HealthReport`,
   `config::VizierConfig` (+ its value types), `auth::{Identity, AuthMethod, Role}`,
   `access::Caller`, `VizierTransport` (only `send_request`, `send_agent_command` and
   `subscribe_session_events`, reached through `VizierRuntime::transport()`), the
   request/response DTOs in `schema` that a host needs to create an agent, send a chat and
   read a reply (`VizierRequest`, `VizierRequestContent`, `VizierResponse`, `VizierChannelId`,
   `VizierSession`, `SessionEvent`, `AgentCommand`, `AgentConfig`), `VizierError`, `Result`,
   and a `prelude` (see `contracts/vizier-core-api.md`).
2. **Adapter surface** (transitional): the modules `vizier-http` and `vizier` need today
   (`dependencies`, `transport`, `storage`, `catalogue`, `llm`, `artifacts`, `session_files`,
   `setup`, `provider_keys`, `provider_migration`, `integrations`, `usage`, `agents`,
   `skill`, `utils`, …) are `pub` but `#[doc(hidden)]`, re-exported under
   `vizier_core::__adapter` *only by path* (no glob), with a crate-level doc paragraph saying
   they are unstable and will be replaced by the service layer (phase 5). Everything else is
   `pub(crate)`.

**Rationale**: `channels/http` alone references 19 top-level core modules (`schema` 119
times, `catalogue` 32, `storage` 25, …). Making them all stable API would freeze internals;
making them private is impossible until phase 5 builds services. rustdoc JSON omits
`#[doc(hidden)]` items, so `cargo public-api` and `cargo-semver-checks` see only tier 1,
which is exactly the contract FR-004 asks for. The issue already accepts exposing
`VizierDependencies` transitionally (decision 7).

**Alternatives considered**: A cargo feature `unstable-internals` gating the adapter
modules — every downstream crate would need it, and a feature that changes visibility is
easy to misuse. A private `vizier-internal` crate — a fifth publishable crate just to hide
types (Principle I). Rejected both.

---

## R4 — Identity and caller types (inversions in `access.rs`, `schema/revision.rs`)

**Decision**: New module `vizier_core::auth` holds `Identity` (renamed from
`AuthenticatedUser`: `user_id`, `username`, `role`, `permissions`, `method`) and `AuthMethod`
(`Jwt`, `ApiKey`; non-exhaustive so phase 7 can add `Desktop`). `Role` already lives in
`storage::user` and is re-exported from `auth`. `access::Caller` gets `From<&Identity>`;
`schema/revision.rs` builds `RevisionOrigin` from `&Identity`. `vizier-http` keeps
`AuthService` (JWT issue/verify, argon2 password hashing, API-key hashing) and its axum
middleware, which now inserts an `Identity` into request extensions; handlers use
`Extension<Identity>`.

**Rationale**: Only the *identity* is needed by core (permission checks, revision
attribution). Token and password mechanics are how HTTP authenticates and have no other
caller today (`argon2`/`jsonwebtoken` are used only under `channels/http`). Moving them would
pull JWT and argon2 into every embedder for nothing (Principle I). `#[non_exhaustive]` lets
the desktop method land later without a breaking change.

**Alternatives considered**: Move `AuthService` to core now, as the issue's layout sketch
suggests ("identity, permissions, tokens") — revisit in the phase 7 spec when the app's
web-access toggle needs to set a password; not needed for phases 0–4. A type alias
`AuthenticatedUser = Identity` in http to cut churn — two names for one thing; rejected, the
rename is mechanical.

---

## R5 — Client tools without `axum::extract::ws::Message`

**Decision**: `ClientConnection` holds an `mpsc::Sender<serde_json::Value>` instead of
`mpsc::Sender<axum::extract::ws::Message>`. The WebSocket writer task in `vizier-http`
converts each value to `Message::Text`. The test helper that decoded `Message::Text` decodes
the JSON value directly.

**Rationale**: `ClientConnection` only ever sends one kind of frame — JSON text
(`connection.rs:87`). A bounded channel of JSON values *is* the transport-neutral sink FR-007
asks for; a Tauri adapter later drains the same receiver into `emit`. A `ClientSink` trait
would have one implementation today, which Principle I forbids.

**Alternatives considered**: A `ClientSink` trait as named in the issue — add it when a second
real sink with different semantics (e.g. synchronous IPC) shows up. `Sender<String>` —
loses nothing but makes every consumer re-parse; JSON value keeps the frame typed.

---

## R6 — `RefreshStatus` and `RateLimiter`

**Decision**: `RefreshStatus` moves from `channels/http/api/v1/catalogue.rs` to
`catalogue/mod.rs` (`catalogue::RefreshStatus`), with `ToSchema` behind the `utoipa` feature
(R9). `RateLimiter` (pure `std`, 111 lines) moves from `channels/http/rate_limit.rs` to
`utils/rate_limit.rs`, since `VizierDependencies.side_chat_limits` already needs it in core;
`vizier-http` imports it from there for its own limiters on `HTTPState`.

**Rationale**: Both are domain/utility types that only *happen* to sit in the HTTP tree.
No behaviour change.

**Alternatives considered**: Duplicate `RateLimiter` per crate — Principle II defect.

---

## R7 — Process-level concerns move to the binary; the runtime reports, never exits

**Decision**:

- `vizier` (binary) owns: `dotenvy`, `tracing_subscriber` init and the default filter,
  `Telemetry::init` (OTLP exporter setup), the rustls provider install, `print_logo`,
  daemonising, clap, `inquire` onboarding, `termimad` output, the command socket server
  (`src/command`) and **config-file and workspace resolution** (`-c` → `$VIZIER_CONFIG` →
  `./.vizier.yaml` → defaults; `$VIZIER_DATA_DIR` → `$HOME/.vizier`).
- `vizier-core` exposes `VizierRuntime::start(RuntimeOptions) -> Result<VizierRuntime>`.
  It spawns, on the **host's** tokio runtime, the core subsystems: agent manager,
  scheduler, channel readers (`ChannelConnections` reconcile + watch), and the transport's
  command handler. It never calls `process::exit` and never builds its own tokio runtime.
- `VizierRuntime::wait(&mut self) -> RuntimeExit` resolves to
  `RuntimeExit::Requested` (the existing `exit_signal`, e.g. `vizier shutdown`) or
  `RuntimeExit::SubsystemStopped { name, outcome }`. The same stop is also broadcast as
  `RuntimeEvent::SubsystemStopped` and recorded in `HealthReport`.
- The binary builds the tokio runtime (worker count from config), starts the runtime, spawns
  `vizier_http::serve(&runtime, http_config)` and the command server as its own subsystems,
  and keeps today's rule: any stop → log and `process::exit(1)`; requested exit →
  `exit(0)` (#51 preserved, US1 scenario 3).
- `VizierConfig::load(path)` is split: core keeps `VizierConfig::from_path(&Path)` and
  `VizierConfig::with_workspace(PathBuf)` (built-in defaults for an explicit workspace);
  the binary's `config_resolve::resolve(args)` applies the env/home resolution order and
  calls them. `apply_overrides` stays in core (pure).

**Rationale**: Exactly the inversions listed in the issue (`main.rs`, `config/mod.rs:155,260`,
`cli/run.rs:144,148`). Splitting "who decides to exit" from "who notices a stop" is what lets
an embedder keep running while the server binary keeps its restart semantics.

**Alternatives considered**: Core returns `Err` from `start` only and panics on later
failure — panics in a library are not a failure report. Core owns the tokio runtime
(`VizierRuntime::block_on`) — a Tauri host already has its own runtime; rejected.

**Note**: env-var reads that are *product behaviour* stay in core: provider credentials from
env (`provider_keys.rs`, catalogue `env[]`) and `VIZIER_ENCRYPTION_KEY`/`_FILE`. They are not
location resolution. Phase 7 may make the encryption-key source injectable; out of scope here.

---

## R8 — HTTP server ownership

**Decision**: `vizier-http` exposes `pub async fn serve(runtime: &VizierRuntime, config:
HTTPChannelConfig) -> vizier_core::Result<()>` (returns only if the server stops) plus
`router(runtime, config) -> axum::Router` for embedding. `VizierChannels` in core stops
spawning HTTP; it becomes `ChannelReaders` (readers + watcher, runs forever). The WebUI embed
(`webui/mod.rs`), the integrations icon embed (`api/v1/integrations.rs`), OpenAPI/Swagger and
`tower-http` CORS move with it. `HTTPChannelConfig` stays in core's config (it is part of the
YAML schema) but is plain data.

**Rationale**: Matches the issue's crate table; the app later calls `serve` only when
web-access is on. Keeping `HTTPChannelConfig` in core avoids splitting the YAML schema.

**Alternatives considered**: `vizier-http` implements `VizierChannel` and core spawns it —
needs core to know about HTTP; inverts the dependency again.

---

## R9 — Feature flags on `vizier-core`

**Decision**: `default = []`. Groups, each gating the listed dependencies (made
`optional = true`) and the registration of what uses them:

| Feature | Dependencies | Gated at |
|---|---|---|
| `discord` | `twilight-*` | `IntegrationKinds::builtin()` registration of the Discord kind; `channels/discord` module |
| `telegram` | `teloxide` | same, Telegram kind; `channels/telegram` |
| `media` | `whisper-rs`, `any-tts`, `symphonia`, `symphonia-adapter-opus-rs`, `hound` | local STT/TTS model constructors in `stt`/`tts` registries |
| `docker-shell` | `bollard`, `indicatif` | Docker backend registration in `agents/shell` |
| `documents` | `pdf-extract`, `docx-rs`, `calamine`, `rust_xlsxwriter`, `ironcalc` | office addon kind registration; document text extraction |
| `utoipa` | `utoipa` | `#[cfg_attr(feature = "utoipa", derive(utoipa::ToSchema))]` on the 34 core files that derive it today |
| `telemetry` | `opentelemetry`, `opentelemetry_sdk`, `tracing-opentelemetry` | `llm/telemetry.rs` histograms and `RequestTrace` context propagation (no-op fallbacks when off) |
| `full` | all of the above | — |

`vizier-http` depends on `vizier-core` with `features = ["utoipa"]`; `vizier` with
`features = ["full"]`. `opentelemetry-otlp` (the exporter) is a dependency of the **binary**
only.

A request for a disabled capability fails with a clear message, using paths that already
exist: an integration row whose kind is not registered is the existing "unknown kind" case;
a Docker shell config without `docker-shell` returns `VizierError("docker shell support is
not compiled into this build")` at agent build, surfacing through the supervisor's `Failed`
state like any build error.

**Rationale**: FR-012 / issue decision 5. Gating at the *registration* point (one `cfg` per
constructor line) keeps Principle II: no `cfg` scattered through dispatch. `default = []`
gives third parties the lean build (US2 scenario 1); both editions opt into `full`, so the
server is unchanged.

**Alternatives considered**: `default = ["full"]` — a third party gets the native-toolchain
heavy build by default and must opt *out*; rejected for the library's own use case. Gate
`fastembed` too — it is pinned (#73) and load-bearing for local embeddings; the issue keeps
it out of the groups. Revisit with the fastembed upgrade project.

---

## R10 — WebUI embedding inside a publishable `vizier-http`

**Decision**: `crates/vizier-http/build.rs`:

1. If `../../webui/node_modules` exists → `npm run build` in `../../webui` (today's rule).
2. Pick the source: `../../webui/build/client` if present, else the packaged
   `crates/vizier-http/webui-dist/` (present only inside a published `.crate`).
3. Copy it to `$OUT_DIR/webui` and point `rust-embed` at it via
   `#[folder = "$OUT_DIR/webui"]` (rust-embed's `interpolate-folder-path` feature; already a
   dependency, so a feature flag, not a new crate).
4. Neither found → panic with today's message.

The publish workflow copies `webui/build/client` into `crates/vizier-http/webui-dist/`
(gitignored, listed in `include`) right before `cargo package`/`publish`.
`assets/integrations/` moves to `crates/vizier-http/assets/integrations/`.

**Rationale**: Cargo cannot package files outside the crate directory, and a build script
must not write into the source tree (`cargo package` verification rejects it). Staging only
at publish time keeps the working tree clean; `OUT_DIR` is always writable.

**Alternatives considered**: A symlink `crates/vizier-http/webui -> ../../webui` — symlink
handling in packaging and on Windows checkouts is fragile. Commit built assets — churns
history. Rejected.

---

## R11 — In-process dummyplug harness (phase 0)

**Decision**: An integration test (`crates/vizier-core/tests/dummyplug_e2e.rs`; an in-crate
`#[cfg(test)]` module before phase 3) that: creates a temp workspace; starts a runtime with
the built-in Rust `dummyplug` provider (`agents/agent/model/dummyplug.rs`, no network); creates
an agent through `AgentCommand::Create`; sends `tools`, a tool name, a JSON tool request and
`context` as `Chat` requests through `VizierTransport::send_request`; asserts the replies per
`specs/008-dummyplug-provider/contracts/dummyplug-protocol.md`; triggers a forced subsystem
stop (test-only hook) and asserts `RuntimeExit::SubsystemStopped` *without* the test process
exiting; then shuts down. The same body becomes `examples/headless.rs` minus assertions.

**Rationale**: FR-013/FR-014/SC-008 from one piece of code. The Node dummyplug server
(submodule) needs Node and a port; the built-in provider needs nothing, which is what an
in-process gate in CI wants.

**Alternatives considered**: Drive the Node `scripts/dummyplug-vizier-e2e.mjs` against a
built binary as the only gate — kept as the black-box before/after check (R13), but too slow
and too outside-in to be the per-PR regression gate for core.

---

## R12 — Proving "no dependency version changes" (FR-019 / SC-006)

**Decision**: A CI step compares `cargo metadata --format-version 1 --locked` package
`(name, version, source)` triples on the PR against `master`; any difference other than the
four workspace crates' own entries fails the job. In particular `fastembed = "=4.9.1"`,
`monty = "=1.0.0"` and `monty-types = "=1.0.0"` keep their `=` pins in
`[workspace.dependencies]`.

**Rationale**: Version strings move from one `Cargo.toml` to the workspace table, so a text
diff of manifests is noisy; the resolved graph is the real contract. Note: the issue's
decision 10 names `sherpa-onnx`, which spec 032 has since replaced with `whisper-rs` +
`any-tts`; those are covered by the same check.

**Alternatives considered**: Diff `Cargo.lock` — works, but feature unification can
legitimately add/remove *unused* entries when features become optional; the triple check
ignores entries absent on one side only if they are explained by a now-optional feature
(the job prints them for review instead of failing).

---

## R13 — Proving "server behaviour identical" (FR-018 / SC-001)

**Decision**: Three checks, run before the first phase-1 PR (baseline) and on every phase PR:

1. **OpenAPI document byte-identical**: `GET /openapi.json` from the built server equals the
   baseline (covers every route, payload schema and status code declared).
2. **CLI surface identical**: `vizier --help` and each subcommand's `--help` equal the baseline.
3. **Black-box dummyplug session**: `scripts/dummyplug-vizier-e2e.mjs` against the built
   binary on a copy of a seeded workspace from the previous release passes unchanged.

Plus the in-process harness (R11) and `cargo test --workspace`.

**Rationale**: Each is cheap, deterministic and covers a different face of the contract.

**Alternatives considered**: Hand-written golden HTTP fixtures per route — duplicates the
OpenAPI document.

---

## R14 — One workspace, one engine (edge case: double start)

**Decision**: `VizierRuntime::start` takes an exclusive lock on
`{workspace}/.runtime/vizier.lock` with `std::fs::File::try_lock` (stable since Rust 1.89;
the toolchain here is 1.99) and fails with `VizierError("workspace … is already in use by
another vizier process")` if held. Released on drop.

**Rationale**: Spec edge case 1. `std` covers it, no new crate (Principle I/IV).

**Deviation note**: today two servers on different ports *can* be started on one workspace
(they then race on SQLite and the memory documents). Refusing the second is a deliberate,
user-visible change against the letter of FR-018 and is called out in the release notes.
It is the only intended behaviour change in this spec.

**Alternatives considered**: `fs2`/`fs4` crates — superseded by `std`.

---

## R15 — CI matrix (phase 0)

**Decision**: New `.github/workflows/ci.yml` on PRs and `master`:

| Job | Runs |
|---|---|
| `core-minimal` | `cargo clippy -p vizier-core --no-default-features -- -D warnings`, `cargo test -p vizier-core --no-default-features` (incl. the dummyplug harness) |
| `core-full` | the same with `--all-features` |
| `server` | build WebUI, `cargo clippy --workspace`, `cargo test --workspace`, `cargo build -p vizier --release`, R13 checks |
| `webui` | `cd webui && npm ci && npm run typecheck && npx vitest run` |
| `layering` | `cargo tree -p vizier-core -e normal --all-features` must not contain `axum`, `axum-extra`, `tower-http`, `utoipa-axum`, `utoipa-swagger-ui`, `clap`, `inquire`, `termimad`, `daemonize`, `dotenvy`, `opentelemetry-otlp`, `rust-embed` *of webui*; plus R12 |
| `package` (phase 4) | `cargo package --workspace` (multi-package packaging with a local overlay, Cargo ≥ 1.90), size report, `cargo doc -p vizier-core --no-deps`, `cargo public-api -p vizier-core`, `cargo semver-checks -p vizier-core` (after first publish) |

Before phase 3 (single crate) `core-*` run against the single crate with the same feature
sets. Upward dependencies between workspace crates are already impossible: cargo rejects the
cycle `vizier-core → vizier-http → vizier-core`, so `layering` guards the third-party
crates that mark a layer.

**Rationale**: FR-015, SC-003, SC-004. There is no test workflow today (only release,
publish, docker, landing, submodule PRs).

**Alternatives considered**: One job running everything — slower feedback, and a minimal
build that breaks is hidden behind a full build that passes.

---

## R16 — Packaging and publishing (phase 4)

**Decision**: Assets move inside the crate that embeds them: `assets/catalogue/` and
`templates/` → `crates/vizier-core/`; `fixtures/office/` → `crates/vizier-core/tests/fixtures/`
(excluded from the package); `assets/integrations/` → `crates/vizier-http/`. Each crate gets
`include`, `description`, `keywords`, `readme`, and `[package.metadata.docs.rs]`
(`vizier-core`: `features = ["discord", "telegram", "documents", "utoipa"]`, i.e. the groups
that need no native toolchain or network at build time). `publish.yml` publishes in order
`vizier-derive → vizier-core → vizier-http → vizier`; `release.yml`'s version sed targets
`[workspace.package] version` and the internal path deps' `version =` requirements.
`vizier-app` does not exist yet and is never published (spec: GitHub Releases only).

**Rationale**: FR-011, FR-016, FR-017. Current sizes: `assets/catalogue` 5.6 MB raw (JSON +
SVG logos, compresses well), WebUI build 3.8 MB raw — both comfortably under the 10 MB
compressed crates.io cap; the `package` job reports the real number every run.

**Risk**: docs.rs builds offline; `fastembed`→`ort-sys` downloads binaries at build time but
skips that under `DOCS_RS`; to be confirmed with `DOCS_RS=1 cargo doc -p vizier-core` in the
`package` job. FR-016 requires the docs to *build* (checked locally/CI); docs.rs rendering
is best-effort.

---

## R17 — Submodules and branches

**Decision**: Per constitution v1.3.0, create `037-core-extraction` in every submodule with
`just checkout 037-core-extraction`. No submodule content changes are planned in this spec:
`webui/` is consumed unchanged (only its build location moves under `vizier-http`'s build
script), `design/`, `landing/`, `dummyplug/` untouched. `landing`'s `sync-vizier.mjs` reads
`docs/src/` which stays at the root.

**Rationale**: Constitution "Matching submodule branches"; the spec's assumption that the
WebUI is unchanged.
