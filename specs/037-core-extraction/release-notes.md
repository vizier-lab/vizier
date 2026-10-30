# Release notes draft: vizier-core and the workspace split

For the pull request descriptions and, after the next release, the changelog (`git-cliff` builds
`CHANGELOG.md` from the conventional commits; this is the prose for the release page).

## For people who run vizier

Nothing to do. `cargo install vizier`, `cargo binstall vizier`, the release archives and the Docker
image are unchanged, and so are the command line, the configuration file, the environment
variables, the HTTP API and your data. Upgrade as usual.

**One behaviour change:** starting a second `vizier run` on a workspace that another vizier process
is already using now fails at startup with "workspace … is already in use by another vizier
process" (exit code 1). Before, the second process started and both wrote to the same database and
memory documents. The lock is `{workspace}/.runtime/vizier.lock`; it is released when the process
exits, however it exits.

## For people who build on vizier

Two new crates on crates.io, next to `vizier` and `vizier-derive`:

- **`vizier-core`**: the agent engine as a library, with no web framework, command-line parser or
  terminal UI. `VizierRuntime::start` runs it on a workspace you choose, inside your own tokio
  runtime; it reports a stopped subsystem instead of exiting your process. Optional parts
  (`discord`, `telegram`, `media`, `docker-shell`, `documents`, `utoipa`, `telemetry`) are features,
  all off by default. See `crates/vizier-core/examples/headless.rs`. Pre-1.0: the minor version may
  change the API, and everything outside `VizierRuntime`, `config`, `schema`, `auth`, `error`,
  `transport` and `access` is `#[doc(hidden)]` and not part of it.
- **`vizier-http`**: the REST, WebSocket and embedded WebUI adapter (`serve`, `router`).

The desktop app planned in #102 will be distributed through GitHub Releases only.

## For contributors

The repository is a Cargo workspace: `crates/vizier-derive`, `crates/vizier-core`,
`crates/vizier-http`, `crates/vizier`. A bare `cargo run` still starts the server. Source paths in
issues and docs move from `src/…` to `crates/vizier-core/src/…` (engine), `crates/vizier-http/src/…`
(HTTP) and `crates/vizier/src/…` (CLI). CI now has `server`, `webui`, `core-minimal`, `core-full`,
`layering`, `deps` and `package` jobs; the branch protection on `master` should require them.
