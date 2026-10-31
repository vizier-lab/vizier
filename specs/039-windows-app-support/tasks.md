---

description: "Task list for Windows support in the desktop app"
---

# Tasks: Windows Support for the Desktop App

**Input**: Design documents from `/specs/039-windows-app-support/`

**Prerequisites**: plan.md, spec.md, research.md, data-model.md, contracts/release-artifacts.md, contracts/just-recipes.md, quickstart.md

**Tests**: No separate test-writing phase was requested. Existing test files are updated where behaviour they pin changes (`scripts/app-latest-json.test.mjs`, `webui/app/api/transport/transport.test.ts`), and the constitution's dummyplug end-to-end gate is a task in the final phase.

**Organization**: Grouped by user story. US1 = install and use on Windows (P1), US2 = release publishes the installer (P1), US3 = develop on Windows with `just` (P2).

## Format: `[ID] [P?] [Story] Description`

- **[P]**: can run in parallel (different files, no dependency on an incomplete task)
- **[Story]**: US1 / US2 / US3
- Commits: conventional format, `just commit "<message>"` (design â†’ webui â†’ vizier order when submodules change)
- Windows prerequisites (MSVC Build Tools, CMake, LLVM + `LIBCLANG_PATH`, Rust MSVC toolchain, Node 22, `tauri-cli ^2`) are assumed installed on the working machine

---

## Phase 1: Setup

**Purpose**: Find out what actually breaks, and set up the branches.

- [X] T001 Run `just checkout 039-windows-app-support` from the repo root so `design`, `webui`, `landing` and `dummyplug` get the same-named branch (constitution: matching submodule branches)
- [X] T002 In `webui/` and `design/` run `npm ci`, then `npm run build` in `webui/` so `webui/build/client/` exists (`crates/vizier-http/build.rs` panics without it)
- [X] T003 From `app/` run `cargo check --all-targets` on Windows and record every error or warning-as-error in a scratch note; this is the compile spike from research R3 (suspects: `whisper-rs`, `any-tts`, `ort`/`fastembed`, `sqlite-vec`)

---

## Phase 2: Foundational (blocks all user stories)

**Purpose**: The app must compile and its own tests must pass on Windows before anything else is meaningful.

**CRITICAL**: No user story work starts until this phase is done.

- [X] T004 Fix each compile error found by T003 in the file it names. Keep every fix as narrow as possible and inside the existing abstraction (no new crate, no new flag). If a dependency cannot build on `x86_64-pc-windows-msvc`, stop and record it in `specs/039-windows-app-support/research.md` under R3 with the exact error before choosing the R3 fallback (target-specific dependency on `vizier-core` without `media` in `app/src-tauri/Cargo.toml`)
- [X] T005 Run `cargo test` and `cargo clippy --all-targets -- -D warnings` in `app/` on Windows; fix failures that are Windows-specific (path separators in assertions, `cfg(unix)`-only helpers, temp-dir assumptions) in the failing test or module, including `app/src-tauri/tests/smoke.rs`
- [X] T006 Run `cargo test -p vizier-core --no-default-features` on Windows and fix Windows-only failures in `crates/vizier-core/` (tests that hard-code `/bin/sh`, such as `crates/vizier-core/src/encryption/storage_tests.rs`, `crates/vizier-core/src/encryption/migrate.rs`, `crates/vizier-core/src/encryption/fields.rs`, only need changing if they fail; leave Unix values in fixtures that merely store a string)

**Checkpoint**: `cargo check`, `cargo clippy` and `cargo test` are green in `app/` on Windows.

---

## Phase 3: User Story 1 - Install and use Vizier on Windows (Priority: P1) ðŸŽ¯ MVP

**Goal**: A Windows user installs the app and has the same working experience as on macOS/Linux, including agent shell, assets, deep links, tray, and data safety.

**Independent Test**: quickstart.md sections 2 and 3: install the locally built installer on a clean Windows 11 machine, create a dummyplug agent, run the shell tool steps, restart, confirm persistence.

### Implementation for User Story 1

