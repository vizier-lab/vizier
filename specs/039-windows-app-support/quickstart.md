# Quickstart: verifying Windows support

Needs a Windows 11 x64 machine (a second clean one or a VM for the install/upgrade checks).

## 1. Developer setup (User Story 3)

```powershell
just install-app     # reports anything missing; fix and re-run until it exits 0
just app-test        # cargo test in app/
just app-dev         # window opens with hot reload
just app-build       # installer at app\target\release\bundle\nsis\Vizier_<version>_x64-setup.exe
```

Expected: no WSL or Git Bash needed; the same four commands still work unchanged on Linux/macOS.

## 2. Install and run (User Story 1)

1. Run the installer. Expect a SmartScreen warning (unsigned): **More info → Run anyway**. No UAC prompt.
2. Launch from the Start menu → first-run screen → create the owner.
3. Workspace exists at `%APPDATA%\rs.vizier.app\workspace`.
4. Start a second copy: the app shows the workspace is in use.
5. Close to tray → tray icon reopens the window; **Quit** exits.
6. `Start-Process "vizier://"` opens/focuses the app.

## 3. End-to-end on the offline `dummyplug` provider

Constitution gate for agent-observable changes. In the app: Settings → create an agent on provider `dummyplug`, then in its chat:

| Send | Expect |
|---|---|
| `tools` | the tool list (includes the shell tool) |
| the shell tool name | a sample request |
| the sample request with the command `echo hello` | tool result contains `hello` — runs through `cmd /C` on Windows |
| `context` | the per-request context block |

Also: send a message, close and reopen the app, confirm the conversation is still there; write a memory document and confirm the `.md` file appears under `workspace\agents\<id>\memory\`.

## 4. Release pipeline (User Story 2)

1. Run `release.yml` with `dry_run: true`: the `Build app - windows-x64` leg is green and `vizier-app-windows-x64` contains `Vizier_<version>_x64-setup.exe`, `.sig`, `.sha256`; the generated `latest.json` has a `windows-x86_64` entry.
2. `just test-scripts` passes (`app-latest-json` test).
3. On a real (pre-)release: the asset is on the release page, the notes name it, and the macOS/Linux/server assets are unchanged.

## 5. Upgrade and auto-update

1. Install release N, create an agent and a conversation.
2. Publish release N+1 (or point the updater at a test manifest); in the app choose **Check for updates → Install and restart**.
3. After restart the agent and conversation are intact; run the installer of N+1 directly over N as well and check the same.
4. Uninstall: the app disappears; `%APPDATA%\rs.vizier.app` remains.

## Edge checks

Install under a user name with a space and a non-ASCII character; create a memory document titled `con` / `a:b` and confirm a clear error rather than a crash (R7).
