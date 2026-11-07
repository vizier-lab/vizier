# Research: Project Language Intelligence

**Feature**: `054-project-lsp` | **Date**: 2026-11-07

Facts below come from reading `develop` at `9266772c`. Measurements come from
[`lsp-spike.py`](./lsp-spike.py), run on macOS.

## Measured

| Server | Result |
|---|---|
| clangd | `initialize` answered in 1.48 s. Negotiated `positionEncoding: utf-8`. No `diagnosticProvider`, so push only. After `didChange`, the error arrived **0.06 s** later, as a push carrying the edit's version. |
| rust-analyzer, no component | `~/.cargo/bin/rust-analyzer` was a `rustup` proxy. It exits with `Unknown binary 'rust-analyzer' in official toolchain`. See R3. |

rust-analyzer 1.99.1, measured with [`lsp-spike.py`](./lsp-spike.py) and
[`lsp-spike-2.py`](./lsp-spike-2.py):

| Question | Result |
|---|---|
| Capabilities | `positionEncoding: utf-8`; `diagnosticProvider` with `interFileDependencies: true` and `workspaceDiagnostics: false` |
| Own analysis after an edit (pull) | Toy crate 0.17 s. Crate with serde, tokio and regex 0.09 s. **Vizier 1.95 s** (build scripts and proc macros off) |
| What it caught | The type error in the edited file, **and** the broken call site in another **open** file |
| Push without a save | None within 3 s. Pull is the only fast route here |
| `cargo check` after `didSave` | Runs and pushes `rustc` errors, 0.2 s on a toy crate. Not measured on a large one |
| A question during startup | Returns `[]` while `Fetching` is open, which looks like "no definition" |
| Time to quiet | Toy crate 4.2 s. Serde/tokio/regex 14 s (6.6 s of it `Building compile-time-deps`). Vizier 26 s (build scripts and proc macros off) |
| Memory on Vizier | **Peak 2.4 GB**, 1.4 GB settled (build scripts and proc macros off, so a real run may use more) |
| `cargo build` during `cargo check` | **Blocked**: `Blocking waiting for file lock`, 5.3 s on a small crate, until the check ended |
| stdin closes after the handshake | Exits in 0.07-0.12 s. **Before** `initialized`, it stayed running |
| Parent killed, pipe closed | Gone in 0.2 s |
| Parent killed, pipe held open by another process | Still running after 15 s, so it does not honour `processId` |

Not measured: `typescript-language-server`, `pyright-langserver`, `gopls` and `lua-language-server` (not
installed here). `tests/lsp_real.rs` covers whichever is present.

Inferred, not run: with build scripts on, a server rooted at Vizier would run
`crates/vizier-http/build.rs`, which runs `npm run build` in `webui/`. The spike kept them off to avoid
writing to the repository.

## R1. Where a server lives and who owns it

**Decision**: One `LspHost` on `VizierDependencies`, beside `project_ledger`, passed to tools through
`ProjectTurnContext`. Servers are keyed `(project_id, root, language_id)`.

**Rationale**: A project has exactly one agent, so per-agent ownership needs no separate key. Chats and
subagent pieces on one folder resolve to the same root and share a server. A worktree has a different
root path, so it gets its own (FR-016).

**Fleet**: the host is local to the process holding the folder, like the project git routes and the
read ledger, which `service/projects.rs` already calls directly. So `service` calls the host directly
too, and no bus subject is added for it. `scripts/048-agent-state-read-check.sh` covers a running
agent's state, which this is not. When Fleet places projects on separate nodes, the host moves with
those routes.

**Alternatives**: Per-agent host (no benefit, more reaping paths). A new `AgentsClient` operation (a
seam for a call that has only one possible location today; Principle I).

## R2. Protocol code

**Decision**: Hand-written JSON-RPC framing and only the message shapes used, over `serde_json`. Add
`url` as a direct dependency for `file://` conversion; it is already in `Cargo.lock`.

**Rationale**: About ten message shapes are used, and `lsp-types` adds a large type surface for them
(Principle I). Windows drive letters and percent-encoding are worth a proven crate.

**Alternatives**: `lsp-types`, `async-lsp`, `tower-lsp` (the last is for writing servers).

## R3. Finding a server, and "found but cannot run"

