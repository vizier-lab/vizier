<!--
Sync Impact Report
==================
Version change: 1.1.0 → 1.2.0
Modified principles: none
Added sections: none
Modified sections:
  - Development Workflow & Quality Gates — added the "End-to-end
    verification through a dummyplug agent" gate: features that change
    agent-observable behavior MUST be exercised against a running binary
    with an agent on the offline `dummyplug` provider
    (specs/008-dummyplug-provider), with each feature's quickstart.md
    scripted as dummyplug steps; the dummyplug chat protocol is now a test
    harness contract.
Templates requiring updates:
  - .specify/templates/plan-template.md ✅ updated (Testing field names the
    dummyplug e2e harness)
  - .specify/templates/tasks-template.md ✅ updated (Polish phase gets a
    dummyplug e2e task next to quickstart validation)
  - .specify/templates/spec-template.md ✅ no change needed (Independent Test
    wording already fits; the harness is a plan/tasks concern)
  - .specify/templates/checklist-template.md ✅ no constitution references present
  - CLAUDE.md ✅ updated (Commands section points at dummyplug for e2e checks)
Follow-up TODOs: none
-->

# Vizier Constitution

## Core Principles

### I. Lean by Default
Every addition — dependency, abstraction, config surface, or line of code —
MUST justify its own existence. Prefer the smallest change that solves the
actual, current requirement over one that anticipates future needs. Do not
introduce a trait, generic parameter, config flag, or crate for a single
call site or a hypothetical second implementation; add the abstraction when
the second real use case shows up, not before. New third-party dependencies
require a concrete reason a hand-rolled few lines cannot cheaply provide,
and must be checked against Principle III before being added.

**Rationale**: A single-binary agent framework accumulates complexity
quickly across providers, channels, storage backends, and tools. Unjustified
abstractions and dependencies compound into slow builds, a larger attack
surface, and code paths nobody can confidently delete.

### II. DRY via Trait-Based Extensibility (NON-NEGOTIABLE)
Behavior that varies by kind (storage backend, channel, provider, tool,
shell) MUST be expressed as one implementation behind a shared trait, never
as `match`/`if` branching over a type tag scattered through call sites.
Adding a new variant means implementing the trait and registering it in its
module's constructor — existing dispatch code MUST NOT change. Shared logic
used by two or more of these variants belongs in one place (a trait default
method, a shared helper, or a supertrait) — copy-pasted logic across
providers/backends/channels is a defect, not a style preference. When a
third near-identical code path appears, it MUST be collapsed into the
shared implementation before new work continues on top of it.

**Rationale**: This is how the codebase already scales its provider,
storage, and tool matrices (`VizierStorageProvider`, `VizierTool`,
`VizierChannel`). Duplication across these surfaces is the single biggest
threat to long-term leanness — every duplicated branch is a second place a
future bug fix or provider quirk must be remembered and applied.

### III. Self-Contained, Zero-Dependency Runtime
A built release binary MUST run without requiring the user to install,
configure, or connect to any external service — no external database,
message broker, cache, or reverse proxy. Storage MUST remain embedded
(bundled SQLite); the WebUI's static assets MUST be embedded into the
binary at build time, not fetched or served from a separate deployment. Configuration MUST have working built-in defaults
("config-less mode") so `vizier run` with zero setup is always a valid
starting point. Any feature that inherently needs a network resource at
runtime (a model provider API, an optional embedding-model download) MUST
be opt-in and clearly degrade — never a requirement for the default path.

**Rationale**: The project ships as `curl | sh` and `cargo install`
one-binary installs. Any hidden runtime dependency (a required external DB,
an assumed sidecar process, non-embedded static assets) breaks that promise
and pushes operational burden onto the user.

### IV. Portability by Default
Code MUST NOT assume a specific OS, filesystem layout, or CPU
architecture without going through an existing abstraction (`dirs`, the
storage traits, the shell execution abstraction). New platform-specific
behavior must be justified and isolated behind a trait implementation
(consistent with Principle II), not `cfg`-scattered through business logic.
Cross-compilation targets (including static `musl` builds) MUST keep
building — a new dependency that breaks `cross build` for an existing
target in `Cross.toml` is a blocking regression, not a follow-up. Paths,
line endings, and process/shell invocation MUST go through existing
platform-neutral abstractions rather than raw OS calls.

**Rationale**: Vizier already ships prebuilt binaries across architectures
and both Docker and bare-metal installs. Portability is a distribution
requirement, not an aspiration — a regression here is a shipped-artifact
failure, not a code-quality nitpick.

### V. Unified Errors & Observability
All fallible internal code MUST return `crate::Result<T>` (`VizierError`),
converting external errors with `throw_vizier_error` rather than inventing
per-module error types or ad hoc `String`/`anyhow` leakage into library
code. `unwrap()`/`expect()` are reserved for tests and `main`'s bootstrap.
All logging MUST go through `tracing` (`info!`/`warn!`/`error!`/…) —
`println!`/`eprintln!` are not permitted outside of CLI-output paths that
are explicitly user-facing terminal output.

**Rationale**: One error type and one logging facade is itself an
application of DRY (Principle II) to cross-cutting concerns: it keeps error
handling and log filtering (per-crate directives in `main.rs`) predictable
across a codebase with dozens of providers, channels, and storage backends
instead of each module reinventing its own convention.

