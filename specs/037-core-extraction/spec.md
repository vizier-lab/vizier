# Feature Specification: Extract vizier-core and split the workspace

**Feature Branch**: `037-core-extraction`

**Created**: 2026-10-30

**Status**: Draft

**Input**: User description: "https://github.com/vizier-lab/vizier/issues/102 let's tackle this issue"

## Scope of this spec

Issue #102 is an epic of nine phases (0–8). This spec covers **phases 0–4**: the guardrails, the extraction of the engine into a publishable library, the removal of the layering inversions that block it, the split into separate packages, and making the library publishable. These are the phases the issue states are shippable with **identical server behaviour**. Phases 5–8 (service layer, WebUI transport seam, Tauri desktop shell, native capabilities) are **out of scope** and get their own specs; this spec only has to leave the engine in a shape they can build on.

## User Scenarios & Testing *(mandatory)*

### User Story 1 - The server edition keeps working exactly as before (Priority: P1)

An operator who runs, installs or deploys vizier today (cargo install, binary release, Docker, `just run`) upgrades to the restructured version and notices nothing: same commands, same config files, same data directory, same HTTP/WebSocket/WebUI behaviour, same Discord/Telegram behaviour.

**Why this priority**: The split is pure restructuring. If existing users see any change, the work has failed regardless of what else it delivers. Every later phase relies on this being a safe baseline.

**Independent Test**: Run the existing server against an existing workspace before and after the change, with an agent on the offline `dummyplug` provider, and compare the observable results of the same scripted session (tool listing, a tool call, context echo, HTTP API responses, WebUI loading).

**Acceptance Scenarios**:

1. **Given** an existing workspace and config from the previous release, **When** the operator starts the new build with the same command, **Then** all agents, conversations, memory, tasks and integrations load and behave as before, with no migration step.
2. **Given** a running server, **When** a client uses the HTTP API, WebSocket and bundled WebUI, **Then** every route, payload and status code is unchanged.
3. **Given** a subsystem failure (as today), **When** it stops unexpectedly, **Then** the server process still exits with a failing status so Docker/systemd restart it.
4. **Given** the install and release paths (cargo install, release archives, Docker image, install script), **When** they are used after the change, **Then** they produce a working `vizier` binary under the same name with no change to how users invoke them.

---

### User Story 2 - A third party can build their own interface on the engine (Priority: P1)

A developer who wants to build a custom front end (terminal UI, desktop app, bot) adds `vizier-core` as a dependency, starts the engine from their own program with a workspace path they choose, creates an agent, sends it a message, and receives replies and session events — without pulling in an HTTP server, a CLI parser, the WebUI, or Node tooling.

**Why this priority**: This is the headline deliverable of the issue ("let other people build a third"), and the reason the crate boundary matters.

**Independent Test**: Build and run the shipped headless example against the offline `dummyplug` provider in a clean directory; it must start the engine, run a turn and print the reply and events, then shut down cleanly.

**Acceptance Scenarios**:

1. **Given** a program depending only on the core library, **When** it builds, **Then** the dependency tree contains no web-server framework, command-line parser, interactive-terminal prompt libraries or WebUI build step.
2. **Given** an explicit workspace location supplied by the host program, **When** the engine starts, **Then** it uses that location and does not read environment variables or the user's home directory to decide it.
3. **Given** a subsystem failure inside the engine, **When** it occurs, **Then** the engine reports it to the host as data/events and does not terminate the host process.
4. **Given** the example program, **When** it sends a message to an agent, **Then** it receives the reply and the session events through the library's public surface.

---

### User Story 3 - Contributors can tell the layers apart and keep them apart (Priority: P2)

A maintainer working on the codebase can see which package owns what (engine, HTTP adapter, CLI/server binary), and automated checks fail if a lower layer starts depending on a higher one or if a protected boundary is crossed (for example, the engine importing the web framework).

**Why this priority**: Without enforcement the layering decays back into the current inversions. It is lower than P1 because users do not see it directly.

**Independent Test**: Introduce a deliberate violation (the engine referencing the web framework) on a scratch branch and confirm CI rejects it; confirm a clean tree passes every CI job.

**Acceptance Scenarios**:

1. **Given** the restructured repository, **When** CI runs, **Then** it checks the engine without optional features, the engine with all features, the server, and the WebUI, and all pass.
2. **Given** a change that makes the engine depend on the HTTP adapter or CLI, **When** CI runs, **Then** it fails.
3. **Given** the in-process end-to-end harness on `dummyplug`, **When** it runs in CI, **Then** it exercises a real agent turn through the engine and gates every merge.

---

### User Story 4 - The packages can be published to the public registry (Priority: P2)

A maintainer can publish the engine, the HTTP adapter and the server binary in dependency order, and each package passes a packaging dry run with documentation that builds, within the registry's size limit.

**Why this priority**: Publishing is the last step that turns the restructure into the third-party story; it needs the earlier stories first.

