---
description: "Task list for the installable Web App"
---

# Tasks: The Installable Web App

**Input**: Issue #197 and `specs/055-pwa-webui/spec.md`.

**Tests**: Included. Pure logic gets vitest tests in `app/lib/`, the guard is a pure function over
the origin, and the serving rules are unit-tested in the module they live in. The end-to-end
check is a Playwright spec against a running server.

**Organization**: The tasks are grouped by repository, because the commit order is design → webui
→ vizier and each repository's work is independent of the others.

## Format: `[ID] [P?] Description`

- **[P]**: Can run in parallel (different files, no dependency on an incomplete task)

## Path conventions

- Design and WebUI are submodules: `design/`, `webui/`.
- Commit order is design → webui → vizier, with `just commit` and `just push`.

---

## Phase 1: Design

- [X] T001 Draw the offline banner first: `specs/055-pwa-webui/mockups/offline-banner.html`, a
      standalone page with the design tokens, light and dark. FigCraft is not reachable here, so
      the mockup is the fallback the constitution allows.
- [X] T002 Let the `Banner`'s offline tone show its second line
      (`design/src/components/Banner/Banner.tsx`). It rendered only its title, so the chat's own
      offline banner has been dropping the message it passes. The story asserts it.
- [X] T003 Update the Banner's offline story to carry a message and say what it must say.

## Phase 2: WebUI

- [X] T004 Write `webui/public/manifest.webmanifest`: the name, `standalone`, `start_url` and
      `scope` at `/`, `color_scheme` "dark light", and the four icons.
- [X] T005 Write `webui/public/icons/icon.svg`: the favicon's mark, unchanged, with the dot grid
      wrapped so the script can lift it out.
- [X] T006 Write `webui/scripts/icons.mjs` and add `sharp` as a devDependency; `npm run icons`
      rasterises the two plain PNGs and composes the maskable one (full-bleed ground, the mark
      inside the 80% safe zone).
- [X] T007 Add the head tags: the manifest, the apple touch icon, `theme-color`,
      `color-scheme`, the `apple-mobile-web-app-*` tags, and `viewport-fit=cover`.
- [X] T008 Let the browser chrome follow the scheme: the pre-paint script sets the `theme-color`
      meta before the first paint, and the appearance store sets it again on every change. One
      source for the two grounds (`THEME_COLORS`).
- [X] T009 Write `webui/public/service-worker.js`: the shell precached, navigations network-first,
      the API never cached, no `skipWaiting` on install.
- [X] T010 Write the registration guard (`app/lib/pwa/guard.ts`) and its tests: loopback and
      HTTPS register, a LAN address over plain HTTP and the desktop app do not.
- [X] T011 Export `isDesktop` from the transport, so "are we in the desktop app" has one answer.
- [X] T012 Write `app/lib/pwa/register.ts` and `app/lib/pwa/update.ts`, and wire both into
      `root.tsx`: the new worker is offered as a toast, and only a selection reloads the page.
- [X] T013 Write the offline banner (`app/screens/shared/OfflineBanner.tsx`) and the rule that
      decides when it shows (`app/lib/pwa/connectivity.ts`), with tests for both.
- [X] T014 Mount the banner in `AppShell`, above the page, in both the phone and the desktop
      layout. "Try again" re-reads the browser's answer and asks every lost socket to reconnect.
- [X] T015 Say it on the sign-in screen: without a connection it cannot succeed, and no token
      refresh exists to fake one.
- [X] T016 Say it in the desktop settings: on another device's address the browser refuses the
      worker, so the install is loopback-only.

## Phase 3: Vizier

- [X] T017 Serve a missing file as a 404 and keep the shell for a route: the rule is the last
      path segment (`crates/vizier-http/src/webui/mod.rs`), with unit tests for both rules.
- [X] T018 Say how long each static file may be kept: the shell, the worker and the manifest are
      revalidated; the content-hashed assets are kept for a year.

## Phase 4: The checks and the record

- [X] T019 Write `webui/e2e/pwa.spec.ts`: the manifest is served with its own type, the worker is
      served as JavaScript, and the app renders with the network cut once the worker is in
      control.
- [X] T020 Write the spec folder: `spec.md`, `tasks.md` and the mockup.

## Not done, and why

- **No API caching.** A stale chat is a lie about a live system. The issue's own recommendation,
  and the "last N topics" cache is a follow-up.
- **No install instructions in `docs/`.** The feature has no configuration surface. A follow-up
  if the maintainer wants it.
- **No safe-area tokens.** `viewport-fit=cover` extends the layout into the notch, and `design/`
  has no safe-area tokens. The phone layout is #152.