- [X] T007 [P] [US1] In `app/src-tauri/tauri.conf.json` add `"nsis"` to `bundle.targets`, add `bundle.windows.nsis` with `"installMode": "currentUser"`, and add `plugins.updater.windows` with `"installMode": "passive"`; keep `createUpdaterArtifacts: true` and all existing targets and keys unchanged
- [X] T008 [P] [US1] In `crates/vizier-core/src/agents/shell/local/mod.rs` make `LocalShell::exec` pick its program per OS inside the one implementation: `sh -c <commands>` on Unix (unchanged), and on Windows `powershell.exe -NoProfile -NonInteractive -Command` with the command prefixed so the console output encoding is UTF-8 (`[Console]::OutputEncoding = [System.Text.Encoding]::UTF8;`). Keep workdir and env handling shared. Replace the hard `String::from_utf8(output.stdout)?` with `String::from_utf8_lossy` so non-UTF-8 output cannot fail the call. Add a unit test in the same file that runs `echo hello` (works under both `sh` and PowerShell) and asserts the output contains `hello`
- [X] T009 [US1] Find the tool that exposes the local shell to the model (search `crates/vizier-core/src/agents/tools/` and `crates/vizier-core/src/agents/shell/` for `ShellProvider` / `exec`) and the `SANDBOX.md` / system-prompt text; where it describes the shell, state the platform and shell (`PowerShell on Windows, sh elsewhere`) so the model writes matching syntax. Change only text that says or implies POSIX; do not add a tool or parameter
- [X] T010 [P] [US1] In `crates/vizier-core/src/agents/tools/skill/execute_skill_resource.rs` make the `"sh" | "bash"` arm return `Err(VizierError("shell scripts (.sh) are not supported on Windows; use a .py or .js resource"...))` under `cfg(windows)` (unchanged on Unix), and make the `"py" | "python"` arm use `python` as it does today (confirm that name resolves on Windows; use `py` only if the check in T016 shows `python` is missing from a stock install)
- [X] T011 [P] [US1] Audit every place a user- or model-chosen string becomes a file or directory name for Windows-invalid input (`<>:"|?*`, reserved names `con prn aux nul com1-9 lpt1-9`, trailing dot or space, `..`, length): `crates/vizier-core/src/storage/document.rs` (`LocalDocumentStore`), `crates/vizier-core/src/storage/memory_bundle.rs` (bundle import/export zip entry names), `crates/vizier-core/src/session_files/`, `crates/vizier-core/src/artifacts/`. Where a gap exists, reject it with a clear `VizierError` through the existing validation for that store (one shared helper only if two stores need the same rule); add a unit test per rule beside the code. Record the findings, including "no gap", in `specs/039-windows-app-support/research.md` under R7
- [X] T012 [P] [US1] In `webui/app/api/transport/tauri.ts` replace the hard-coded `ASSET = 'vizier-asset://localhost'` with a platform-aware base: on Windows WebView2 serves custom schemes as `http://vizier-asset.localhost`, elsewhere `vizier-asset://localhost` (use `convertFileSrc` from `@tauri-apps/api/core` or a platform check; do not import Tauri anywhere else, `boundary.test.ts` enforces that). Update the assertion in `webui/app/api/transport/transport.test.ts` (currently line ~79) so it covers both forms, then run `cd webui && npm test && npm run typecheck`. This is a `webui` submodule change: commit it on the `039-windows-app-support` branch in `webui/` first, then bump the pin in the workspace
- [X] T013 [P] [US1] In `app/src-tauri/src/shell.rs` verify (and fix only if broken on Windows) that a second launch with a `vizier://â€¦` argument reaches `handle_urls` through the single-instance callback at line ~253, and that `app.deep_link().register_all()` at line ~329 does not error on Windows; if the single-instance plugin needs its `deep-link` feature for argv forwarding, enable it in `app/src-tauri/Cargo.toml` and re-run `cargo fetch` so `app/Cargo.lock` updates
- [X] T014 [P] [US1] In `crates/vizier-core/src/encryption/key.rs` and `app/src-tauri/src/web_access.rs` confirm the non-Unix path (no `0600` mode) logs nothing alarming and still creates `{workspace}\.runtime\encryption.key` and `web-access.key`; add a one-line comment at each `cfg(unix)` site saying Windows relies on the profile directory's ACL. No behaviour change expected
- [X] T015 [US1] Build the installer locally with `cd app && cargo tauri build` and confirm `app/target/release/bundle/nsis/Vizier_<version>_x64-setup.exe` exists (depends on T004, T007)
- [X] T016 [US1] Manually walk quickstart.md sections 2 and 3 on a Windows 11 machine with the T015 installer (install without UAC, Start menu entry, first run, workspace at `%APPDATA%\rs.vizier.app\workspace`, second instance shows workspace in use, tray, `vizier://` link, dummyplug shell `echo hello` and `ls`, restart persistence, uninstall keeps `%APPDATA%\rs.vizier.app`). Fix defects found in the files above; record outcomes in `specs/039-windows-app-support/research.md` under a new "Verification log" heading

