# Contract: `just` recipes for the app on Windows

Only the app recipes change. Everything else in the `justfile` stays Unix-only and is not promised to work on Windows.

| Recipe | Linux / macOS (unchanged) | Windows (PowerShell, no WSL) |
|---|---|---|
| `install-app` | apt / Xcode CLT checks, `cargo install tauri-cli`, `cargo fetch` in `app/` | installs `tauri-cli ^2` if absent; `cargo fetch` in `app/`; `git submodule update --init`; `npm ci` in `design/` and `webui/`. Does **not** check or install system prerequisites |
| `app-dev` | `cargo tauri dev` in `app/` | same command, same working directory |
| `app-build` | build WebUI, then `cargo tauri build` in `app/` | same; the output is the NSIS installer under `app/target/release/bundle/nsis/` |
| `app-test` | `cargo test` in `app/` | same |

## Requirements

- `set windows-shell := ["powershell.exe", "-NoLogo", "-NoProfile", "-Command"]` at the top of the file; works on Windows PowerShell 5.1 (no `&&`, no `?:`).
- Recipes use `[working-directory: 'app']` rather than `cd app && …`.
- `install-app` is split with `[unix]` / `[windows]` attributes; the Unix body is not edited.
- The Windows `install-app` is idempotent, needs no administrator rights, and never installs system software.
- System prerequisites are documented, not scripted. The README (Development section) and `docs/src/getting-started/contributing.md` list them with `winget` commands:

  ```powershell
  winget install --id Microsoft.VisualStudio.2022.BuildTools --override "--passive --wait --add Microsoft.VisualStudio.Workload.VCTools --includeRecommended"
  winget install --id Kitware.CMake
  winget install --id LLVM.LLVM
  [Environment]::SetEnvironmentVariable('LIBCLANG_PATH', 'C:\Program Files\LLVM\bin', 'User')
  ```

  plus Rust (`rustup`, MSVC host), Node.js 22+, `just`, and the WebView2 runtime (preinstalled on Windows 11).
- Minimum `just` version stated in the same docs.