**Decision**: Resolve in this order: project override, then the built-in table. Look up the command on
the search path by hand, honouring `PATHEXT` on Windows. **A command is only "installed" once it has
answered `initialize`.** Exiting before answering, with a spawn failure or a known "not installed"
pattern on stderr, reads as `missing`, with the language's install hint. Any other early exit reads as
`failed`.

**Rationale**: The rust-analyzer case above is real: the command is on `PATH` and cannot run. The Rust
hint is `rustup component add rust-analyzer`. Commands are never resolved inside the project folder
unless a person set that override (FR-011).

## R4. Search path for a GUI app (FR-013)

**Decision**: On Unix, if a command is not on the process's `PATH`, read the login shell's once
(`$SHELL -lc 'printf %s "$PATH"'`, 3 s limit, off the async runtime) and add the well-known folders
(`~/.cargo/bin`, `~/.local/bin`, `~/go/bin`, `/opt/homebrew/bin`, `/usr/local/bin`). The result is
cached in the host and also passed to the child as its `PATH`, because rust-analyzer needs to find
`cargo`. Windows GUI apps inherit the user's `PATH`, so it only gets the process value.

**Rationale**: `app/src-tauri` does nothing about this today, and a Finder launch gets a minimal
`PATH`. It runs only when a lookup misses, so a terminal launch never pays for it.

**Alternative**: Fixing `PATH` once in the app at startup. It would also help `workspace_shell`, but
it would run a login shell on every launch and leave the server build unhelped. A separate follow-up.

## R5. Finding the root

**Decision**: A data table (`languages.rs`), one row per language: id, extensions, LSP language id,
command, args, install hint and a `root: RootRule`. Walk up from the file, stop at the chat's `cwd`,
and fall back to `cwd`.

- `RootRule::Nearest(&[..])`: the nearest folder holding a marker. TypeScript: `tsconfig.json`,
  `jsconfig.json`, `package.json`. Python: `pyproject.toml`, `setup.py`, `requirements.txt`. Go:
  `go.work`, `go.mod`, with `go.work` winning at any level. C/C++: `compile_commands.json`,
  `.clangd`, `CMakeLists.txt`. Lua: `.luarc.json`. Java and Kotlin: `settings.gradle`,
  `settings.gradle.kts`, `pom.xml`, `build.gradle`, `build.gradle.kts`, with the settings files winning at
  any level, so a multi-module build roots at the top. C#: `*.sln`, `*.csproj`. Ruby: `Gemfile`. PHP:
  `composer.json`. Swift: `Package.swift`. Dart: `pubspec.yaml`. Elixir: `mix.exs`. Zig: `build.zig`. Bash:
  no marker, so the chat's folder. A marker may be a `*.ext` pattern, which C# needs. Markers listed as
  winning at any level are a `RootRule` of their own, shared by Go, Java and Kotlin.
- `RootRule::CargoWorkspace`: the nearest ancestor `Cargo.toml` containing a `[workspace]` table, or
  else the nearest `Cargo.toml`. It is found with a line scan, so no TOML dependency and no running
  `cargo`.

**Vizier's own tree** (checked against its files): `crates/*` resolves to the repo root, `app/src-tauri`
to `app/`, and `webui/`, `design/` and `landing/` each to themselves.

**Rationale**: Data, not a `match` per language (Principle II). A nested `.vizier/` path never reaches
this, because `path::resolve` refuses it.

## R6. Diagnostics after an edit (FR-027 to FR-030)

**Decision**: After a successful write, send `didChange` (or `didOpen`) and `didSave`. Then, for up to
3 s, take the first of:

1. a **pull** answer, if the server advertises `diagnosticProvider`;
2. a **push** carrying this version;
3. an unversioned **push** received after the sync.

Then report one of these statuses:

| `status` | Meaning |
|---|---|
| `current` | An answer reflects this edit, and the server reports no work in progress |
| `pending` | No answer in time, or the server still has work in progress (items so far may be listed) |
| `unavailable` | The server is missing or failed, and a message says which |

The field is absent when no language applies or the feature is off.

**Rationale**:

- clangd proves push works well (0.06 s). rust-analyzer proves pull has to be supported: it sent no push
  within 3 s of an edit, and answered a pull in 0.09-1.95 s.