## Distribution & Technology Constraints

- **No required external services**: the only supported storage backend is
  embedded SQLite (`rusqlite` with the `bundled` feature — statically
  linked, no system SQLite dependency). A prior filesystem-based backend
  was removed once agent memory itself moved onto a pluggable
  `DocumentStore` abstraction writing human-readable documents to disk
  (see `specs/004-memory-open-format/`), leaving no remaining entity that
  needed a second, flat-file storage backend. Adding a storage backend
  that requires a running external server (Postgres, Redis, etc.) as the
  *only* option is out of scope for this project; such a backend, if ever
  added, MUST remain strictly opt-in and additive.
- **Single-binary output**: `cargo build --release` (with the WebUI
  pre-built or `webui/node_modules` present) MUST produce one self-contained
  executable capable of serving the WebUI, running channels, and persisting
  state without further installation steps.
- **Dependency additions are reviewed for weight, not just correctness**:
  before adding a crate, check whether an existing dependency already
  covers the need (e.g., don't add a second HTTP client alongside
  `reqwest`, a second templating/markup crate, a second JSON path
  library). Prefer crates already in the dependency tree.
- **Optional network features must degrade gracefully**: features that
  fetch something at runtime (e.g., a `fastembed` model download for local
  embeddings) MUST be gated behind explicit user configuration and MUST NOT
  be on the default/minimal path a fresh `vizier onboard` produces.
- **Cross-compilation targets in `Cross.toml` are a compatibility
  contract**: changes that require new system libraries on those targets
  must update the corresponding `pre-build` steps in the same change, not
  as a follow-up.

## Development Workflow & Quality Gates

- **Extensibility by implementation, not branching**: reviewers MUST reject
  new tools/channels/storage backends/providers that are added by extending
  an existing `match` over a type enum instead of implementing the relevant
  trait and registering it in its module constructor (per Principle II).
- **Before adding code, check for an existing home for it**: new logic
  that duplicates behavior already present in another provider, channel,
  storage backend, or tool implementation MUST be refactored into a shared
  location instead of copied.
- **`cargo clippy` and `cargo test` MUST pass** before a change is
  considered done; there is no separate `just lint`/`just test` — use these
  directly. WebUI changes MUST pass `cd webui && npm run typecheck`.
- **Conventional commits** (`feat:`, `fix:`, `doc:`, `perf:`, `refactor:`,
  `chore:`, with `[**breaking**]` where applicable) are required, since the
  changelog is generated from commit history via `git-cliff`.
- **Manual verification for runtime-affecting changes**: since the
  automated test suite is sparse, changes to process startup, storage
  migrations, or the build pipeline (`build.rs`, `Cross.toml`, `Dockerfile`)
  MUST be exercised by actually running the binary (`just run` / `cross
  build`), not assumed correct from a passing `cargo build`.

- **End-to-end verification through a `dummyplug` agent**: a change to
  agent-observable behavior — the agent loop, hooks, tools (built-in or
  MCP), sessions/history, memory/CORE, the scheduler or dream cycle,
  channels, or the HTTP/WebUI surface — MUST be exercised end to end
  against a running binary with an agent on the offline `dummyplug`
  provider before it is considered done. Dummyplug is the default e2e
  harness: no credentials, no network, and deterministic tool invocation
  (`tools` → tool name → JSON request → echoed result). Each feature's
  `quickstart.md` MUST script its checks as dummyplug steps: the message
  to send and the expected reply, stored state, or history entry. A live
  provider is used in addition only where behavior depends on real model
  output (prompting, provider-specific parsing, token accounting), and
  those steps MUST be marked as such. The dummyplug chat protocol
  (`specs/008-dummyplug-provider/contracts/dummyplug-protocol.md`) is a
  test-harness contract: changing it MUST keep existing quickstart scripts
  working or update them in the same change. *Rationale*: the unit suite
  is sparse and live models are nondeterministic, paid, and need keys;
  dummyplug makes the real pipeline reproducible offline (Principle III).

## Governance

This constitution supersedes ad hoc practice and prior informal convention
for anything it explicitly addresses. `CLAUDE.md` remains the authoritative
day-to-day guidance for architecture and conventions; where the two
conflict, this constitution's Core Principles take precedence and
`CLAUDE.md` MUST be updated to match rather than silently diverge.

**Amendments**: proposed via a pull request that edits this file directly,
describing the principle(s) touched and the reasoning. Amendments MUST
update the Sync Impact Report comment at the top of this file and bump the
version per the policy below.

**Versioning policy** (semantic versioning applied to governance):
- **MAJOR**: a principle is removed or redefined in a way that is backward
  incompatible with prior guidance (e.g., relaxing the zero-external-service
  storage constraint).
- **MINOR**: a new principle or a materially expanded constraint/section is
  added.
- **PATCH**: wording clarifications, typo fixes, or non-semantic
  refinements that don't change what is required or forbidden.

**Compliance review**: every PR/code review pass MUST check the diff
against these principles, in particular Principle II (no new type-branching
where a trait impl belongs) and Principle III (no new required external
service or non-embedded runtime dependency). Deviations MUST be justified
in the PR description (equivalent to the plan template's Complexity
Tracking table) or the change MUST be revised before merge.

**Version**: 1.2.0 | **Ratified**: 2026-08-13 | **Last Amended**: 2026-09-27
