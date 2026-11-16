# Feature Specification: The Installable Web App

**Feature Branch**: `055-pwa-webui`

**Created**: 2026-11-10

**Status**: Draft

**Input**: Issue #197 — PWA support for the WebUI: a web app manifest, icons, a service worker, the serving changes the worker needs, and the update lifecycle.

## Context

The WebUI is a page in a browser tab today. It works, but the browser keeps no part of it: the
address bar is the only thing that says "this is a program", there is no icon on the phone's home
screen, and the moment the network drops the tab shows a browser error page instead of the app.

An installable app fixes the first two. The service worker fixes the third, but only honestly: a
cached conversation that is not the live conversation is a lie about a live system, so the shell is
cached and the data is not, and the app says what it cannot do.

The desktop app already has its own updater and its own window frame. This feature is for the web
edition only: the desktop app must never register a worker.

## Scope

- A web app manifest, the icons it names, and the tags the head needs.
- A service worker at the origin root: the shell precached, navigations network-first, the API
  never cached.
- The guard that decides where the worker may register, and the update lifecycle that lets a
  person choose a new worker.
- The serving changes the worker needs: a missing file is a 404, and the shell is revalidated.
- The honest offline surface: a banner that says what is unavailable.

## Out of scope

- Caching the API. A stale chat is a lie about a live system. The "last N topics" cache is a
  follow-up.
- Signing in offline. No token refresh exists, so an expired token offline is a `/login` that
  cannot succeed. The sign-in screen says so rather than faking a session.
- The phone layout. `viewport-fit=cover` is added because the issue asks for it; the layout it
  implies is #152.
- Installing the desktop app as a PWA. It has its own updater.

## User Scenarios & Testing

### User Story 1 - A person installs Vizier (Priority: P1)

A person opens Vizier in a browser and installs it. It appears on their home screen with Vizier's
own mark, and opening it shows Vizier, not a browser tab.

**Why this priority**: it is the feature's name. Without the manifest and the icons there is no
install, and everything else is groundwork.

**Independent Test**: serve the build over HTTPS or on loopback; check that `/manifest.webmanifest`
is served with `application/manifest+json` and names the four icons; check that every icon it
names is served.

**Acceptance Scenarios**:

1. **Given** the web app served on a secure origin, **When** the person installs it, **Then** the
   installed app opens at the root and shows the app shell, not a browser tab.
2. **Given** the manifest, **When** it is read, **Then** it names Vizier, declares `standalone`,
   and points `start_url` and `scope` at `/`.
3. **Given** the desktop app, **When** it starts, **Then** it registers no worker: it has its own
   updater.

---

### User Story 2 - The app says what it cannot do (Priority: P1)

A person's network drops. The app keeps working for what is already open, and says plainly that
replies and new conversations need a connection.

**Why this priority**: a banner that says "you are offline" is a symptom; one that says what
stops working and what keeps working is information. The issue asks for the second.

**Independent Test**: with the network cut, the app shell still renders from the worker's cache
and the banner is on screen with its message.

**Acceptance Scenarios**:

1. **Given** the app has been loaded once and the worker has taken control, **When** the network
   goes away and the page is reloaded, **Then** the app shell renders and the banner says that
   replies and new conversations need a connection, and that what is already open stays readable.
2. **Given** a topic whose socket has lost its connection, **When** the app is on any page,
   **Then** the shell-wide banner is on screen.
3. **Given** the sign-in screen and no network, **When** the person looks at it, **Then** it says
   that signing in needs a connection, rather than failing with a message about a password.
4. **Given** the app is served on another device's address over plain HTTP, **When** the person
   opens it, **Then** the app says the install is loopback-only and does not attempt to register
   a worker: the browser would refuse it anyway.

---

### User Story 3 - A new version arrives without taking the page away (Priority: P2)

A person is using Vizier and a new version is deployed. Nothing changes under them. A toast
offers the new version, and only if they select it does the app reload into it.

