# Contract: `just` recipes for the app on Windows

Only the app recipes change. Everything else in the `justfile` stays Unix-only and is not promised to work on Windows.

| Recipe | Linux / macOS (unchanged) | Windows (PowerShell, no WSL) |
|---|---|---|
| `install-app` | apt / Xcode CLT checks, `cargo install tauri-cli`, `cargo fetch` in `app/` | runs `scripts/install-app.ps1`: checks Rust (MSVC target), MSVC build tools, CMake, LLVM/libclang, Node, WebView2; installs `tauri-cli ^2`; `cargo fetch` in `app/`; `git submodule update --init`; `npm ci` in `design/` and `webui/` |
| `app-dev` | `cargo tauri dev` in `app/` | same command, same working directory |
| `app-build` | build WebUI, then `cargo tauri build` in `app/` | same; the output is the NSIS installer under `app/target/release/bundle/nsis/` |
| `app-test` | `cargo test` in `app/` | same |

## Requirements

- `set windows-shell := ["powershell.exe", "-NoLogo", "-NoProfile", "-Command"]` at the top of the file; works on Windows PowerShell 5.1 (no `&&`, no `?:`).
- Recipes use `[working-directory: 'app']` rather than `cd app && …`.
- `install-app` is split with `[unix]` / `[windows]` attributes; the Unix body is not edited.
- `scripts/install-app.ps1`:
  - exit code 0 only when every prerequisite is present;
  - for each missing item prints one line: what is missing and the exact command or URL to fix it;
  - never needs administrator rights, never installs system software silently;
  - is idempotent.
- Minimum `just` version stated in `docs/src/getting-started/contributing.md`.