**Independent Test**: Run the packaging dry run and documentation build for each package, and the public-API/semver check against the engine, in a clean environment.

**Acceptance Scenarios**:

1. **Given** the repository, **When** each package is packaged as a dry run, **Then** it succeeds with the required assets included and stays under the registry size cap.
2. **Given** the engine package, **When** its documentation is built, **Then** it builds and shows the deliberate public surface, with internals not exposed.
3. **Given** the publish order (derive macros → engine → HTTP adapter → server), **When** followed, **Then** each step resolves its dependencies from the registry-equivalent packages.
4. **Given** the already-published `vizier` package name, **When** the server package is released, **Then** it keeps that name, so `cargo install vizier` and binary downloads continue to work.

---

### User Story 5 - The engine does not depend on the web layer's identity types (Priority: P2)

Identity, caller, permission and revision-origin concepts the engine needs are owned by the engine itself and do not reference types from the HTTP layer; HTTP-only concerns (the live-socket message type, refresh-status types defined in a handler) are replaced by transport-agnostic equivalents.

**Why this priority**: This is the prerequisite that makes the split possible, but it is invisible to users until the split lands, so it follows the user-facing outcomes in priority while being sequenced first in the work.

**Independent Test**: With the engine as its own package, build it with the web framework absent from its dependency graph; and confirm client-declared tools and catalogue refresh status work end to end through the server.

**Acceptance Scenarios**:

1. **Given** the engine, **When** it resolves who is acting on a request, **Then** it does so using engine-owned types, and the HTTP layer converts its authenticated user into them.
2. **Given** a client that declares its own tools over a live connection, **When** the engine calls one, **Then** it does so through a transport-neutral channel abstraction, and WebSocket clients behave as before.
3. **Given** catalogue refresh and media listing, **When** they report status, **Then** the status type lives in the catalogue layer, not in an HTTP handler module.

---

### Edge Cases

- A host program starts the engine twice against the same workspace: the second start must fail clearly rather than corrupt data.
- A host program drops the engine handle without a graceful shutdown: running turns must be handled as today (aborted/resumable), not leaked.
- Optional features (Discord, Telegram, media, Docker shell, documents, API schema generation, telemetry) are turned off: the engine must still build and run an agent turn, and a request for a disabled capability must fail with a clear message rather than panic.
- A user upgrades from a pre-split release: config file resolution, environment variables, command socket location and data directory resolve identically.
- The WebUI build output is absent when building the engine alone: engine builds must not require it or Node tooling.
- The registry package size limit is exceeded by bundled assets (catalogue seed, templates): the assets that must ship are identified and the package still fits.
- Existing tests and the dependency pins for the embedding and speech libraries must remain untouched; the split must not bump any dependency version.

## Requirements *(mandatory)*

### Functional Requirements

- **FR-001**: The repository MUST provide four separately buildable packages: the engine (`vizier-core`), the HTTP adapter (`vizier-http`), the server binary (`vizier`), and the existing derive macros package (`vizier-derive`), with a one-way dependency direction: server → HTTP adapter → engine, and derive ← engine.
- **FR-002**: The engine MUST NOT depend on the web framework, the command-line parser, interactive terminal prompt/rendering libraries, WebUI embedding, or Node tooling, and MUST NOT terminate the host process.
- **FR-003**: The engine MUST expose a start/stop/health/events surface (a runtime handle) that a host program uses to run agents, send requests, and observe session events, and MUST report subsystem failures to the host as data/events instead of exiting.
- **FR-004**: The engine's public surface MUST be a deliberate, documented facade; everything else MUST be crate-private. Broad access to the internal shared-dependencies object MAY be exposed transitionally and MUST be recorded as such.
- **FR-005**: The engine MUST accept explicit workspace and configuration locations from the host and MUST NOT resolve them from environment variables or the home directory itself; the server binary MUST keep the existing resolution order (`-c`, `$VIZIER_CONFIG`, `./.vizier.yaml`, defaults, `$VIZIER_DATA_DIR`/`$HOME/.vizier`) unchanged.
- **FR-006**: Identity, caller, permission and revision-origin types used by the engine MUST be defined in the engine without reference to the HTTP layer; the HTTP adapter MUST convert its authenticated user into them.
- **FR-007**: Client-declared tools MUST write to a transport-neutral sink abstraction defined in the engine; the WebSocket adapter MUST implement it with unchanged client-visible behaviour.
- **FR-008**: Catalogue/media refresh status types MUST be owned by the catalogue layer, not by an HTTP handler module.
- **FR-009**: Process-level concerns (environment-file loading, logging initialisation, TLS provider installation, process exit, daemonising, the control-socket server and the interactive onboarding wizard) MUST live in the server binary, not the engine.
- **FR-010**: The HTTP adapter MUST own the REST routes, WebSocket handling, authentication middleware, OpenAPI document, embedded WebUI and integration icons, and the WebUI build step; building the HTTP adapter MUST still embed the same WebUI output as today.
- **FR-011**: Assets the engine needs at runtime (catalogue seed and logos, templates) MUST be located inside the engine package so it can be published on its own.
- **FR-012**: Heavy or optional groups (Discord, Telegram, media, Docker shell, documents, API schema generation, telemetry) MUST be selectable by feature flags; the server edition MUST enable all of them by default so its behaviour is unchanged.
- **FR-013**: The engine MUST ship a headless example that starts it on the offline `dummyplug` provider, runs an agent turn, prints the reply and events, and shuts down.
- **FR-014**: An in-process end-to-end harness on `dummyplug` MUST exist and run in CI against the engine, covering at least: starting the engine, tool listing, one tool call through the real loop, the per-request context echo, and a clean shutdown.
- **FR-015**: CI MUST build and test the engine without optional features, the engine with all features, the server, and the WebUI; and MUST fail if a lower layer depends on a higher one.
- **FR-016**: Each publishable package MUST pass a packaging dry run, stay under the registry size cap, and build its documentation; the engine MUST additionally be checked with public-API and semver tooling, with publish order derive → engine → HTTP adapter → server documented.
- **FR-017**: The server package MUST keep the name `vizier` and the `vizier` binary, so installation, binary-install tooling, Docker, the install script and the release workflow keep working with only path updates.
- **FR-018**: Server behaviour (CLI commands and flags, config schema, environment variables, HTTP/WebSocket API, storage schema and data, channel behaviour, process-exit semantics) MUST be identical before and after, with no user-visible migration.
- **FR-019**: No dependency version MAY change as part of this work; in particular the embedding and speech-recognition library pins stay exactly as they are, and the diff MUST be reviewable for this.
- **FR-020**: Project documentation (architecture guide, contributor docs, Docker/Justfile/release paths) MUST be updated to describe the new layout and the one-way dependency rule.

