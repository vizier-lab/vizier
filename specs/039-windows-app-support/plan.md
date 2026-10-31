# Implementation Plan: Windows Support for the Desktop App

**Branch**: `039-windows-app-support` | **Date**: 2026-10-31 | **Spec**: [spec.md](spec.md)

**Input**: Feature specification from `/specs/039-windows-app-support/spec.md`

## Summary

Make the Tauri desktop app (`app/`, spec 038) build, run, install, update and be developed on Windows 10/11 x64, without adding Windows to the server edition. Four pieces of work:

1. **Engine/app portability.** Compile `vizier-app` (with `vizier-core/full`) for `x86_64-pc-windows-msvc` and fix what breaks. The audit found little: the encryption key code already has a `cfg(not(unix))` branch, `File::try_lock` for the workspace lock is cross-platform, and the CSP already lists the `http://ipc.localhost` / `http://vizier-asset.localhost` origins WebView2 uses. The real gaps are the **local shell** (`sh -c`), **skill resource scripts** (`sh`/`bash`), and a path/name audit of the document, artifact and upload stores.
2. **Installer.** Add a per-user NSIS installer to the Tauri bundle targets, with a signed updater payload.
3. **Release workflow.** Add a `windows-x64` leg to `release.yml`'s `build-app` matrix, extend the signature check, `latest.json`, checksums and release notes; add a Windows compile leg to `ci.yml`'s `app` job so regressions are caught before a release.
4. **Developer experience.** Make `install-app`, `app-dev`, `app-build`, `app-test` work from native Windows (PowerShell, no WSL), plus docs.

## Technical Context