**Checkpoint**: US1 works from a locally built installer. The app is shippable by hand even if nothing else lands.

---

## Phase 4: User Story 2 - Release publishes a Windows installer automatically (Priority: P1)

**Goal**: Every release carries the Windows installer, its signature and checksum, and a `windows-x86_64` update entry; a Windows build failure blocks the release.

**Independent Test**: quickstart.md section 4: run `release.yml` with `dry_run: true`, confirm the `windows-x64` leg is green, the artifact contains the three files, and `latest.json` has the Windows entry; run `node --test scripts/*.test.mjs`.

### Implementation for User Story 2

- [X] T017 [P] [US2] In `scripts/app-latest-json.mjs` add `const win = \`Vizier_${version}_x64-setup.exe\`;` and the entry `'windows-x86_64': entry(win)` to `buildManifest`, following contracts/release-artifacts.md
- [X] T018 [US2] Update `scripts/app-latest-json.test.mjs` (create it if the existing test file has a different name; check `scripts/*.test.mjs`) so it asserts the five platform keys and the Windows URL shape `â€¦/releases/download/v<version>/Vizier_<version>_x64-setup.exe` with its signature, then run `just test-scripts` (depends on T017)
- [X] T019 [US2] In `.github/workflows/release.yml` `build-app` matrix add `{ name: windows-x64, os: windows-latest, args: '', rust_targets: '' }`; before the Checkout step add a step (Windows only) running `git config --global core.longpaths true`; in the Build app step set `LIBCLANG_PATH: C:\Program Files\LLVM\bin` for the Windows leg only (via a matrix field or `if`-guarded env), leaving the Linux/macOS legs exactly as they are
- [X] T020 [US2] In the same file extend the `Verify architectures` step with a `Windows` branch (before the `else` that assumes Linux): assert `app/target/release/bundle/nsis/*-setup.exe` exists and that `file` reports `PE32+ executable` and `x86-64`; extend `Collect artifacts` with a `Windows` branch copying that exe to `app-dist/Vizier_${VERSION}_x64-setup.exe` and its `.sig` to `app-dist/Vizier_${VERSION}_x64-setup.exe.sig` (same `[ -f â€¦ ] && cp â€¦ || true` pattern as the other platforms); use `sha256sum` instead of `shasum -a 256` for the `.exe` and include `*.exe` in the checksum loop (depends on T019)
- [X] T021 [US2] In `create-release` of the same file: add `Vizier_${{ needs.prepare-release.outputs.version }}_x64-setup.exe.sig` to the `Check update signatures` list, add a Windows line (`Windows 10/11 x64: Vizier_<version>_x64-setup.exe`) to the release body, and add a sentence that the Windows installer is not code-signed with the SmartScreen steps (**More info â†’ Run anyway**). Do not touch the `build` (server) job or its matrix (FR-002)
- [X] T022 [P] [US2] In `.github/workflows/ci.yml` turn the `app` job into a matrix over `ubuntu-22.04` and `windows-latest` (`fail-fast: false`, `runs-on: ${{ matrix.os }}`), guard `Install Linux dependencies` with `runner.os == 'Linux'` in addition to the existing condition, add the `core.longpaths` step and `LIBCLANG_PATH` for Windows, and set `shell: bash` on the `Build the WebUI` and `Clippy and test` steps so the existing scripts run unchanged on both OSes
- [X] T023 [P] [US2] In `docs/src/getting-started/desktop-app.md` add the Windows row (`Windows 10/11 x64 | Vizier_<version>_x64-setup.exe`), a **Windows** paragraph (unsigned: SmartScreen **More info â†’ Run anyway**; installs for the current user without admin; WebView2 is installed automatically on Windows 10; ARM64 PCs run it under emulation on Windows 11), the Windows workspace path `%APPDATA%\rs.vizier.app\workspace` in "Where your data lives", and that closing/uninstalling keeps the workspace
- [X] T024 [P] [US2] In `README.md` line ~41 (the Desktop app paragraph) add the Windows installer (`Vizier_<version>_x64-setup.exe`, unsigned for now, link to the guide)
- [X] T025 [US2] Trigger `release.yml` with `dry_run: true` from the branch and confirm: `Build app - windows-x64` is green, artifact `vizier-app-windows-x64` has `Vizier_<version>_x64-setup.exe`, `.sig`, `.sha256`, `release-assets/latest.json` has `windows-x86_64`, and the macOS, Linux and server legs are unchanged. Record the run URL and any fixes in `specs/039-windows-app-support/research.md` "Verification log" (depends on T019â€“T022; needs a push, so ask the user first)

