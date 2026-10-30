# Research: Vizier Desktop App Edition

Decisions for issue #102 phases 5–7, made against the tree at `8493ea0` (the merged
`037-core-extraction`). Each entry: **Decision**, **Rationale**, **Alternatives considered**.

Facts gathered from the tree that drive these decisions:

- `vizier-http` has **118 routes** in 23 handler files (≈ 15.9k lines). About **100 permission
  checks** live there: `require_permission` route layers for settings/roles/users/providers, and
  per-handler helpers (`user_can_view_agent`, `user_can_edit_agent`,
  `user_can_show_platform_chats` in `api/v1/agents/mod.rs`) plus calls into the three core
  access modules that already exist (`integrations/access.rs`, `secrets/access.rs`,
  `artifacts/access.rs`, all on `access::Caller`).
- Handlers return the envelope `APIResponse { status, message, data }`; refusals with a code
  (`409 stale_revision`, `422 moved_to_integrations`, …) put it in `data.code` or a top-level
  `code`. The WebUI's `ApiError` reads exactly these fields.
- The chat WebSocket (`api/v1/agents/channel.rs::chat` + `handle_socket`, ≈ 260 lines) is the
  only streaming route. It validates client tools, builds a `ClientConnection`, delivers
  deferred client calls, publishes the person's turn on the session-event broadcast and
  forwards frames. The session is `VizierSession(agent, VizierChannelId::HTTP(username,
  channel), topic)`.
- WebUI: `app/api/client.ts` (`request`) and `app/api/socket.ts` (`TopicSocket`) are the JSON
  and socket entry points. **Twelve** other places build `/api/v1/...` URLs with `?token=` or
  call `fetch` with a bearer header: `FilePreviewModal.tsx`, `FilePreviewDialog.tsx`,
  `ArtifactsGallery.tsx`, `endpoints/artifacts.ts`, `endpoints/memory.ts` (bundle export),
  `endpoints/chat.ts` (socket URL), `stores/connection.ts`, `lib/generativeUi/media.ts` and
  its users in `registry/{anchor,media,canvas2d}.tsx`, `lib/trail.ts`.
- `release.yml` already builds `aarch64-unknown-linux-gnu` natively on `ubuntu-24.04-arm` and
  both macOS targets on `macos-latest`. The server sends no global CSP.
- `User.password_hash` is a non-optional `String`; `Role.is_system` is the bypass flag;
  `HTTPChannelConfig` has `port`, `jwt_secret`, `jwt_expiry_hours`, `ws_idle_timeout_secs`;
  `vizier_http::router` hard-codes `CorsLayer::allow_origin(Any)`.
- sherpa-onnx is gone since spec 032 (`whisper-rs` + `any-tts`). The load-bearing pin is
  `fastembed = "=4.9.1"` (ort rc.9, the last line with an `x86_64-apple-darwin` prebuilt).

---

## R1. How the app's window reaches the engine: in-process dispatch through the one route table

**Decision**: The app does **not** get one Tauri command per API operation. It gets one
command, `api`, that takes `{method, path, headers, body}` and dispatches it **in memory**
through the same axum `Router` the server mounts (`tower::ServiceExt::oneshot`), built in a
desktop auth mode that resolves the bound desktop owner instead of reading a bearer token. No
socket is opened, nothing is parsed off a wire, and the router built this way is never handed
to a listener (R4). Binary GETs (files, artifact content, logos, icons, bundle export) go
through a custom URI scheme backed by the same dispatcher (R6). The chat stream is the one
thing that cannot be a request/response, and gets its own transport-neutral connection (R5).

**Rationale**:
- The WebUI already talks in `(method, path, body)` (`client.ts::request`), and the issue's
  phase-6 `Transport.request` is generic. A per-operation command set means 118 Rust commands
  plus a 118-entry path→command table in `tauri.ts`, two API surfaces to keep in step forever:
  the duplication Principle II forbids.
- Parity (FR-008, FR-009, SC-002, SC-003) becomes structural: both editions run the same
  handlers, the same service calls, the same refusals, the same envelope.
- It honours what decision #1 in the issue protects against. The issue rejects an *embedded
  localhost HTTP server* because it opens a port and makes the app a network service; this
  opens none (SC-004).

