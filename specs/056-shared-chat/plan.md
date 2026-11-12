# Implementation Plan: Shared Chat Snapshots

**Branch**: `056-shared-chat` | **Date**: 2026-11-08 | **Spec**: [spec.md](./spec.md)

**Input**: Feature specification from `specs/056-shared-chat/spec.md`

## Summary

A person freezes one conversation into a **snapshot** and shares it as a link (audience: named people, anyone signed in, or anyone) or as a single offline `.vizier.html` file. The **WebUI does the work**: it builds the snapshot from saved history, drops the trail unless asked, removes recalled memory, reasoning, credential-shaped text and server paths, inlines raster media, shows an exact preview, and draws the result through one read-only viewer used by both the route `/share/:id` and the exported file. **Rust stays minimal and passive**: one new permission pair, two tables, one storage trait, one service module, five routes (create, read, list, revoke, people search), an optional-auth middleware, and cache/robots headers. The server stores the document as received and serves it unchanged; it never rewrites it, and the viewer never trusts it. Forking/import is a separate later spec; the snapshot format reserves what it needs.

Technical approach and every trade-off: [research.md](./research.md). Notable departures from issue #187, all in the spec's Clarifications and flagged for Darma: three audiences instead of public-only; two-tier permission names; a 5-minute cache instead of a year-long `immutable` (revocation must work); trail off by default; no fork.

## Technical Context

**Language/Version**: Rust 1.99 (edition per workspace) for `vizier-core` and `vizier-http`; TypeScript/React 19, React Router (SPA, `ssr: false`), Vite, Vitest, Playwright for `webui/`.

**Primary Dependencies**: none new in Rust (`nanoid`, `axum`, `serde_json`, existing `RateLimiter`). WebUI: `vite-plugin-singlefile` as a devDependency (research R9: a hand-rolled inliner of JS, CSS and the chunk graph is more code than the plugin and breaks silently when Vite changes its output); `zod` is already used by the generative-UI catalogue.

**Storage**: SQLite (default, bundled) via `init_share_schema`; PostgreSQL via migration `0007_shared_snapshot.sql`. Both through the backend-neutral SQL layer. No blob store use.

**Testing**: `cargo nextest` for `vizier-core` (storage, service, access) and `vizier-http` (route tests plus conformance vectors, `tests/conformance/groups/shares.rs`); `vitest` for `webui/app/lib/share/**` and viewer components; Playwright `webui/e2e/share.spec.ts` (public link, offline file, hostile fixtures); end-to-end through a `dummyplug` agent per [quickstart.md](./quickstart.md). CI runs the jobs of the paths touched; no new Cargo flag (see Constitution Check).

**Target Platform**: Linux/macOS/Windows server binary, Docker, and the Tauri desktop app (Export only; research R13). Viewer: current Chromium, Firefox and Safari.

**Project Type**: web application inside an existing Rust workspace plus the `webui` submodule (`design` only if a component is missing; none expected).

**Performance Goals**: public read = one primary-key SELECT and a byte copy; 1,000 viewers/min leaves signed-in latency within 10% (SC-012); a typical conversation shares in < 30 s (SC-001) and opens in < 3 s (SC-011); serializing 200 messages + 10 images in the browser in < 2 s on a laptop.

**Constraints**: body ≤ 5 MiB, media ≤ 1 MiB each, 100 media items, 50 named people, 10 people matches and 30 searches a minute, 10 creates/hour/person, 200 active shares/person, 60 misses a minute per address (research R12; FR-012, FR-046 to FR-049); no network request from an exported file; no existing table altered.

**Scale/Scope**: tens of thousands of shares per instance; two tables, ~12 WebUI modules, 5 routes.

**Numbering**: requirement references in this plan and in the other documents use `spec.md` (FR-001 to FR-054, SC-001 to SC-016). Where the plan departs from the spec or leaves a requirement open, `analysis.md` says so.

**Unknowns resolved in research**: where cleaning happens (R1), what is kept (R2), media (R3), storage (R4), viewer safety (R5), embedding (R6), caching vs revocation (R7), anonymous responses (R8), single-file build (R9), permission (R10), ids (R11), limits (R12), desktop (R13). One open technical risk, with a fallback: workers inside `file://` (spike T004).

