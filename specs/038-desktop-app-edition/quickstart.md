# Quickstart: verifying the desktop app edition

Agent steps use the offline `dummyplug` provider (protocol:
`specs/008-dummyplug-provider/contracts/dummyplug-protocol.md`). No keys, no network.
Steps marked **[live]** need a real provider; **[mac]**, **[linux]**, **[arm64]** need that
machine.

## 0. Setup

```sh
just checkout 038-desktop-app-edition   # workspace + every submodule
just install
cd webui && npm run build && cd ..
cargo install tauri-cli --version '^2' --locked   # once
# Linux build deps: libwebkit2gtk-4.1-dev libayatana-appindicator3-dev librsvg2-dev
```

## 1. Service layer: server unchanged (every phase-5 PR)

Baseline once on `master` (as in 037 §1): `/openapi.json`, `--help`, and the conformance
vectors' recorded responses (`target/038-baseline/`).

```sh
cargo test --workspace
cargo test -p vizier-http --test conformance          # network router vs embedded dispatcher
cargo test -p vizier-http --test desktop_principal    # SC-010
cargo clippy --workspace
cargo test -p vizier-core --no-default-features
diff <(curl -s localhost:19999/openapi.json) target/038-baseline/openapi.json   # only additive: has_password, cors_origins absent from API
```

Then `just run` with a dummyplug agent and, in the WebUI over HTTP:

| Send in chat | Expect |
|---|---|
| `tools` | the four sandbox tools |
| `execute_python` | a sample JSON request |
| the sample JSON request | the tool result; history shows call + result |
| `context` | the context block |

Log in as a user with no `all_agents:view` and open another person's agent URL → the same
"Access denied" as before the port.

## 2. App boots on a fresh workspace (US1)

```sh
cd app && cargo tauri dev        # devUrl = Vite on :5173 (run `cd webui && npm run dev` first)
```

1. The window opens on the desktop bootstrap: **needs owner**. No login page.
2. Enter a name, leave password empty → main interface, signed in, no Sign out.
3. `ss -ltnp | grep -i vizier` (Linux) / `lsof -iTCP -sTCP:LISTEN | grep -i vizier` (mac) →
   **nothing** (SC-004).
4. Create an agent on `dummyplug`. In chat run the four sends from §1 → same replies, tool
   steps stream in as they happen (SC-005).
5. Upload a file by dragging it onto the chat, and another with the attach button → both
   attached; ask with `context` → both listed.
6. Open a session file preview (image and a large text file) → renders; Range preview
   works; **Download** opens a native save dialog and writes the file.
7. Provider logos and integration icons render on the providers and integrations pages.
8. Reload on `/agents/<id>/chat` (Cmd/Ctrl-R) → same screen (SPA fallback).

## 3. Lifecycle (US1)

1. Start a turn with a long `execute_python` (`time.sleep`-style sample from dummyplug), quit
   from the tray mid-turn, reopen → the turn resumes or shows as interrupted; nothing lost
   (SC-006).
2. Launch the app a second time → the first window comes forward; no second engine
   (`desktop_state` stays `ready`, no lock error).
3. With the app closed, `vizier run` on the app's workspace (`VIZIER_DATA_DIR=<app
   workspace>`) → then open the app → **locked** screen naming the holder; "Open another
   workspace" works.
4. Point the app at an existing server workspace (copy of 037's baseline workspace) with one
   system user → it binds silently; agents, memory, history are there (US1 sc. 5). With two
   system users → asked once, remembered across restarts.
5. Force a subsystem stop (debug command `runtime_debug_stop_scheduler`, compiled only in
   debug builds) → degraded banner with Restart; Restart returns to `ready`.

## 4. Parity (US2)

1. `cargo test -p vizier-http --test conformance` green, coverage check lists every OpenAPI
   path (SC-002, SC-003).
2. In the app, create a second user with a role lacking `all_agents:view`; bind the app to it
   (Settings › Desktop › Switch account is not offered: do it by editing `desktop.json` in a
   debug build) → another person's agent shows "Access denied", exactly as over HTTP.
3. Edit CORE and a memory document in the app → history shows the desktop owner and method
   `desktop`.
4. Background job from chat (`background_subagents` sample request via dummyplug) → job tray
   updates live, woken reply streams in.

## 5. Desktop conveniences (US3)

1. Close the window with close-to-tray on → tray icon remains; a dummyplug task scheduled a
   minute ahead runs; an OS notification appears; selecting it opens the task run.
   **[mac] [linux]**: record whether the click navigates or only focuses (R11 fallback).
2. Close-to-tray off → closing the window quits; the engine shuts down cleanly (lock file
   released).
3. A link in an agent reply opens the default browser; the window stays put.
4. `xdg-open 'vizier://agents/<id>/chat'` / `open 'vizier://…'` → window focuses on that
   chat; works when the app was not running too.
5. **[mac]** Audio chat: mic permission prompt, record, reply plays. **[linux]** same on the
   AppImage (bundled GStreamer).
6. Generative UI: dummyplug `ui_render` sample → renders, a `logic` function runs.

## 6. Web access (US4)

1. Fresh owner without password: toggle web access → refused with "set a password"; set one
   in the offered flow → toggle succeeds; `127.0.0.1:9999` only.
2. Browser at `http://127.0.0.1:9999` → login page; sign in → same conversation as the app
   window, live in both.
3. `curl -s localhost:9999/api/v1/agents` → 401. No header form yields the desktop identity
   (covered by `desktop_principal` test; spot-check with `Authorization: Desktop x`).
4. Cross-origin `fetch` from another local origin → blocked by CORS.
5. "Allow other devices" → warning shown; listens on `0.0.0.0`; reachable from another
   device on the LAN.
6. Toggle off with a browser open → the browser's next request fails, socket closes.

## 7. Release rehearsal (US5)

Run `release.yml` with `dry_run: true` from the branch:

1. The server job's four tarballs are produced as before.
2. `build-app` produces the universal `.dmg` + `.app.tar.gz`, two AppImages, `.sig`s,
   `latest.json`; the job's `lipo -archs` prints `x86_64 arm64`; `file` reports both Linux
   machines.
3. **[mac]** Install the `.dmg` on an Apple Silicon and an Intel Mac (macOS 11+): the first
   launch is blocked; **Open Anyway** in System Settings › Privacy & Security opens it; the
   documented steps match what happens (SC-007). After an update, the microphone prompt
   reappears (expected until #132).
4. **[linux] [arm64]** Run each AppImage on Ubuntu 22.04 (x86_64 and arm64) → opens, §2 steps
   1–4 pass.
5. Updater: install version N, publish N+1 to a test release channel (override the endpoint
   with `VIZIER_UPDATER_ENDPOINT` in a debug build) → app offers, installs, relaunches on all
   four (SC-008).
6. `diff <(grep -E '^name = "(fastembed|ort|ort-sys|whisper-rs|any-tts)"' -A1 Cargo.lock)
   <(grep -E '…same…' -A1 app/Cargo.lock)` → no output (FR-023).

## 8. Onboarding time (SC-001) [live optional]

From a fresh install: open the app → owner → create an agent on Ollama (or dummyplug offline)
→ first reply. Target under 5 minutes without a terminal.