**Why this priority**: it comes after the app exists offline, because an update that reloads the
page is the one thing that can lose a person's place.

**Independent Test**: serve a build, then serve a second one; the app shows the toast, and the
page only reloads when the toast is selected.

**Acceptance Scenarios**:

1. **Given** a worker is in charge and a new one is installed, **When** the new one is waiting,
   **Then** the app offers it as a toast and nothing reloads on its own.
2. **Given** the toast, **When** the person selects it, **Then** the new worker takes over and the
   app reloads into it.
3. **Given** the first worker ever installed, **When** it finishes installing, **Then** no update
   toast appears: there is nothing to update to.

---

## The decisions this took

The issue asks for four decisions. This branch takes a position on each, and says why.

1. **The worker lives in `webui/public/`, not in `app/`.** `boundary.test.ts` scans `app/` and
   bans `fetch(`, `new WebSocket(`, `'/api/v1'` and `@tauri-apps/`. A service worker calls
   `fetch()` for a living, so putting it in `app/` would need an allowlist entry that reads like
   "the worker may talk to the API". Outside `app/` the boundary stays exactly as strict.
2. **The cache holds the shell and nothing else.** The issue's own recommendation, and the safest
   reading. The cost is stated below.
3. **No `vite-plugin-pwa`.** A ~90-line hand-written worker covers the three rules the issue
   needs. The constitution asks every dependency to justify itself against a hand-rolled
   alternative; this one does not.
4. **The serving fix ships as its own commit.** It is correct with or without the PWA feature and
   is independently reviewable.

## What the issue asks that this does not do

- **The last conversation is not shown from cache, marked offline.** The shell is cached;
  conversation history is not. This is deliberate: the banner says what is unavailable rather
  than showing a stale session that looks live. A "last N topics" cache is a follow-up.
- **`theme_color` in the manifest cannot follow the stored appearance.** A manifest is a static
  file and cannot read `localStorage`. The `<meta name="theme-color">` does follow it, before the
  first paint and on every change; the manifest declares `color_scheme: "dark light"`.

## Corrections to the issue as written

- **A miss on `assets()` cannot always be a 404.** The client-side router owns about thirty
  routes, and the fallback to the shell is what makes them work. The rule used here: a miss is a
  404 when the last path segment carries the extension of a static file, and the shell otherwise.
  `/settings/roles` still gets the shell; `/nope.js` does not. (The extension, not a bare dot: a
  memory document named `notes.v1.md` is addressed as `notes.v1`, and its route is a route.)

## The worker is never stored

The worker is served `no-store`, not `no-cache`. A CDN in front of the server may keep its own
copy and hand the browser a stale one whatever the origin says — Cloudflare's default browser TTL
is four hours for a `.js` file, which is long enough to pin a worker for a whole afternoon. The
worker also carries the id of the build it belongs to, so a new build is a new worker: the two
together mean a deployment reaches the browser when it happens, not when a cache expires.

## The branches

| Repo | What changes |
|---|---|
| `vizier-lab/design` | The `Banner`'s offline tone shows its second line, so it can say what is unavailable. |
| `vizier-lab/webui` | The manifest, the icons, the worker, the guard, the update toast, the offline banner, the tests. |
| `vizier-lab/vizier` | `crates/vizier-http/src/webui/mod.rs` and the pin bumps. |

Commit order: design → webui → vizier.

## Verification

- `design`: the Banner's offline story, in both themes.
- `webui`: `npm run typecheck`, `npm test`, `npm run build`, and `npm run e2e` against a running
  server.
- `vizier`: `cargo test -p vizier-http` and the CI clippy lint set.

The WebUI's own test suites are not part of the repository's CI (only `npm run build` is), so
these tests are not enforced automatically. That is worth raising separately: the WebUI has over a
thousand vitest tests that nothing runs.