**Checkpoint**: A release produces the Windows installer and manifest entry; CI compiles the app on Windows for every app-related PR.

---

## Phase 5: User Story 3 - Develop on Windows with `just` (Priority: P2)

**Goal**: A contributor on Windows uses `just install-app`, `just app-dev`, `just app-build`, `just app-test` from PowerShell, with system prerequisites documented in the README and docs rather than scripted.

**Independent Test**: quickstart.md section 1 on Windows; the same four recipes still behave as before on Linux/macOS.

### Implementation for User Story 3

- [X] T026 [US3] In `justfile` add `set windows-shell := ["powershell.exe", "-NoLogo", "-NoProfile", "-Command"]` at the top; split `install-app` into a `[unix]` recipe (existing body, untouched) and a `[windows]` recipe that installs `tauri-cli ^2` if `cargo tauri --version` fails, runs `cargo fetch` in `app/`, `git submodule update --init`, and `npm ci` in `design/` and `webui/` (one command per line; no `&&`, PowerShell 5.1 has none); it checks and installs no system software
- [X] T027 [US3] In `justfile` change `app-dev`, `app-build` and `app-test` to use `[working-directory: 'app']` (and for `app-build` run the WebUI build first with `npm --prefix ../webui run build`, then `cargo tauri build`) so they work in both shells; keep the Linux/macOS behaviour identical (depends on T026, same file)
- [X] T028 [US3] Run `just --list` and `just install-app`, `just app-test`, `just app-build` on Windows to confirm the recipes work from PowerShell, and confirm `just --dry-run app-dev app-test` parses on Linux if a Linux checkout is available; note the minimum `just` version that supports `[working-directory]` and the `[unix]`/`[windows]` attributes (check `just --version` and the just changelog)
- [X] T029 [P] [US3] In `README.md` Development section add **Windows** subsection: system prerequisites with commands â€” `winget install --id Microsoft.VisualStudio.2022.BuildTools --override "--passive --wait --add Microsoft.VisualStudio.Workload.VCTools --includeRecommended"`, `winget install --id Kitware.CMake`, `winget install --id LLVM.LLVM`, set `LIBCLANG_PATH` to `C:\Program Files\LLVM\bin`, Rust via `rustup` with the MSVC host, Node.js 22+, `just` (with the minimum version from T028), WebView2 (preinstalled on Windows 11) â€” then `just install-app`, `just app-dev`, `just app-build`, and the statement that only the app recipes are supported on Windows (the server is Linux/macOS/Docker)
- [X] T030 [P] [US3] Add the same Windows development section (prerequisites, recipes, troubleshooting: missing `libclang`, long paths, first build time) to `docs/src/getting-started/contributing.md`, and link it from `docs/src/getting-started/desktop-app.md`

**Checkpoint**: A new contributor can follow only the README and reach a running dev app on Windows.

---

## Phase 6: Polish & Cross-Cutting Concerns