### Key Entities

- **Engine (vizier-core)**: The library that owns agents, tools, storage, providers, scheduler, integrations, channel readers, notifications, usage, secrets and the in-process message bus, and exposes a runtime handle and events.
- **Runtime handle**: What a host program holds to start, stop, health-check and subscribe to a running engine; the only way the engine reports lifecycle and failures.
- **Caller / acting identity**: Who a request is made on behalf of, plus the method used to authenticate; owned by the engine so permission checks and revision attribution don't depend on a transport.
- **Client sink**: A transport-neutral outlet for messages to a connected client, implemented by the WebSocket adapter.
- **HTTP adapter (vizier-http)**: The package that exposes the engine over REST/WebSocket and serves the WebUI and icons.
- **Server binary (vizier)**: The CLI package that starts the engine, mounts the HTTP adapter, and owns process-level behaviour.
- **Headless example**: A small program proving the engine can be used without any shell.

## Success Criteria *(mandatory)*

### Measurable Outcomes

- **SC-001**: An existing operator upgrades with zero manual steps: 100% of the scripted before/after session comparisons on `dummyplug` (tools, tool call, context echo, API responses) match.
- **SC-002**: A new developer can go from an empty directory to an agent reply using only the engine package and its example in under 15 minutes, following the package documentation.
- **SC-003**: The engine's dependency graph contains none of: the web framework, the CLI parser, the terminal prompt/rendering libraries; verified by an automated check on every merge.
- **SC-004**: All four CI jobs (engine minimal, engine full, server, WebUI) pass on the merged result, and a deliberate upward-dependency violation is rejected by CI.
- **SC-005**: Every publishable package passes its packaging dry run under the registry size cap and builds its documentation.
- **SC-006**: Zero dependency version changes in the restructure diff.
- **SC-007**: All existing install and release paths (cargo install, release archives, Docker image) produce a working server in a release dry run covering every currently shipped target.
- **SC-008**: The engine never exits the host process in any tested failure scenario (a forced subsystem failure surfaces as an event/health state and the host keeps running).

## Assumptions

- Phases 5–8 of issue #102 (service layer, WebUI transport seam, Tauri app, native capabilities) are separate specs; the desktop identity method and web-access toggle are therefore not built here, though the identity types are shaped not to prevent them.
- The crate name `vizier-core` is available on the public registry (to be re-confirmed in the guardrails step), and the `vizier` name remains owned by this project.
- The server's observable behaviour is the contract; internal module paths and the facade's exact contents are free to change.
- Pre-1.0 versioning applies: the engine's public API may break in minor releases, and exposing the shared-dependencies object is a transitional measure to be narrowed by the later service-layer spec.
- Windows support, retiring local speech libraries (#101), upgrading the embedding library, and splitting the model catalogue into its own package remain deferred, as stated in the issue.
- Migration and backward-compatibility code stays untouched and tracked in #82; any new compatibility shim introduced by the split is added to that checklist.
- The WebUI and design submodules are not changed by this spec, and the existing commit-order cascade (design → webui → vizier) is unaffected.
