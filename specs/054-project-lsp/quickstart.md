# Quickstart: Project Language Intelligence

**Feature**: `054-project-lsp`

Validation guide. Dummyplug steps are the constitution's end-to-end gate, using the `fake_lsp` example
as a project override, so nothing needs installing. Real-server steps are marked. Protocol:
`specs/008-dummyplug-provider/contracts/dummyplug-protocol.md`.

## 0. Setup and automated checks

```sh
cargo clippy --workspace
cargo test --workspace
cargo test -p vizier-core --no-default-features
scripts/048-agent-state-read-check.sh
cd design && npm test && cd ..
cd webui && npm run typecheck && npm test && cd ..
```

Build the fixture and create a project folder:

```sh
cargo build -p vizier-core --example fake_lsp
rm -rf /tmp/vz-054 && mkdir -p /tmp/vz-054/src /tmp/vz-054/app/src-tauri && cd /tmp/vz-054
git init -q -b master
printf '[workspace]\nmembers=["crates/*"]\n' > Cargo.toml
printf '[workspace]\nmembers=["src-tauri"]\n' > app/Cargo.toml
printf 'pub fn greet() -> u32 { 1 }\n' > src/lib.rs
printf 'fn main() {}\n' > app/src-tauri/main.rs
```

Run Vizier with a `dummyplug` agent and open `/tmp/vz-054` as a project. Put a chat in Build mode.

## 1. The default (US2, FR-001)

- A new project reports `lsp_enabled: true`, and `ps` shows no `fake_lsp` or language server yet.
- A project opened before the upgrade reports `lsp_enabled: false`.
- `tools`, then `list_tool_functions` for `coding`, lists the five `lsp_*` tools in Plan and Build.
- `## Project state` (`context`) reads `Language servers: on.`

## 2. Missing server (US2, FR-012)

With nothing installed for Rust and no override:

```json
{"tool":"execute_python","arguments":{"code":"print(lsp_definition(path='src/lib.rs', line=1, symbol='greet'))"}}
```

Expected: `lsp_unavailable` naming `rust-analyzer` and the `rustup` hint. `GET …/lsp` shows `missing`
with the same hint. Nothing was installed.

**Proxy case** (a `rustup` proxy with no component, or any command that exits before answering): also
`lsp_unavailable`, not `lsp_failed`.

## 3. Override and diagnostics after an edit (US5, US1, FR-027 to FR-029)

```sh
curl -X PUT …/lsp/overrides/rust -d '{"command":"<target>/debug/examples/fake_lsp","args":["--script","diagnostics"]}'
```

Read the file, then introduce the fixture's trigger text:

```json
{"tool":"execute_python","arguments":{"code":"print(read_file(path='src/lib.rs'))"}}
```

```json
{"tool":"execute_python","arguments":{"code":"print(edit_file(path='src/lib.rs', old_text='1', new_text='ERROR'))"}}
```

Expected: `diagnostics.status: current`, `errors: 1`, and a location at line 1. `other_files` lists the
file the script reports. Edit it back: `current`, with `errors: 0`.

Repeat with `--script slow`: the edit succeeds in under 4 s with `status: pending`, and no claim of clean.
Repeat with `--script progress`: an immediate pull is clean, but `status` is `pending`.

## 4. Semantic questions in Plan mode (US3, FR-020 to FR-024)

Switch the chat to Plan. Run `lsp_definition`, `lsp_references`, `lsp_hover` and `lsp_symbols` with
`--script answers`. Expected:

- Relative paths and 1-based lines; no absolute path anywhere in the output.
- An outside location reads `outside_project: true` with only a file name.
- A symbol not on the line returns `symbol_not_on_line`.
- `write_file` is still refused in Plan.
- A 60-reference answer is cut to 50 with `truncated: true` and a `hint`.

## 5. Roots (US4, FR-014 to FR-019)

Read `src/lib.rs`, then `app/src-tauri/main.rs`. Expected: `GET …/lsp` lists two Rust servers, with roots
`.` and `app`. A second chat on the project reuses them (still two). Create a worktree and read a file
there: a third entry under `.vizier/…`. `grep` and `list_files` start nothing.

Lower `lsp.max_servers` to 1: the second file evicts the first, and using the first again restarts it.

## 6. Lifecycle and failure (FR-035 to FR-038, SC-006, SC-007)

- **`--script hang`**: a call returns `lsp_timeout` within 10 s. Another chat replies normally throughout.
- **`--script slow-start`**: an immediate call returns `lsp_starting` in under 1 s.
- **`--script crash`**: three restarts, then `failed` with a reason. **Restart** clears it.
- **Stop paths**: for each, `ps` shows no fixture process within 10 s: switch off, project removed,
  worktree deleted, agent deleted, `vizier shutdown`.
- **`kill -9` on Vizier**: the fixture exits on stdin close.
- **Mid-turn switch-off**: later calls in that turn return `tool_not_offered` or `lsp_disabled`, and edits
  still succeed without `diagnostics`.

## 7. Safety (FR-011, FR-025, FR-039)

- **`--script dump-env`**: with `VIZIER_JWT_SECRET` and `FOO_API_KEY` set in Vizier's environment, the
  fixture's report contains neither. An override's own `env` is present.
- **`--script apply-edit`**: the file on disk is unchanged, and the fixture records `applied: false`.
- A config file in the project cannot change the command; only the API can.
- As a non-manager, `PUT …/overrides/rust` and `PATCH lsp_enabled` are refused, and `GET …/lsp` holds no
  `overrides`.

## 8. Positions (FR-034, SC-008)

Unit tests in `position.rs` cover UTF-8 and UTF-16 servers, with CJK, emoji and CRLF lines.
`--script utf16` proves the same end to end.

## 9. UI (desktop and web)

Compare with `mockups/lsp-status-popup.html`, in both themes and at phone width:

- The strip shows `<> N` and the dot in project chats only.
- The status updates with no reload while a server starts (inbox `lsp` frame).
- Each mockup state appears: Working, Needs attention, Override, Nothing yet, Off, View only.
- Desktop opened from Finder finds a server in `~/.cargo/bin` (FR-013).

## 10. Real servers (needs them installed)

- **clangd**: `edit_file` introducing an undeclared identifier gives `current` with one error.
- **rust-analyzer**: same on a three-file crate, plus a signature change that breaks a call site in
  another file. Record how long the cargo-check errors take to appear, and confirm `pending` until then.
- `crates/vizier-core/tests/lsp_real.rs` runs these when the binary answers, and prints a visible skip
  otherwise.

## 11. Live provider (real model output)

**Needs a live provider.** Ask an agent how a function is used in a real repository. Expected: it reaches
for `lsp_references` before `grep`. Ask for a signature change: it reads `diagnostics` and fixes the
call site in the same turn.