- [X] T031 [P] Add a short "Windows" note to the Desktop app section of `CLAUDE.md` (NSIS installer, `windows-x86_64` update key, PowerShell local shell, asset URL scheme on Windows, app-only support); leave the Spec Kit marker block untouched
- [X] T032 Run `cargo clippy --workspace` and `cargo test --workspace` on Linux (CI) and Windows for the changed crates, plus `cd webui && npm run typecheck`, and confirm no Linux/macOS behaviour changed
- [X] T033 End-to-end check with a `dummyplug` agent on the running Windows app (constitution e2e gate): walk the table in quickstart.md section 3 and confirm each expected reply
- [X] T034 Manual upgrade check (quickstart.md section 5): install the previous Windows build, create data, install the new build over it (and, once a real release exists, via **Check for updates**), confirm agents, conversations and memory survive; then uninstall and confirm `%APPDATA%\rs.vizier.app` remains
- [X] T035 Run the whole quickstart.md top to bottom and tick off each step; file follow-ups (Authenticode signing, Windows on ARM, `tauri-cli` prebuilt install to speed the Windows leg) as issues or add them to the existing signing issue #132

---

## Dependencies & Execution Order

### Phase dependencies

- **Phase 1 (Setup)** â†’ **Phase 2 (Foundational)** â†’ user stories â†’ **Phase 6 (Polish)**
- T003 (compile spike) decides the content of T004; if it finds nothing, T004 closes immediately.
- Phase 2 blocks everything: nothing can be verified until the app builds and passes its tests on Windows.

### User story dependencies

- **US1 (P1)**: starts after Phase 2. No dependency on US2 or US3.
- **US2 (P1)**: needs only that the app builds on Windows (Phase 2); T017/T018/T023/T024 are independent of US1. T020 assumes the NSIS target from T007 (US1). T025 needs T007 and the Phase 2 fixes pushed.
- **US3 (P2)**: independent of US1 and US2 except that T028 and T029 reference the built installer output of T007.

### Within each story

- T008 â†’ T009 (the description text follows the behaviour), T012 is a submodule commit that must land before the workspace pin bump.
- T019 â†’ T020 â†’ T021 (same file `release.yml`); T017 â†’ T018; T026 â†’ T027 â†’ T028 (same `justfile`).

### Parallel opportunities

- US1: T007, T008, T010, T011, T012, T013, T014 touch different files and can run together.
- US2: T017, T022, T023, T024 can run together; T019â€“T021 are sequential.
- US3: T029 and T030 (docs) can run together and alongside T026â€“T028.
- After Phase 2, US1, US2 and US3 can be done by different people in parallel.

## Parallel Example: User Story 1

```text
Task: "tauri.conf.json NSIS target and updater installMode (T007)"
Task: "LocalShell PowerShell on Windows in crates/vizier-core/src/agents/shell/local/mod.rs (T008)"
Task: "execute_skill_resource Windows .sh error in crates/vizier-core/src/agents/tools/skill/execute_skill_resource.rs (T010)"
Task: "Path/name audit across document, artifact, session-file stores (T011)"
Task: "WebUI asset URL on Windows in webui/app/api/transport/tauri.ts (T012)"
```

## Implementation Strategy

### MVP first (User Story 1 only)

1. Phase 1 and Phase 2 (the app builds, clippy and tests pass on Windows)
2. Phase 3 (US1): local installer works, agent shell works, assets and deep links work
3. **Stop and validate** with quickstart sections 2 and 3: this is a hand-distributable Windows app

### Incremental delivery

1. MVP above
2. US2: releases ship the installer and update manifest entry (the first dry run is the proof)
3. US3: contributors on Windows can follow the README
4. Polish: CLAUDE.md note, full quickstart, upgrade and e2e checks

## Notes

- **Unverified at merge**: T016, T025, T032, T033, T034 and T035 were checked off at the maintainer's request before they were run (install, release dry run, workspace-wide and Linux checks, dummyplug end-to-end, upgrade, full quickstart). They are to be tested after the merge; reopen any that fail.

- The only submodule change is `webui` (T012); follow design â†’ webui â†’ vizier commit order and `just commit`
- `crates/vizier-core` changes (T008, T010, T011) are agent-observable and covered by the e2e gate (T033)
- Server edition, `Cross.toml`, Docker and the `build` matrix in `release.yml` must not change (FR-002, SC-004)
- Pushing the branch and triggering the dry run (T025) are outward-facing; confirm with the user before doing either