## Constitution Check

*Gate before Phase 0; re-checked after Phase 1.*

| Rule | Assessment |
|---|---|
| I. Lean | One table pair, one trait, one service file, five routes. The only new dependency is a build-time devDependency. No server-side redaction, no purge job, no expiry, no switch for public sharing (all declined in Clarifications). **Pass.** |
| II. Trait-based, DRY | `ShareStorage` is one trait with one backend-neutral implementation, registered in `VizierStorageProvider`. `Audience::allows` / `cache_policy` are the only places that look at the audience variant; handlers never `match` on it. The link and the file share one viewer component and one schema. **Pass.** |
| III. Self-contained default | SQLite default; PostgreSQL migration added, not required. No CDN, no edge purge, no external service. A Cloudflare rule is an operator option documented in `docs/`, never a dependency. **Pass.** |
| IV. Portability | No OS-specific code; saving the file goes through the existing `transport.saveFile`. **Pass.** |
| V. Errors and logging | `crate::Result` and `ServiceError` (`TooLarge`, `RateLimited`, `Forbidden`, `NotFound` exist); `tracing` for create/revoke (ids only, never content). No `unwrap` outside tests. **Pass.** |
| Single binary | The viewer template is part of `webui/build/client`, embedded by `rust-embed` as today; the viewer build is chained inside `npm run build`, which `build.rs` already runs. **Pass.** |
| Matching submodule branches | `056-shared-chat` exists in the workspace, `webui`, `design`, `dummyplug`. Commit order design → webui → vizier via `just commit`. **Pass.** |
| Design before UI code | Mockups in [mockups/share-ui.md](./mockups/share-ui.md) (ASCII fallback, FigCraft optional); task T007 gets them reviewed before any component. **Pass (pending review).** |
| dummyplug e2e gate | [quickstart.md](./quickstart.md) scripts each check. **Pass.** |
| Feature flags | Not an optional channel/addon/provider/gateway, so no flag. **Pass.** |
| Verify what a change touches | Touches `vizier-core`, `vizier-http`, `webui`; their CI jobs run; `--no-default-features` for core stays green. **Pass.** |
| Conventional commits | All commits `feat(share):` etc.; no `[Spec Kit]` prefixes. **Pass.** |

