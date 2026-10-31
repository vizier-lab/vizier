# Research: Windows Support for the Desktop App

No `NEEDS CLARIFICATION` remained in the spec; the items below are the design decisions and the risks the code audit surfaced.

## R1. Installer format: NSIS, per-user

- **Decision**: add `"nsis"` to `bundle.targets`, with `bundle.windows.nsis.installMode = "currentUser"`. The updater artifact is the `*-setup.exe` and its `.sig` (Tauri 2 updater installs NSIS bundles directly, `windows.installMode = "passive"`).
- **Rationale**: per-user install needs no admin (FR-004); NSIS can cross-handle upgrade-in-place and keeps user data outside the install dir (FR-005). **MSI is unsuitable**: WiX/MSI versions reject non-numeric pre-release identifiers, and this project releases versions like `0.14.0-rc.1`.
- **Alternatives**: MSI/WiX (version scheme, per-machine default, extra tooling); portable `.exe` zip (no Start menu, no uninstall, no updater story); `winget`/MSIX (later, needs signing).

## R2. Release asset naming and update manifest

- **Decision**: ship `Vizier_<version>_x64-setup.exe` (Tauri's NSIS name, kept as is), its `.sig`, and `.sha256`; add `windows-x86_64` to `latest.json` pointing at the setup exe with its signature.
- **Rationale**: existing names are `Vizier_<version>_<arch>.<ext>`; `x64` is the Tauri/Windows convention. The "Flatten artifacts" step already globs `*.exe`/`*.sig`/`*.sha256`, so the release job needs no glob change.
- **Alternatives**: rename to `amd64` for symmetry with AppImage — rejected, it would fight Tauri's output and Windows users' expectations.

## R3. Build runner and toolchain on Windows

- **Decision**: `windows-latest` (x64) leg in the `build-app` matrix and in CI's `app` job. Rely on the image's MSVC, CMake and LLVM; set `LIBCLANG_PATH` explicitly if bindgen cannot find libclang. Enable `git config --global core.longpaths true` before checkout (deep `webui`/`node_modules` and Rust `target` paths).
- **Rationale**: `whisper-rs` (via `vizier-core` `media`) needs CMake + libclang; both are preinstalled on GitHub's Windows images. `rusqlite` `bundled`, `sqlite-vec` and aws-lc compile with MSVC. `fastembed` 4.9.1 → `ort` ships prebuilt Windows binaries (the macOS-x86 pin note in `Cargo.toml` is unrelated).
- **Risk**: first-build failure in `whisper-rs`/`any-tts`/`ort`. **Spike first** (task 1): `cargo check -p vizier-app` on Windows. Fallback, only if proven necessary and only for the app on Windows: depend on `vizier-core` without `media` via a target-specific dependency and say so in docs (FR-012 graceful degradation). Not adopted pre-emptively (Principle I).
- **Local machine note**: the prerequisites are installed manually per the docs (R9); they are not handled by `just`.

## R4. Local shell on Windows

- **Finding**: `agents/shell/local/mod.rs` runs `Command::new("sh").arg("-c")`. Windows has no `sh`.
- **Decision**: inside `LocalShell` (the single `ShellProvider` impl for local), choose the program per OS: `sh -c` on Unix, `powershell.exe -NoProfile -NonInteractive -Command` on Windows with the output encoding forced to UTF-8; the working directory and env handling stay shared. Output decoding stays UTF-8 with lossy fallback on Windows (OEM code pages are not UTF-8) — replace the current hard `String::from_utf8(...)?` with a lossy conversion on Windows only if the dummyplug check shows a failure.
- **Rationale**: smallest change satisfying FR-012; Docker shell (via `bollard`) remains available on Windows for people who want POSIX semantics.
- **Open check (task)**: any system-prompt/tool text that tells the model the shell is POSIX should name the platform so the agent writes `dir` not `ls`. Verify in `agents/tools` descriptions and `SANDBOX.md`; change only if it says "sh"/"bash".
- **Why PowerShell, not `cmd`**: models write POSIX-style commands, and PowerShell aliases the common ones (`ls`, `cat`, `rm`, `cp`, `mv`, `echo`, `pwd`, `mkdir`), which `cmd` lacks; it ships with every Windows 10/11, has better errors and UTF-8 handling. Cost: ~200–400 ms slower start per call and quoting quirks.
- **Alternatives**: `cmd /C` (faster start, but POSIX-style commands fail); require Git Bash/WSL (new external dependency, violates the zero-setup promise).
- **Tool text**: the shell tool's description must name the platform and shell (PowerShell on Windows) so the model uses matching syntax.

## R5. Skill resource scripts

- **Finding**: `tools/skill/execute_skill_resource.rs` maps `sh`/`bash` extensions to `sh <script>`.
- **Decision**: on Windows keep `.py`/`.js` etc. as today (interpreter looked up on `PATH`), and return a normal tool error naming the unsupported interpreter for `.sh`/`.bash`. Check the Python interpreter name (`python3` vs `python`/`py`) used there.
- **Rationale**: graceful degradation with a clear message (FR-012).

## R6. Workspace lock and key files

- **Finding**: `WorkspaceLock` uses `std::fs::File::try_lock` (works on Windows via `LockFileEx`; the `.runtime/vizier.lock` file is only locked, never read, so mandatory locking is harmless). `encryption/key.rs` and `web_access.rs` set `0600` under `cfg(unix)`; `key.rs` already has a `cfg(not(unix))` branch.
- **Decision**: no code change expected. Files under `%APPDATA%` inherit user-only ACLs; document that, and confirm the non-unix branch of `key.rs` logs nothing alarming. A workspace on a synced/network folder gets the same advice as on other OSes.
- **Check**: `tests/smoke.rs` and the lock behaviour (two instances → `locked` state) on Windows.

## R7. Paths and names in document/artifact/upload stores

- **Risk**: memory concepts are addressed `(agent_id, bundle, path)` with `/` separators and mapped to files under `agents/{id}/memory/`; artifacts and uploads use ids. Windows forbids `<>:"|?*`, reserved device names (`con`, `nul`, `aux`, `com1`…), trailing dots/spaces, is case-insensitive (macOS already is), and caps paths near 260 chars unless long paths are enabled.
- **Decision**: audit `LocalDocumentStore`, bundle import/export (`.zip` entry names), `uploads/`, `artifacts/` and `scratch` code for any place a user- or model-chosen string becomes a filename. Add one shared validation (reject with a clear `VizierError`) only where a gap is found, in the existing store, not per call site.
- **Mitigation for length**: the app manifest can opt into long paths (`longPathAware`), and `tauri` apps on Windows 10 1607+ honour it when the OS setting is on; document rather than depend on it.

## R8. WebView2, protocols, single-instance, deep links

- **Finding**: the CSP in `tauri.conf.json` already allows `http://ipc.localhost` and `http://vizier-asset.localhost` — the origins WebView2 uses for custom schemes — so the 038 design anticipated Windows.
- **Decisions / checks**:
  - `webui/app/api/transport/tauri.ts` must build asset URLs with Tauri's platform-aware helper (on Windows `vizier-asset://x` is `http://vizier-asset.localhost/x`). Verify for streamed/ranged media.
  - `vizier://` deep links: the NSIS installer registers the scheme; for a second launch from a link, `tauri-plugin-single-instance` needs its `deep-link` feature enabled so arguments reach the running instance. Verify and enable if missing.
  - WebView2: Windows 11 has it; NSIS `webviewInstallMode` default (`downloadBootstrapper`) covers Windows 10. Offline install is out of scope.
  - Tray, notifications, dialog, opener, process plugins are cross-platform; verify close-to-tray and toast click-through.

## R9. `just` on Windows

- **Findings**: `justfile` recipes are bash-only (`#!/usr/bin/env bash`, `echo -e`, `cd x && y`). On this machine `bash` resolves to the WSL stub in `WindowsApps`, not Git Bash, so the default shell cannot be relied on. Windows PowerShell 5.1 has no `&&`.
- **Decision**: add `set windows-shell := ["powershell.exe", "-NoLogo", "-NoProfile", "-Command"]`; give the four app recipes `[unix]`/`[windows]` variants where the body differs and use `[working-directory: 'app']` instead of `cd app && …`; implement Windows `install-app` inline in PowerShell: install `tauri-cli ^2` if absent, `cargo fetch` in `app/`, initialise submodules and run `npm ci` in `design/` and `webui/` — because `just install` itself is bash/`cargo-watch`-oriented and out of scope. **System prerequisites are not checked or installed by `just`** (decision): they are documented in the README and `docs/src/getting-started/desktop-app.md`/`contributing.md` with `winget` commands (MSVC Build Tools with the C++ workload, CMake, LLVM + `LIBCLANG_PATH`, Node 22, WebView2, Rust MSVC toolchain).
- **Rationale**: keeps Linux/macOS recipes byte-for-byte equivalent (FR-009, SC-004 analogue for dev); no WSL needed.
- **Alternatives**: a `scripts/install-app.ps1` prerequisite checker / opt-in `winget` installer (rejected for now: more to maintain, docs suffice); a cross-platform Node script; requiring Git Bash (extra assumption). The non-app recipes are deliberately left Unix-only.
- **Note**: confirm the minimum `just` version for `[working-directory]` and `[windows]`/`[unix]` attributes and state it in docs.

## R10. Release workflow changes

- **Decisions** (all in the `build-app` job and `create-release`):
  - Matrix entry `name: windows-x64`, `os: windows-latest`, `args: ''`, `rust_targets: ''`.
  - Skip "Install Linux dependencies" (already `runner.os == 'Linux'`); `cargo install tauri-cli` stays (slow but cached by `rust-cache`; swap for a prebuilt install later if too slow).
  - "Verify architectures": a Windows branch asserts the NSIS setup exe exists and is a PE32+ x86-64 image; "Collect artifacts": copy `bundle/nsis/*-setup.exe` and `.sig` to `Vizier_${VERSION}_x64-setup.exe(.sig)`; checksums via `sha256sum` on Windows (Git Bash) rather than `shasum`.
  - "Check update signatures": add the Windows `.sig` to the required list for real releases.
  - `latest.json`: add `windows-x86_64`.
  - Release body: list the Windows download and the SmartScreen note (More info → Run anyway).
  - `fail-fast: false` is already set, but `create-release` `needs: build-app`, so a failed Windows leg already blocks publishing (FR-008) — no change needed there.
- **CI**: `ci.yml` `app` job becomes a matrix of `ubuntu-22.04` and `windows-latest`; Linux-specific apt step guarded by `runner.os`.

## R11. Unsigned installer

- **Decision**: ship unsigned, say so in release notes and docs (FR-007), reuse the macOS #132 follow-up for code signing. Tauri updater signatures (minisign) are separate from Authenticode and still apply.

## R12. Windows on ARM

- **Decision**: out of scope. State "x64 only; ARM64 runs it under emulation on Windows 11" in docs. A `windows-11-arm` runner leg is a possible later follow-up.

## R13. Workspace location

- **Decision**: keep Tauri's `app_data_dir()` default (`%APPDATA%\rs.vizier.app\workspace`, Roaming). Document it. Roaming profiles syncing a live SQLite DB are a corner case; the existing "open another workspace" setting is the escape hatch.