- **Other files**: when a server reports `interFileDependencies`, also pull the other **open** documents
  under the root, at most 20, within the same 3 s. That caught the broken call site in the test.
  `read_file` therefore opens the file on a ready server. A call site in a file nobody has opened is not
  caught this way, which R17 weighs.
- **"Clean" must never be claimed while a check runs.** A pull can be clean while a slower build check
  has just started. The rule is generic, with no Rust special case: any open `$/progress` token after
  a 300 ms grace period forces `pending`. The agent then calls `lsp_diagnostics` later.
- Pushes for **other** files that arrive during the wait are included as `other_files`. That is the
  "broke a call site" case in Story 1.
- Diagnostics are cached per URI with the version and the time received, and never stored.

## R7. Keeping the server in step (FR-032, FR-033)

**Decision**:

- **Before any question**: read the file, compare its sha256 to what was sent, and `didChange` with
  full text if different. A change event with no range is valid for both sync kinds, so there are no
  incremental edits. Open documents are capped at 256 per server, closing the oldest. Files over
  2 MiB or not UTF-8 are refused with `file_unsupported`.
- **After `workspace_shell` or a branch switch**: in the background, walk each running root with
  `ignore` (the walker the tools already use), compare path and mtime against a snapshot taken when
  the server became ready, and send `workspace/didChangeWatchedFiles`. Above 5,000 changes, restart the
  server instead.

**Rationale**: A stateless re-sync per call covers any change to the asked-about file. Servers such as
gopls and pyright also rely on the client for changes to files that are not open. A `notify` watcher
would be a new dependency and a process-long resource, which this avoids.

## R8. Readiness and cold start (FR-035)

**Decision**: States are `starting`, `ready`, `failed`, `missing`. `ready` means `initialize` was
answered and no work-done progress has been open for 1 s (or none ever opened). A question against a
`starting` server returns `lsp_starting` at once, with the progress title and percentage. The error
codes `-32801` and `-32802` are retried once and then read as `lsp_starting`. Each request has its own
10 s limit, returning `lsp_timeout`.

**Rationale**: `tools.timeout` is 30 s in the default and wraps every nested call in
`agents/tools/python/bridge.rs`. Waiting inside it cannot work on a 10-60 s index. The measured `[]`
during startup means a question must be refused until ready, or the agent reads "no definition" as fact.

**Start trigger**: `read_file`, `edit_file`, `write_file` and any `lsp_*` call start the server in a
spawned task and never wait for it (FR-017). `list_files`, `grep` and `glob` do not, because a grep
over a tree would start every language at once.

## R9. Process lifetime (FR-036, FR-038)

**Decision**: Reuse `process_tree::isolate` and `kill`, `process::no_window` and `kill_on_drop`. Stop
is `shutdown`, then `exit`, a 2 s wait, then `kill`. Each server runs in its own task, so a hang
affects only its own calls. stderr goes into a capped 8 KiB ring for failure reasons.

**Abnormal exit** (`process_tree::bind_to_parent`, isolated per Principle IV):

- Linux: `PR_SET_PDEATHSIG` through `libc`.
- Windows: a job object with `KILL_ON_JOB_CLOSE`, using `windows-sys` (already in `Cargo.lock`).
- macOS: no kernel equivalent. Relies on servers exiting when stdin closes, measured at 0.2 s for
  rust-analyzer after its parent was killed. That needs the handshake to have finished, so send
  `initialized` straight after the `initialize` answer. Do not rely on `processId`, which it did not honour.

**Cleanup paths**: switch off, `service::projects::remove`, `delete_worktree`, agent delete
(`AgentHost` near `storage.delete_agent`), override change, idle sweep, and `VizierRuntime::shutdown`.

**Capacity**: 6 by default (`lsp.max_servers` in config). Evicts the least recently used server with no
request in flight. If none qualifies, the new one reads as `lsp_busy`.

## R10. Switch off during a turn

**Decision**: The turn context reads the switch once, like `mode`, but the host also keeps a `closed`
set of project ids, filled by switch-off and removal and cleared by switch-on. Host calls check it, so
a running turn gets `lsp_disabled` and a racing start cannot leave a server behind.

## R11. Safety (FR-011, FR-025, FR-039)

**Decision**:

- **Environment**: inherited minus `VIZIER_*`, `*_API_KEY`, `*_SECRET*`, `*_TOKEN`,
  `AWS_SECRET_ACCESS_KEY`, `AWS_SESSION_TOKEN` and `DATABASE_URL`. An allowlist would break toolchains
  (`RUSTUP_HOME`, `NVM_*`, `GOPATH`). An override's own env is added back last, so a person can restore
  a token deliberately.
- **Server requests**: `workspace/applyEdit` answers `applied: false`. `workspace/configuration` answers
  nulls. `client/registerCapability` and `window/workDoneProgress/create` are accepted. Anything else
  is `MethodNotFound`.
- **`initialize`** carries only the root and capabilities.
- **Results** never hold absolute host paths. Inside `cwd`: relative. Outside: `{outside_project: true,
  file: "<file name>"}`. Results under `.vizier/` are dropped.

## R12. Override storage

**Decision**: New table `project_lsp_override`. The `env` map is sealed whole through the existing
cipher with AAD `(table, record, "env")`. Plaintext to managers in the API, on the shell-env precedent;
other viewers never receive overrides. Stored per language, so it applies across all roots.

**Default**: column `lsp_enabled` is added with `DEFAULT 0`, so existing rows stay off, and
`open_project` inserts `1` for new ones. `shell_enabled` already defaults to on in both the column and
`open_project`, so issue #156's "off by default, exactly like the shell" was inaccurate.

## R13. Live status

**Decision**: A new `InboxEvent::ProjectLsp { agent_id, project_id, snapshot }` and `InboxFrame::Lsp`,
through the existing stream and `judge_with`. Throttled to 250 ms per project, like
`download_progress`. `GET …/lsp` loads the panel and recovers after `hello`/`resync`.

**Rationale**: Spec 051 removed timer polling, and the inbox stream is already open on both web and
desktop. The chat socket is per session, while servers are per project.

**Alternative**: Polling while the panel is open. The strip's count shows all the time, so it would
poll for the whole chat.

## R14. Compile-time flag

**Decision**: None. The constitution's flag rule names channels, addons, providers and gateways. This
adds no heavy dependency, the coding tools it joins are not flagged, and the runtime switch already
makes it optional. `cargo test -p vizier-core --no-default-features` must still pass.

## R15. Tool position input

**Decision**: `{path, line, symbol, occurrence?}`, with `line` 1-based and `occurrence` 1-based
defaulting to the first. The symbol is matched on word boundaries in that line, then converted with
the negotiated encoding (`position.rs`, pure and unit-tested on CJK, emoji and CRLF). No match is
`symbol_not_on_line`, returning the line's text.

**Rationale**: Agents miscount columns, and `grep` and `read_file` already give them lines.

## R17. rust-analyzer's `cargo check`

**Decision**: Applied as the default when tasks were generated, without an explicit answer from the
person; it is one field on one table row, so easy to change. Turn it off by default, with
`initializationOptions: {"checkOnSave": false}` on the Rust row of the language table. The agent relies on the fast analysis and runs `cargo check` through the
shell when it wants a full build.

| | **Check off (recommended)** | Check on, own target folder (`cargo.targetDir: true`) | Check on, shared target |
|---|---|---|---|
| Time per edit | 0.09-1.95 s, `current` | Pending until the check ends | Pending until the check ends |
| Agent's own `cargo` commands | Unaffected by checks | Unaffected | **Blocked** (measured) |
| Disk and CPU | No check builds | A second target folder per root, per worktree; a full cold check | A check on every edit |
| Borrow-check and lifetime errors | **Not reported** | Reported | Reported |
| Call site in an unopened file | **Not reported** | Reported | Reported |

**Rationale**: A turn makes several edits, so checks would stack, and each worktree would pay for a cold
build. The fast analysis covers type and argument errors immediately, and the shell covers the rest.

**Limit**: `"current"` for Rust means "the fast analysis agrees", so the prompt sentence says to run the
build before finishing when the shell is on. Startup still builds compile-time dependencies, so build
scripts and proc macros run, and a concurrent `cargo` can wait during that step.

## R16. Test fixture

**Decision**: `crates/vizier-core/examples/fake_lsp.rs`, a small scriptable stdio server. Tests find it
next to the test binary, and the quickstart sets it as a project override.

**Rationale**: No Python or Node on CI, Windows included, and it drives the real path end to end without
anything installed. Real-server tests skip with a visible message when the binary cannot run.