**Alternatives considered**:
- *Typed Tauri command per service operation* (the issue's literal phase 7): rejected for the
  duplication above. The service layer (R2) still gives third-party embedders typed calls.
- *Embedded localhost server on a random port*: opens a port, needs a token handshake, and is
  what decision #1 rejects.
- *Route everything through the custom URI scheme with `fetch`*: WebKitGTK drops request bodies
  on custom schemes on some versions (the reason Tauri's own IPC on Linux avoids them), so
  POST/PUT/uploads would be unreliable on Linux. Kept for GETs only.

## R2. Service layer shape (phase 5)

**Decision**: `vizier_core::service`, one module per domain the WebUI calls (agents, core,
memory, documents, tasks, dream, topics/history/files, jobs, conversations, artifacts,
secrets, integrations, skills, users/roles/api-keys/profile/setup, providers, custom
providers, catalogue, usage, notifications, overview, files, embedding, setup state). Each
operation is an `async fn` taking `&Services` (cheap clone over `VizierDependencies`), a
`&Principal` and a plain input struct, returning `Result<T, ServiceError>`. Authorization is
the first thing each operation does, through `access` helpers that **move from `vizier-http`
into core** (`can_view_agent`, `can_edit_agent`, `can_show_platform_chats`, the
`require(permission)` check behind today's `require_permission` layer). Handlers in
`vizier-http` shrink to: extract → call service → map `ServiceError` to status + envelope.

`ServiceError` is a closed set of kinds that each map to one HTTP status: `NotFound`,
`Forbidden`, `Unauthorized`, `Invalid { code? }` (400/422), `Conflict { code, data? }` (409),
`Gone { code }` (410), `TooLarge`, `RateLimited`, `Unavailable` (503), `Internal`. Each carries
the message the handler sends today, so the envelope bytes stay identical (contract
[service-layer.md](contracts/service-layer.md)).

`Principal` replaces `auth::Identity` as the owned value handlers pass (same fields, plus
`AuthMethod::Desktop`); `access::Caller` keeps borrowing from it.

**Rationale**: Phase-5 exit criteria in the issue (one home for the permission checks, thin
handlers, `ServiceError` the only error crossing the boundary, adapter tier narrowed). R1
means the app does not *need* services to reach parity, but third-party UIs (issue goal 3)
and the narrowing of the `#[doc(hidden)]` tier do, and the user confirmed phase 5 is in
scope. Moving the checks next to the domain logic is also what keeps a future typed IPC or
remote client honest.

**Alternatives considered**: a `tower`-style generic service trait per operation (rejected:
one implementation each, Principle I); per-domain error enums (rejected: Principle V, and the
HTTP mapping would branch per domain).

**Order**: R5's topic connection and `AuthMethod::Desktop` land first (the app needs them);
the domain ports follow one domain per PR and can run in parallel with app work, since R1
does not depend on them.

## R3. Desktop identity and binding

**Decision**: `AuthMethod::Desktop` (the enum is already `#[non_exhaustive]`). The desktop
principal is a real `user` row. Binding lives in the app's config file
(`{app_config_dir}/desktop.json`), keyed by canonical workspace path so a person who opens a
second workspace gets a second binding. Startup states:

| Workspace state | App does |
|---|---|
| No users | First-run screen: name (password optional) → create a user with the built-in system role → bind |
| Exactly one user with a system role | Bind it silently |
| Several | Ask once, store the choice |
| Bound user deleted | Back to "choose" |

A user created without a password stores `password_hash = ""`; login refuses an empty hash
explicitly (today argon2 would merely fail to parse it). No schema change.

**Rationale**: Issue decisions #3 and #9; FR-006, FR-007. A real row keeps ownership,
`chat_reply` audience, usage and revision attribution working with zero special cases.
Conversations stay keyed `HTTP(username, channel)`, so the same conversation shows in the
app and over web access.

**Alternatives considered**: a synthetic god-mode identity (rejected by the issue: breaks
ownership and audit); storing the binding in the workspace DB (rejected: a server edition on
the same workspace would read it).

## R4. Keeping the desktop principal off the network

**Decision**: `HTTPState` gains `auth: AuthMode` (`Network` | `Desktop { user_id }`).
`require_auth` reads it: `Network` behaves exactly as today; `Desktop` resolves that user's
role and permissions from storage on every request (so a role change applies at once) and
inserts a `Principal { method: Desktop }`. `vizier_http::serve` and `vizier_http::router`
always build `Network`. The only constructor of a `Desktop` state is
`vizier_http::embedded::Dispatcher::new(runtime, user_id)`, which exposes `dispatch(Request) ->
Response` and no `Router`, so it cannot be bound to a listener. In `Network` mode
`require_auth` also rejects any request that already carries a desktop principal (defence in
depth; nothing creates one today). An automated test sends a request with every credential
form to the network router and asserts none yields `AuthMethod::Desktop` (SC-010).

**Rationale**: FR-018, issue decision #9, with one field and no second router definition.

## R5. Live frames without a WebSocket

**Decision**: Extract the body of `handle_socket` into a transport-neutral
`TopicConnection` (in `vizier-http`, module `topic`, next to the handler it came from): it
takes the authorized session, the per-connection limits and an outbound
`flume::Sender<serde_json::Value>`, and exposes `handle_inbound(Value)` and `close()`.
The WebSocket handler becomes a pump between the socket and a `TopicConnection`. The app
drives the same type from three commands: `topic_open {agent, channel, topic, frames:
Channel}` (authorizes through the desktop dispatcher's principal, returns a handle id),
`topic_send {id, frame}`, `topic_close {id}`. Frames go out over a Tauri `ipc::Channel`,
which is ordered and meant for streaming (Tauri's events are not ordered and re-serialize per
listener). Client tools (spec 019) work unchanged because `ClientConnection` already sends
`serde_json::Value` (037 R5).

**Rationale**: FR-010 and the spec-023/029 behaviours (reopen mid-turn, second tab) all hang
off `subscribe_session_events`, which `TopicConnection` keeps using. One implementation, two
pumps.

**Alternatives considered**: putting it in core (rejected for now: it depends on the
WebSocket handler's request types, which move to core with the topics service port in R2,
and it moves then); Tauri events (unordered).

## R6. Binary assets, uploads and downloads

**Decision**:
- **GET assets** (session files with `Range`, artifact content, provider logos, integration
  icons, bundle export): custom URI scheme `vizier-asset://localhost/api/v1/...`, registered
  with `register_asynchronous_uri_scheme_protocol`, dispatching a GET (with the `Range`
  header) through the desktop dispatcher. `transport.assetUrl(path)` returns this URL in the
  app and `/api/v1/...?token=` over HTTP, which replaces the twelve call sites listed above.
- **Uploads**: `tauri.ts` turns the `FormData` into bytes with `new Response(form)` (which
  also yields the `multipart/form-data; boundary=` header) and calls the `api` command with a
  raw body (`InvokeBody::Raw`, method/path/content-type in invoke headers). Same handler, same
  multipart parser; drag-and-drop and `<input type=file>` keep producing `File` objects.
- **Drag-and-drop**: the window sets `dragDropEnabled: false`, so the webview delivers HTML5
  drops with `File` objects exactly as a browser does. No native drop handler is needed.
- **Downloads / open in tab**: `transport.saveFile(path, filename)` and
  `transport.openFile(path)`. In the app: a native save dialog, then the Rust side streams the
  dispatcher's response to that file; "open" saves to a temp file and opens it with the
  opener plugin. The webview's own download handling differs across WebKitGTK and WKWebView,
  so it is not relied on.

**Rationale**: FR-011, FR-012, spec edge case on large files (bytes never cross JSON).

## R7. Frontend packaging and the WebUI build

**Decision**: `tauri.conf.json` `build.frontendDist = "../../webui/build/client"`,
`devUrl = "http://localhost:5173"`. The webview runs the same `webui/build/client` the server
embeds (FR-005). The app also enables `vizier-http`'s embedded WebUI so web access (R10) can
serve it, which puts a second copy of the built UI in the binary (≈ a few MB compressed);
accepted to keep the window on Tauri's own asset protocol.

`vizier-http/build.rs` keeps working for the app: it builds the WebUI if `webui/node_modules`
exists, else uses `webui/build/client`. The release job builds the WebUI once and removes
`node_modules`, as the server job does today.

SPA fallback: Tauri's asset resolver serves `index.html` for unknown extension-less paths;
quickstart §5 checks a reload on a nested route. Deep links navigate in-app instead of
reloading.

**Alternatives considered**: serving the window from a custom scheme backed by
`vizier-http`'s embed (one copy, server's own fallback), rejected because IPC availability
and capability scoping for custom-scheme origins is less proven than Tauri's default origin.

## R8. WebUI transport seam

**Decision**: `webui/app/api/transport/` with `index.ts` (interface + selection),
`http.ts` (today's `client.ts` fetch path and `TopicSocket`) and `tauri.ts`. Selection is at
module load: `'__TAURI_INTERNALS__' in window` → Tauri. `@tauri-apps/api` is imported only
from `tauri.ts` through a dynamic `import()`, so the HTTP build path never loads it.
`client.ts::request`, `TopicSocket` and every asset URL go through the transport; screens and
stores do not change apart from the twelve URL call sites and the auth bootstrap. Interface in
[webui-transport.md](contracts/webui-transport.md). An ESLint `no-restricted-syntax` rule (or
a vitest scan if lint is not set up) forbids `fetch(`/`new WebSocket(`/`/api/v1/` outside
`app/api/transport/`.

`authMode: 'desktop'` skips `/login`, seeds the session store from `GET /auth/users/me`
(through the desktop dispatcher), hides Sign out, and shows the desktop bootstrap screen
(R3) when the app reports it is not bound yet.

**Rationale**: issue phase 6 as written; FR-005.

## R9. Native shell features and Tauri plugins

**Decision**: Tauri 2 (exact 2.x pinned at implementation, in `app/src-tauri/Cargo.toml`),
with the official plugins only:

| Need | Plugin / API |
|---|---|
| FR-004 single instance | `tauri-plugin-single-instance` (registered first; focuses the window, forwards deep-link args) |
| FR-016 `vizier://` | `tauri-plugin-deep-link` (Linux: registered at runtime for the AppImage via `register_all`) |
| FR-013 external links | `tauri-plugin-opener` + an `on_navigation` guard and new-window handler: anything not the app origin opens in the default browser |
| FR-014 OS notifications | `tauri-plugin-notification` (R11) |
| FR-015 tray | Tauri `tray-icon` feature (menu: Open, Quit; close-to-tray setting) |
| Workspace picker, save dialog | `tauri-plugin-dialog` |
| FR-021 updates | `tauri-plugin-updater` + `tauri-plugin-process` (relaunch) |

No other third-party crates. Each is the standard Tauri answer, so Principle I's "concrete
reason" is the feature requirement itself.

## R10. Web access in the app

**Decision**: Off by default (FR-017). When the person turns it on in the desktop settings
page, the app calls `vizier_http::serve` on the same `VizierRuntime` with:
- bind `127.0.0.1:<port>` (default 9999, editable), or `0.0.0.0` only after the explicit
  "allow other devices" switch and its warning (FR-019);
- `jwt_secret` from `{workspace}/.runtime/web-access.key` (32 random bytes, generated once,
  mode 0600, same handling as `encryption.key`);
- CORS restricted to the served origin. This needs `HTTPChannelConfig.cors_origins:
  Option<Vec<String>>` (new, `#[serde(default)]`); `None` keeps today's `Any`, so the server
  edition is unchanged (FR-022). The app always passes `Some(served origins)`.

The toggle is refused unless the bound user has a password or at least one API key; the
settings page then offers the existing change-password flow (with the "no current password"
case allowed only for a desktop principal whose hash is empty). Turning it off stops the
listener; open browser sessions end (spec edge case). The HTTP listener runs as a task the app
owns, so its failure is shown in the desktop settings and does not stop the engine.

**Rationale**: issue decisions #8 and #9.

## R11. OS notifications

**Decision**: The WebUI already polls `/notifications/count` every 15 s while visible and
raises an in-app toast on new ids (`NotificationToasts.tsx`). In desktop mode the window is
**hidden, not destroyed**, on close-to-tray, so that code keeps running; when the document is
hidden or the window unfocused, it calls `transport.notify({title, body, href})` instead of the
in-app toast. The Rust side shows it through `tauri-plugin-notification`; selecting it focuses
the window and navigates to `href`. Where the platform does not report the click (verified in
quickstart per OS), focusing the app opens the notifications panel instead.

The poll keeps running while hidden (it currently stops when not visible); in desktop mode it
continues at 60 s while hidden.

**Rationale**: no new core event, no second notification path (the core `Notifications` stays
the only creator, per spec 029). FR-014.

**Alternatives considered**: a core broadcast of new notification rows consumed by the Rust
shell (rejected for now: audience/visibility filtering would be reimplemented outside the
service that owns it).

## R12. Engine lifecycle in the app

**Decision**: The app owns one tokio multi-thread runtime and one `VizierRuntime` at a time.
`RuntimeEvent::SubsystemStopped` is forwarded to the window as a `runtime` event; the WebUI
shows a blocking banner with **Restart** (`runtime_restart`: shutdown, then start again on the
same workspace) and the stop reason (FR-020). A workspace lock refusal (the server edition is
running on it) is shown on the bootstrap screen with the lock-holder message from
`WorkspaceLock` (FR-003). Quit (tray or last window close without close-to-tray) calls
`VizierRuntime::shutdown`, which aborts turns resumably (spec 023), satisfying US1 scenario 3.

## R13. App workspace and dependency pins

**Decision**: `app/` is its own Cargo workspace (issue decision #6) with
`app/src-tauri` depending on `../../crates/vizier-core` (`features = ["full"]`) and
`../../crates/vizier-http`. Its own `Cargo.lock` is committed. A `policy` CI step diffs the
versions of `fastembed`, `ort`, `ort-sys`, `whisper-rs`, `any-tts` between `app/Cargo.lock`
and the root `Cargo.lock` and fails on any difference (FR-023). The root workspace's
`cargo test --workspace` never sees `app/` (FR-022).

The app's version comes from `app/src-tauri/Cargo.toml`, which the release `prepare` job bumps
next to the root (tauri.conf.json omits `version`, so Tauri reads the Cargo version).

## R14. Release artifacts and updates

**Decision**: A new `build-app` job in `release.yml` (after `prepare-release`, parallel to
the server `build` job), uploading into the same `create-release` step:

| Asset | Runner | Notes |
|---|---|---|
| `Vizier_<v>_universal.dmg` + `Vizier.app.tar.gz` (+ `.sig`) | `macos-latest` | `tauri build --target universal-apple-darwin`; both rust targets installed; unsigned for now (ad-hoc signature only, #132); minimum macOS 11.0 |
| `Vizier_<v>_amd64.AppImage` (+ `.sig`) | `ubuntu-22.04` | `bundleMediaFramework: true` (GStreamer for audio recording/playback) |
| `Vizier_<v>_aarch64.AppImage` (+ `.sig`) | `ubuntu-22.04-arm` | native build, same config |
| `latest.json` | `create-release` | assembled from the four `.sig` files: `darwin-aarch64` and `darwin-x86_64` both point at the universal `.app.tar.gz`; `linux-x86_64`, `linux-aarch64` at the AppImages |

Ubuntu 22.04 runners give the AppImages a glibc 2.35 floor (24.04 would raise it to 2.39).
The updater endpoint is
`https://github.com/vizier-lab/vizier/releases/latest/download/latest.json`; prereleases are
not marked `latest`, so app users only follow stable releases. Checks in the job:
`lipo -archs` on the universal binary reports `x86_64 arm64`; `file` on each AppImage reports
the right machine. The server `build` job, tarballs, Docker image and `install.sh` are not
touched (FR-022). Dry runs build everything and skip upload, like the server job.

**Signing**: the macOS build is **unsigned and not notarized** for now (decided
2026-10-30, tracked in #132). Consequences, documented in the install docs and release notes:
Gatekeeper blocks the first launch until the person clicks **Open Anyway** in System Settings ›
Privacy & Security (macOS 15 removed the right-click › Open bypass), and the microphone
permission is asked again after each update because the ad-hoc signature changes per build.
The job signs and notarizes automatically when the `APPLE_*` secrets are present (Tauri reads
`APPLE_CERTIFICATE`, `APPLE_CERTIFICATE_PASSWORD`, `APPLE_SIGNING_IDENTITY`, `APPLE_ID`,
`APPLE_PASSWORD`, `APPLE_TEAM_ID`) and builds unsigned otherwise, so #132 is only adding secrets
and removing the doc step.

**Update signatures** are required on every platform and independent of Apple:
`TAURI_SIGNING_PRIVATE_KEY` (+ `_PASSWORD`) secrets, public key committed in
`tauri.conf.json`. A non-dry release fails if they are missing; dry runs skip the `.sig` files.

**Alternatives considered**: `tauri-apps/tauri-action` for build + upload (rejected: it creates
or edits the release itself, which fights the existing `create-release` job; we only use
`tauri build` via the CLI and our own upload).

## R15. Webview differences to bridge

| Concern | Decision |
|---|---|
| Audio chat (`MediaRecorder`, `getUserMedia`) | macOS: `NSMicrophoneUsageDescription` in `Info.plist` + `com.apple.security.device.audio-input` entitlement. Linux: WebKitGTK with the bundled GStreamer (R14); if `MediaRecorder` is missing at runtime, the mic button shows "audio recording isn't available in this window". |
| Downloads | Never rely on the webview (R6). |
| Large payloads over IPC | Raw bodies (`InvokeBody::Raw`) for uploads; assets via the URI scheme; JSON over `api` only for envelopes. |
| CSP | The server sends none. The app sets one in `tauri.conf.json`: `default-src 'self'`, `connect-src 'self' ipc: http://ipc.localhost vizier-asset:`, `img-src 'self' data: blob: vizier-asset:`, `media-src 'self' blob: vizier-asset:`, `frame-src 'self' blob: data:` and `worker-src 'self' blob:` (generative UI's sandboxed iframe and logic worker), `style-src 'self' 'unsafe-inline'`. |
| Generative UI iframe (`sandbox="allow-scripts" srcdoc`) | Checked in quickstart on both webviews. |

## R16. Testing

**Decision**:
- **Conformance suite** (`crates/vizier-http/tests/conformance.rs`): a table of request
  vectors (every route group, allowed and refused cases, for a system user, an owner with
  `owned_agents:*` permissions and a user with none) sent through the network router with a
  JWT and through `embedded::Dispatcher` for the same user; asserts equal status and envelope
  (FR-009, SC-002, SC-003). Coverage check: every route in the OpenAPI document appears in at
  least one vector.
- **Desktop principal check** (SC-010): every credential form against the network router;
  none yields `AuthMethod::Desktop`, and a dispatcher cannot be turned into a `Router`.
- **Service unit tests** where a domain has branching rules (access helpers moved from
  `vizier-http` keep their behaviour; `access.rs` tests extend).
- **WebUI**: vitest for both transports against the same fixtures (envelope unwrapping,
  401 handling skipped in desktop mode, asset URLs, upload bytes); `npm run typecheck`.
- **App smoke test** (`app/src-tauri/tests/smoke.rs`, run on desktop CI runners only): boots
  the runtime on a temp workspace with a dummyplug agent, calls the `api` and `topic_*`
  command functions directly (no window), runs a tool through the dummyplug loop.
- **Dummyplug end to end** against the built app: [quickstart.md](quickstart.md).

## R17. Out of this feature

Native tools for agents and the OS-keychain secret store (phase 8), Windows, `.deb`/`.rpm`/
Flatpak/Homebrew, app lock, remote-vizier mode in the app, TLS for web access, catalogue split,
`fastembed` upgrade.

## R18. Branches and repos

`just checkout 038-desktop-app-edition` creates the branch in every submodule (the hook only
created it in the workspace; constitution follow-up). Changes land in `webui` (transport,
desktop bootstrap and settings screens) and the workspace (`crates/`, `app/`, workflows,
docs). `design` changes only if a component is missing (expected: none; the desktop screens
compose existing `Dialog`, `Banner`, form controls). Commit order design → webui → vizier.