**Language/Version**: Rust 2024 (stable toolchain, as in CI); WebUI TypeScript/React (unchanged); Node.js 22 for builds; PowerShell 5.1+ (the `just` Windows shell and the agent's local shell on Windows).

**Primary Dependencies**: existing only — Tauri 2 (+ `tauri-plugin-updater`, `-single-instance`, `-deep-link`, `-notification`, `-dialog`, `-opener`, `-process`), `vizier-core` `full` feature group (`whisper-rs`, `any-tts`, `bollard`, `fastembed`/`ort`, `rusqlite` bundled, `sqlite-vec`, `ring`/aws-lc). **No new crates.** New build-time needs on Windows: MSVC build tools, CMake, LLVM/libclang (for `whisper-rs` bindgen), WebView2 runtime, NSIS (fetched by `tauri-cli`).

**Storage**: unchanged (embedded SQLite + markdown documents on disk under the workspace). Default workspace is Tauri's `app_data_dir()` → `%APPDATA%\rs.vizier.app\workspace`.

**Testing**: `cargo test` in `app/` and `cargo test --workspace` already exist; add the `app` CI job on `windows-latest`. `node --test scripts/*.test.mjs` covers `app-latest-json.mjs`. End-to-end: dummyplug agent on the built Windows app (see [quickstart.md](quickstart.md)); installer/upgrade/auto-update checks are manual on a Windows 11 machine (no automated runner for an installed GUI).

**Target Platform**: Windows 10 (1809+) and 11, x86_64, WebView2. Linux/macOS app and the server are unchanged.

**Project Type**: desktop-app (Tauri shell hosting `VizierRuntime`) + CI/CD + developer tooling.

**Performance Goals**: none new. Windows CI app leg should finish inside the existing release timeout budget (expect ~25–40 min cold; cached with `rust-cache`).

**Constraints**: server edition, its tarballs, `Cross.toml`, Docker images and the `build` matrix untouched (FR-002, SC-004). Unsigned installer (tracked with #132). No admin rights required to install. Existing user data survives upgrade and uninstall.

**Scale/Scope**: ~1 new CI leg ×2 workflows, 1 manifest key, 1 bundle target, ~4 `just` recipes, 2–3 small `cfg` sites in `vizier-core`, 2 doc pages.

## Constitution Check

*GATE: Must pass before Phase 0 research. Re-check after Phase 1 design.*

| Principle / gate | Status | Notes |
|---|---|---|
| I. Lean by default | PASS | No new dependency or config flag. One small per-OS choice inside `LocalShell`, no new abstraction. |
| II. Trait-based extensibility | PASS | Per-OS shell behaviour lives inside the one `LocalShell` `ShellProvider` impl (a single `cfg` there), not scattered in call sites. Skill-resource interpreter selection stays in its one function. |
| III. Self-contained runtime | PASS (note) | The app runs with zero external services. The installer needs the WebView2 runtime; Windows 11 ships it, and on Windows 10 the NSIS installer fetches Evergreen's bootstrapper at install time. That is install-time, not runtime, and is documented. |
| IV. Portability by default | PASS — this feature *is* the principle | Platform behaviour goes through existing abstractions (`dirs`/Tauri path API, `ShellProvider`, `DocumentStore`). The `Cross.toml` server targets are not touched. |
| V. Unified errors & observability | PASS | New Windows-only failure messages use `VizierError`/`tracing`.  |
| Conventional commits / PR titles | Applies | `feat(app): windows support`, `ci: …`, `chore(just): …`, `doc: …`. |
| Matching submodule branches via `just` | Applies | Touches the workspace only (`app/`, `crates/`, `.github/`, `justfile`, `scripts/`, `docs/`). `just checkout 039-windows-app-support` is still run so submodule branches exist; no submodule commits expected. |
| Manual verification of build-pipeline changes | Applies | The workflow is exercised with `release.yml` `dry_run` and the installer is installed on a real Windows machine. |
| dummyplug e2e for agent-observable changes | Applies | The local shell is agent-observable → quickstart scripts a dummyplug step for it on Windows. |

No violations; Complexity Tracking is empty.

**Post-design re-check**: unchanged — the design added no crate, trait or flag.

## Project Structure

### Documentation (this feature)

```text
specs/039-windows-app-support/
├── plan.md
├── research.md
├── data-model.md
├── quickstart.md
├── contracts/
│   ├── release-artifacts.md
│   └── just-recipes.md
└── tasks.md             # /speckit-tasks — not created here
```

### Source Code (repository root)

```text
app/src-tauri/
├── tauri.conf.json                 # + "nsis" target, bundle.windows.nsis (currentUser), updater windows installMode
├── Cargo.toml                      # single-instance "deep-link" feature if the audit needs it
└── src/{shell.rs,web_access.rs}    # Windows audit: deep-link/single-instance args, key file note

crates/vizier-core/src/
├── agents/shell/local/mod.rs       # per-OS shell inside LocalShell: sh -c / PowerShell (UTF-8)
├── agents/tools/skill/execute_skill_resource.rs   # .sh on Windows → clear error; interpreter names
├── storage/…document store, artifacts, session_files   # Windows-invalid names / reserved names audit (shared validation only if a gap is found)
└── encryption/key.rs               # already cfg(not(unix)); confirm behaviour and message

.github/workflows/
├── release.yml                     # build-app: windows-x64 leg; verify/collect/checksum/sig-check/latest.json/notes
└── ci.yml                          # app job: windows-latest leg

scripts/
├── app-latest-json.mjs             # + windows-x86_64 entry
├── app-latest-json.test.mjs        # updated

justfile                            # set windows-shell; [unix]/[windows] install-app (project deps only); working-directory for app-*
README.md                           # Windows download + Windows development prerequisites (winget commands)
docs/src/getting-started/{desktop-app.md,contributing.md}
```

**Structure Decision**: No new crates or directories beyond one PowerShell script. Everything extends the existing app, workflow and `just` surfaces from spec 038. `CLAUDE.md` gets a short Windows note under the desktop-app section during implementation (the Spec Kit marker block is left as is, per its own instruction).

## Complexity Tracking

*(empty — no constitution violations)*