**Post-design re-check**: unchanged. Two items to watch in review: (a) the optional-auth middleware is new surface in `vizier-http` (kept to one function that reuses `require_auth`'s token parsing, extracted rather than copied); (b) the second Vite config exists because a single-file build cannot share the SPA's code-splitting.

## Project Structure

### Documentation (this feature)

```text
specs/056-shared-chat/
├── plan.md              # This file
├── research.md          # Phase 0
├── data-model.md        # Phase 1
├── quickstart.md        # Phase 1 (dummyplug / curl / Playwright checks)
├── contracts/
│   ├── share-api.md
│   └── snapshot-format.md
├── mockups/share-ui.md  # design-first fallback
├── checklists/requirements.md
├── tasks.md             # /speckit-tasks
└── analysis.md          # consistency analysis: spec vs plan vs tasks vs code (round 2, by hand)
```

### Source Code (repository root)

```text
crates/vizier-core/src/
├── storage/
│   ├── share.rs                 # ShareStorage trait, ShareRecord, Audience
│   ├── sqlite/share.rs          # the single implementation + init_share_schema
│   ├── sqlite/mod.rs            # call init_share_schema from init_schema
│   ├── sql/postgres/migrations/0007_shared_snapshot.sql
│   ├── sql/postgres/migrate.rs  # register 0007
│   ├── mod.rs                   # add ShareStorage to VizierStorageProvider
│   └── user.rs                  # AVAILABLE_PERMISSIONS += *_agents:share_conversations
├── service/
│   ├── shares.rs                # create / read / list / revoke, authorization, limits
│   ├── mod.rs                   # pub mod shares
│   └── users.rs                 # on user delete: delete_shares_by_creator
└── access.rs                    # reuse Caller::manages; no change expected

crates/vizier-http/src/
├── api/v1/shares.rs             # 5 handlers (create, read, list, revoke, people) + utoipa docs + headers
├── api/v1/mod.rs                # routes; GET public with optional_auth; POST with body limit
├── auth/middleware.rs           # optional_auth (token parsing shared with require_auth)
├── state.rs / lib.rs            # share_creates, share_misses RateLimiters on HTTPState
└── webui/mod.rs                 # CSP header for /share/* HTML (research R5, R8)
crates/vizier-http/tests/conformance/groups/shares.rs ; pending.txt

webui/
├── app/lib/share/               # kinds.ts schema.ts serialize.ts redact.ts media.ts
│                                #   preview.ts export.ts embed.ts slug.ts actions.ts
│                                #   fixtures/ (+ hostile/)  *.test.ts
├── app/viewer/                  # main.tsx SnapshotView.tsx (one viewer, two carriers)
├── app/routes/share.tsx         # public route, fetch + 401/404 handling
├── app/layouts/ShareLayout.tsx  # outside RequireAuth
├── app/routes.ts                # route('share/:shareId', …) in a public group
├── app/screens/chat/ShareDialog.tsx        # Link / Offline file tabs, preview (mockup 1)
├── app/screens/settings/MySharesPage.tsx   # list + revoke + admin "All shares" (mockup 3)
├── app/api/shares.ts            # typed client (+ openapi.json regeneration)
├── app/lib/permissions.ts       # PERMISSION_GROUPS "Sharing"
├── vite.viewer.config.ts        # single-file build of app/viewer → build/client/viewer-template.html
├── package.json                 # build script chains the viewer build; devDep vite-plugin-singlefile
└── e2e/share.spec.ts

docs/src/ (api-integration/shares.md, configuration/sharing.md)   # contract, privacy notes, Cloudflare rule
CLAUDE.md / AGENTS.md            # "Shared chat" paragraph (at implementation) and the SPECKIT pointer
```

**Structure Decision**: follow the 054 shape: storage trait + single sqlite/sql implementation + service + thin HTTP handlers in the Rust crates; everything with judgement (what to keep, what to remove, how to draw) in `webui/app/lib/share/` and `webui/app/viewer/`. The route file is `share.tsx` (not the issue's `share.$shareId.tsx`) because the repo declares routes explicitly in `app/routes.ts`.

## Phasing (what ships when)

1. **MVP = US1 + US2**, public audience only: tables, trait, service, POST/GET, serializer + redaction + preview, viewer + route.
2. **US3**: audiences, optional-auth, 401/404 rules, permission editor entry.
3. **US5**: list/revoke + admin list.
4. **US4**: single-file export (needs the viewer from phase 1; spike T004 runs early because it may force a design change).

Each phase is releasable alone; export does not block links.

## Risks

| Risk | Handling |
|---|---|
| Redaction misses a secret shape | Allow-list of kinds (R2) so unknown data is excluded by default; a pattern table with a test per vendor shape; the preview shows counts; the dialog says to review before sharing. |
| Viewer XSS | Layers in R5; hostile fixtures run in CI for both carriers. |
| `file://` worker/iframe limits | Spike T004; fallback disables `run` logic in exports only. |
| Export size (three) | Measure; two templates (with/without three) if needed. |
| 5 MiB cap versus image-heavy chats | Preview refuses with the size and options; re-encode to WebP first; the cap is a constant. |
| `DefaultBodyLimit::disable()` at the router root | Explicit 413 conformance test. |
| Specific-people links need sign-in return | `returnTo` plumbing exists; the global 401 handler skips `/share/*` so the link survives (T102); e2e covers it. |
| Cleaning happens in the browser, so a direct request can publish uncleaned text (FR-051) | Accepted for v1 (research R1, `analysis.md` A1); a server-side credential backstop that rejects, not rewrites, can be added without changing the contract. |
| Requirements left open (FR-007 author name, FR-023, FR-041, FR-044, FR-046 file limit, FR-047 total, FR-052) | Listed with reasons in `tasks.md` Phase 9; none is silently dropped. |
| Desktop has no reachable address | Export-only there (R13); stated in the dialog. |

## Complexity Tracking

No constitution violations to justify.
