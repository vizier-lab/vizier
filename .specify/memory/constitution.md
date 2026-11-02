<!--
Sync Impact Report
==================
Version change: 1.3.0 → 2.0.0
Modified principles:
  - III. Self-Contained, Zero-Dependency Runtime — embedded SQLite/local
    storage remains the mandatory zero-setup default; PostgreSQL and
    S3-compatible storage are now permitted as strictly opt-in server
    deployment backends.
Added sections: none
Removed sections: none
Modified sections:
  - Distribution & Technology Constraints — documents the embedded default
    and the optional managed-storage topology.
Templates requiring updates:
  - .specify/templates/plan-template.md ✅ no change needed (Constitution
    Check reads the gates from this file)
  - .specify/templates/spec-template.md ✅ no change needed
  - .specify/templates/tasks-template.md ✅ no change needed
  - AGENTS.md ✅ updated with the shared SQL and blob-store architecture
  - docs/src/configuration/storage-shell.md ✅ updated with backend setup,
    topology, backups and migration
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

### III. Self-Contained Default, Optional Managed Services
A built release binary MUST run without requiring the user to install,
configure, or connect to any external service — no external database,
message broker, cache, or reverse proxy. The default database and file
backends MUST remain bundled SQLite and the local workspace, and the WebUI's
static assets MUST remain embedded into the binary. Configuration MUST have
working built-in defaults ("config-less mode") so `vizier run` with zero
setup is always a valid starting point. Server operators MAY explicitly
select managed PostgreSQL and S3-compatible storage; those backends MUST be
strictly opt-in, independently selectable, and MUST NOT affect the default
path or the desktop app. Any other feature that inherently needs a network
resource at runtime (a model provider API, an optional embedding-model
download) MUST likewise be opt-in and clearly degrade.

**Rationale**: The project ships as `curl | sh` and `cargo install`
one-binary installs, while server operators also need durable managed
storage. Preserving embedded defaults protects the zero-operations promise;
making managed services explicit avoids hidden runtime dependencies.

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

- **No required external services**: embedded SQLite (`rusqlite` with the
  `bundled` feature) and local workspace files are the default and MUST
  remain fully supported without system services. PostgreSQL with pgvector
  and S3-compatible object storage are supported only as explicit,
  additive server choices; neither may become the sole backend or a desktop
  dependency. The prior flat-file entity backend survives only as a
  migration source; human-readable memory documents now use the shared
  `DocumentStore`/`BlobStore` abstraction on either local or S3 storage.
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
- **Conventional commits everywhere**: every commit message AND every pull
  request title MUST be in conventional-commit format (`feat:`, `fix:`,
  `doc:`, `perf:`, `refactor:`, `chore:`, with `[**breaking**]` where
  applicable), in the workspace and in every submodule. The changelog is
  generated from history via `git-cliff`, and a squash-merged PR title
  becomes the commit. Tool-generated messages (Spec Kit auto-commits, bots)
  are held to the same rule; a `[Spec Kit] ...` style prefix is not allowed.
- **Matching submodule branches, driven by `just`**: every spec branch
  (`NNN-short-name`) MUST exist under the same name in the workspace and in
  every submodule (`design`, `webui`, `landing`, `dummyplug`), so a feature
  that spans repos is one branch name everywhere and its PRs can be linked.
  Cross-repo git interaction MUST go through the Justfile recipes, not
  hand-rolled per-submodule git: `just checkout <branch>` (create or switch
  everywhere), `just status`, `just pull`, `just commit "<message>"`,
  `just push` (submodules first, then the workspace, so pins never point at
  unpushed commits), `just branch-rm`, `just worktree`. If a needed
  interaction has no recipe, add one to the Justfile instead of repeating
  the raw commands. Commit order across repos is design → webui → vizier.
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

**Version**: 2.0.0 | **Ratified**: 2026-08-13 | **Last Amended**: 2026-11-02
