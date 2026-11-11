---
description: "Task list for Shared Chat Snapshots"
---

# Tasks: Shared Chat Snapshots

**Input**: Design documents from `specs/056-shared-chat/`

**Prerequisites**: [plan.md](./plan.md), [spec.md](./spec.md), [research.md](./research.md), [data-model.md](./data-model.md), [contracts/](./contracts/), [quickstart.md](./quickstart.md), [mockups/share-ui.md](./mockups/share-ui.md)

**Tests**: Included, and written before the code they cover. The feature is a security boundary, and the constitution requires the dummyplug end-to-end checks for an HTTP/WebUI change. Section "Test plan" lists the user-facing cases; each is owned by a task.

**Organization**: grouped by user story. US1 + US2 together are the MVP (a link must never ship without its cleaning). Do not release US1 alone.

## Format: `[ID] [P?] [Story] Description`

- **[P]**: can run in parallel (different files, no dependency on an incomplete task)
- **[Story]**: the user story the task serves (US1 public link, US2 what leaves, US3 audiences, US4 offline file, US5 list and revoke)

## Path Conventions

- Engine: `crates/vizier-core/src/`; HTTP: `crates/vizier-http/src/`; HTTP tests: `crates/vizier-http/tests/`
- WebUI is a submodule: `webui/app/...`. Commit order design → webui → vizier, with `just commit "<message>"`
- Decision references such as R7 are in `research.md`; contracts are in `contracts/`

## Rules that apply to every task

- Errors are `crate::Result` / `ServiceError`; logging is `tracing` only and **never logs snapshot content**, titles or people (ids only); no `unwrap()` outside tests.
- No `match` on the audience variant outside `Audience::allows` and `Audience::cache_policy`.
- Do not write `Utc::now` or `DateTime<Utc>`; use `crate::time`.
- The server never rewrites a snapshot; the viewer never trusts one.
- Keep `cargo test -p vizier-core --no-default-features` and `cd webui && npm run typecheck` passing.
- UI tasks (T036, T048, T058, T066, T076) wait for T007 (reviewed mockups).

---

## Phase 1: Setup

**Purpose**: Build plumbing, fixtures, and the two early risk spikes.

- [x] T001 Add `vite-plugin-singlefile` to `webui/package.json` devDependencies and create `webui/vite.viewer.config.ts` (single entry `app/viewer/main.tsx`, `inlineDynamicImports`, `cssCodeSplit: false`, output `build/client/viewer-template.html`); chain it after the SPA build in the `build` script so `crates/vizier-http/build.rs` produces it with no change
- [X] T002 [P] Create `webui/app/lib/share/limits.ts` with every constant of research R12 (body 5 MiB, media 1 MiB, 100 items, title 120, people 50, turns 5,000, parts 64, text 256 KiB) and the Rust twin as consts at the top of `crates/vizier-core/src/service/shares.rs` (stub file; registered in T015)
- [X] T003 [P] Create the seeded conversation fixture `webui/app/lib/share/fixtures/conversation.json` covering every history kind (request with attachment, response, tool call/result, `ui_render` with image + Scene3D + Canvas2D, question set, sensitive ask, recall, checkpoint, command, reasoning) and 10 planted secrets (provider keys, AWS id, bearer token, PEM block, JWT) and paths (`/home/alice/.vizier/…`, `C:\Users\alice\…`)
- [x] T004 Spike (time-boxed, write findings to `specs/056-shared-chat/spike-notes.md`): in Chromium, Firefox and WebKit via Playwright, open a `file://` page containing a sandboxed `srcdoc` iframe that starts a Blob `Worker` (the generative-UI logic runner); and build a hello-world viewer with `three` through T001's config and record the file size. Decide: keep `run` in exports or disable it; one template or two (with/without three); fix the export CSP string for `contracts/snapshot-format.md`
- [X] T005 [P] Create hostile fixtures under `webui/app/lib/share/fixtures/hostile/` (`script-tag.json`, `onerror.json`, `js-link.json`, `data-block-breakout.json` with `</script><script>` and `<!--<script>`, `svg-media.json` (image/svg+xml with script), `css-remote.json`, `separators.json` (U+2028/2029), `huge-string.json`, `deep-nesting.json`, `unknown-kind.json`, `future-version.json`, `damaged.vizier.html`)
- [X] T006 [P] Create a unicode fixture `webui/app/lib/share/fixtures/unicode.json` (RTL text, combining marks, zero-width joiners, emoji ZWJ sequences, lone surrogate escapes, NUL-like `\u0000`, 100 KB unbroken line, title with markup)
- [ ] T007 Get `specs/056-shared-chat/mockups/share-ui.md` reviewed by the maintainers (or re-drawn in FigCraft if reachable; do not block or retry if it is not). Record the outcome at the bottom of the mockup file. UI tasks (T036, T048, T058, T066, T076) do not start before this

---

## Phase 2: Foundational (blocking)

**Purpose**: Permission, storage, optional sign-in, schema, and the allow-list that all stories stand on. No story starts before this phase is done.

**⚠️ CRITICAL**

- [X] T008 [P] Add `"all_agents:share_conversations"` and `"owned_agents:share_conversations"` to `AVAILABLE_PERMISSIONS` in `crates/vizier-core/src/storage/user.rs`; no default role gains them (R10); extend the existing permissions test so a role cannot be saved with an unknown name and these two are accepted
- [X] T009 [P] Add the "Sharing" group to `PERMISSION_GROUPS` in `webui/app/lib/permissions.ts` with labels "Share conversations with agents they own" / "Share conversations with any agent they can see", and cover it in `webui/app/lib/permissions.test.ts`
- [X] T010 Create `crates/vizier-core/src/storage/share.rs`: `Audience` enum (`Specific`, `SignedIn`, `Public`) with `allows(viewer, named) -> Access` and `cache_policy()`, `ShareRecord`, `ShareContent`, and the `ShareStorage` trait (`insert_share`, `get_share`, `list_shares_by_creator`, `list_all_shares`, `revoke_share`, `count_active_by_creator`, `delete_shares_by_creator`) per `data-model.md`; unit tests for `Audience::allows` over the full 3 × 4 table plus a disabled and a deleted named person
- [X] T011 Create `crates/vizier-core/src/storage/sqlite/share.rs` with `init_share_schema` (both tables and indexes of `data-model.md`, `CREATE TABLE IF NOT EXISTS`) and the single implementation of `ShareStorage` against the backend-neutral `Connection`; call `init_share_schema` from `init_schema` in `crates/vizier-core/src/storage/sqlite/mod.rs` and declare `mod share;`
- [X] T012 [P] Create `crates/vizier-core/src/storage/sql/postgres/migrations/0007_shared_snapshot.sql` (same columns, `TEXT COLLATE "C"` keys, `BIGINT` times, `ON DELETE CASCADE`) and register it in `crates/vizier-core/src/storage/sql/postgres/migrate.rs`
- [X] T013 Register `ShareStorage` in the `VizierStorageProvider` supertrait list and `pub mod share;` in `crates/vizier-core/src/storage/mod.rs`; make `SqlStore` compile for both backends
- [X] T014 Storage tests in `crates/vizier-core/src/storage/sqlite/share.rs` (and the shared SQL tests in `crates/vizier-core/src/storage/sql/tests.rs`, run on PostgreSQL when `VIZIER_TEST_POSTGRES_URL` is set): insert + get round trip keeps `snapshot_json` byte-identical; list never selects content; revoke is conditional and idempotent; audience rows cascade; `count_active_by_creator` ignores revoked; two parallel inserts at the cap leave at most the cap; the schema check (`expected_schema`) still passes; **existing tables unchanged** (SC-009: snapshot the `sqlite_master` rows of every other table before/after `init_share_schema`)
- [X] T015 Create `crates/vizier-core/src/service/shares.rs` (consts from T002, `new_share_id()` = `nanoid!(21)`, `is_valid_share_id`, `ServiceError` mappings) and add `pub mod shares;` to `crates/vizier-core/src/service/mod.rs`; unit tests for the id validator (22 chars, 20 chars, `../`, `%00`, unicode look-alikes, empty)
- [X] T016 [P] Add `share_creates` (10 per hour) and `share_misses` (30 per minute) `RateLimiter`s to `HTTPState` in `crates/vizier-http/src/state.rs`, construct them in `crates/vizier-http/src/lib.rs` `new_state`, and update the other constructor of the state in `crates/vizier-http/src/api/v1/notifications.rs` tests
- [X] T017 Add `optional_auth` to `crates/vizier-http/src/auth/middleware.rs` by extracting `require_auth`'s credential parsing into a shared function (no copy); it inserts `Option<Principal>` and never rejects; desktop mode yields the desktop principal; tests: no header, bad token, expired token (treated as anonymous), valid token, desktop
- [X] T018 [P] Create `webui/app/lib/share/schema.ts`: the zod schema of `contracts/snapshot-format.md` v1 (strict, caps from `limits.ts`, unknown keys dropped, unknown part kinds → `placeholder`, `format_version` check returning `needs_newer` / `damaged`), with `schema.test.ts` over the fixtures from T003, T005, T006
- [X] T019 [P] Create `webui/app/lib/share/kinds.ts`: the allow-list table of research R2 (`kind → trail-off | trail-on | never`), unknown kinds default to `never`; `kinds.test.ts` fails when a `SessionHistoryContent` variant known to `webui/app/api/frames.ts` is not listed
- [X] T020 [P] Create `webui/app/api/shares.ts` typed client for `contracts/share-api.md` (create, read, list, revoke, people search) using the existing transport; regenerate `webui/openapi.json` after T028

**Checkpoint**: storage, permission, optional sign-in, schema and allow-list exist; stories can start.

---

## Phase 3: User Story 1 - Share a conversation as a public link (Priority: P1) 🎯 MVP (with US2)

**Goal**: A permitted person previews, creates, and copies a link; anyone opens it and sees text, images, 3D, 2D and interactive pages, read-only.

**Independent Test**: [quickstart.md](./quickstart.md) §1 and §4 with the audience set to public.

### Tests for User Story 1 (write first, expect failure)

- [X] T021 [P] [US1] Service tests in `crates/vizier-core/src/service/shares.rs`: create by owner with `owned_agents:share_conversations`; refused without it; refused for an agent they do not own with only the owned tier; allowed with the all tier; system bypass; read returns stored bytes unchanged; id collision retried once
- [X] T022 [P] [US1] Route tests in `crates/vizier-http/tests/shares.rs`: POST 201 then anonymous GET 200 with `Cache-Control: public, max-age=300, s-maxage=300`, no `immutable`, `ETag`, `X-Robots-Tag: noindex, nofollow`, `Referrer-Policy: no-referrer`, `X-Content-Type-Options: nosniff`; `If-None-Match` → 304; body byte-identical to what was posted; no viewer-specific field
- [X] T023 [P] [US1] Negative route tests in `crates/vizier-http/tests/shares.rs`: body 5 MiB + 1 → 413 (proves the route limit beats the router-wide `DefaultBodyLimit::disable()`); invalid JSON → 400; `format_version: 2` → 400; title 121 chars → 400; unknown top-level key handled per contract; empty `turns` → 400; 11th create in an hour → 429 with `retry_after_secs`; 201st active share → 429
- [X] T024 [P] [US1] Conformance vectors in `crates/vizier-http/tests/conformance/groups/shares.rs` for POST/list/DELETE through router and dispatcher, and add `/shares/{share_id}` (GET) with the reason to `crates/vizier-http/tests/conformance/pending.txt` *(vektor POST + pending GET selesai; vektor `GET /shares` dan `DELETE` menunggu rute T064)*
- [x] T025 [P] [US1] Viewer tests in `webui/app/viewer/SnapshotView.test.tsx`: renders text, markdown, an image from `media`, a `ui` part (using `app/test/oneReact.ts`), a Scene3D part, a placeholder; has no composer, no send, no reaction, no delete

### Implementation for User Story 1

- [X] T026 [US1] Implement `create` in `crates/vizier-core/src/service/shares.rs`: permission via `Caller::manages`, body structural checks (parses, version, known top-level keys, size, title, `people` rules), id, cap inside the insert transaction, one `tracing::info!` with ids only (depends on T010–T015)
- [X] T027 [US1] Implement `read` for the public audience in `crates/vizier-core/src/service/shares.rs`: validate id, load, `Access` via `Audience::allows`, return stored text plus `cache_policy` (depends on T026)
- [X] T028 [US1] Create `crates/vizier-http/src/api/v1/shares.rs`: `POST /shares` and `GET /shares/{share_id}` handlers with `#[utoipa::path]`, the response headers of the contract, ETag/304, and the raw stored text embedded in the envelope without parsing it into a value; register the module in `crates/vizier-http/src/api/v1/mod.rs`
- [X] T029 [US1] Wire routes in `crates/vizier-http/src/api/v1/mod.rs`: POST under `require_auth` with `DefaultBodyLimit::max(5 MiB)` and the `share_creates` limiter; GET with `optional_auth` and no `require_auth`
- [X] T030 [P] [US1] Create `webui/app/lib/share/serialize.ts`: saved history (read **all** pages of `GET …/topic/{id}/history` until exhausted, bounded by the 5,000-turn cap with a refusal message) and session files → snapshot v1 using `kinds.ts`; turn grouping; question sets as read-only cards; `ui_render` calls become `ui` parts with the saved state as initial state; the redaction hook is a no-op until T045
- [X] T031 [P] [US1] Create `webui/app/lib/share/media.ts`: collect names with `fileReferences` and attachments, fetch each file, allow-list raster MIME, re-encode to WebP once when over 1 MiB, EXIF dropped by the re-encode, otherwise placeholder (`oversized`, `missing`, `unsupported`); output the `media` map and rewrite spec file names to keys
- [X] T032 [P] [US1] Create `webui/app/lib/share/preview.ts`: counts (messages, images, interactive pages, size, removals by kind), size-limit refusal with the options that reduce it, "turn in progress not included" note
- [x] T033 [US1] Create `webui/app/viewer/SnapshotView.tsx`: header, turns, text through `react-markdown` + `remark-gfm` + `remark-math` + `rehype-katex` with no `rehype-raw`, links through `app/lib/generativeUi/links.ts` with `rel="noopener noreferrer nofollow"`, read-only footer line (mockup 2)
- [x] T034 [US1] Extend the viewer for media and `ui` parts: images only from the `media` map via an `<img>` with allow-listed MIME; `ui` parts revalidated by `generativeUi/validate.ts` and drawn by the existing registry with a media resolver that reads the map instead of `transport.assetUrl`; `share/actions.ts` makes `sendMessage` visibly inert and keeps local state actions; Scene3D stays lazy in the route build
- [x] T035 [US1] Add the route: `route('share/:shareId', 'routes/share.tsx')` in a public group of `webui/app/routes.ts` with `webui/app/layouts/ShareLayout.tsx` (no `RequireAuth`, no app shell); `webui/app/routes/share.tsx` fetches, validates with `schema.ts`, shows loading, the neutral page, and the damaged page; sets `<title>` as text only
- [x] T036 [US1] Create `webui/app/screens/chat/ShareDialog.tsx` (Link tab only for now, mockup 1) and add the Share button to `webui/app/screens/chat/ConversationHeader.tsx` (shared by agent chats and project chats), hidden without the permission (`webui/app/lib/access.ts` helper + test), disabled with a tooltip on an empty conversation, desktop notice in place of the Link tab
- [x] T037 [P] [US1] Add a Content-Security-Policy and `Referrer-Policy: no-referrer` to the HTML served for `/share/*` in `crates/vizier-http/src/webui/mod.rs` (policy fixed by T004/R5), with a test that other paths are unchanged
- [x] T038 [P] [US1] Write `docs/src/api-integration/shares.md` (the contract, the headers, the Cloudflare "cache eligible" rule for `/api/v1/shares/*`, why there is no `immutable`)
- [ ] T039 [US1] Playwright `webui/e2e/share.spec.ts` scenario 1: create a public link in a seeded conversation, open it in a fresh context, assert text, image, canvas present and orbit changes the frame, tabs switch, no composer, `sendMessage` inert; scenario 2: original conversation deleted after sharing, link unchanged; the same flow from a project chat; the shared page is interactive in under 3 s (SC-002, local)

**Checkpoint**: a public link works end to end. Do not ship before US2.

---

## Phase 4: User Story 2 - Decide what leaves the instance (Priority: P1) 🎯 MVP

**Goal**: What is stored is what was previewed: trail off by default, never recall/reasoning/secrets, credential-shaped text and server paths removed, all counted.

**Independent Test**: [quickstart.md](./quickstart.md) §6.

### Tests for User Story 2 (write first)

- [X] T040 [P] [US2] `webui/app/lib/share/redact.test.ts`: one case per shape (OpenAI, Anthropic, Google, AWS access key id, GitHub `ghp_`/`github_pat_`, Slack, Stripe, JWT, `Authorization: Bearer`, PEM private key block, `password=`/`token=` in URLs and `.env` lines), keys split across lines or inside JSON strings and arguments, plus **no false positives** on git SHAs, UUIDs, base64 image data, ordinary prose with the word "token"
- [X] T041 [P] [US2] `webui/app/lib/share/paths.test.ts`: `/home/alice/x`, `/root/.vizier/…`, `/Users/…`, `C:\Users\…`, `\\server\share\…`, `file:///…`, workspace-relative paths kept, URLs not touched, path inside a code block, path with spaces and unicode
- [X] T042 [P] [US2] `webui/app/lib/share/serialize.test.ts`: the kinds matrix (trail off vs on) over the T003 fixture, including that `Recall`, reasoning, `SensitiveAsk`, `Checkpoint`, `Command` are absent in both modes and `answerer` is removed from question sets
- [X] T043 [P] [US2] `webui/app/lib/share/corpus.test.ts`: generate 50 conversations seeded with random secrets/paths/recalls (fixed seed), serialize with trail on and off, `JSON.stringify` the whole document, assert none of the seeds occurs (SC-003)
- [X] T044 [P] [US2] `webui/app/lib/share/preview.test.ts`: removal counts per kind equal the number of removals made; counts shown before creation equal counts in the stored `removals`; cancel stores nothing (no network call in the cancel path)

### Implementation for User Story 2

- [X] T045 [US2] Create `webui/app/lib/share/redact.ts` (pattern table, marker `[removed: key]`, applied to messages, tool arguments, tool results, titles, alt text, question text) and `paths.ts` (directory part removed, base name kept); wire both into `serialize.ts` with counters
- [X] T046 [US2] Apply the trail switch in `serialize.ts`: default off; trail parts only when on; `include_trail` set from the same flag
- [X] T047 [US2] Add the structural check `422 trail_mismatch` in `create` in `crates/vizier-core/src/service/shares.rs` (flag off but trail parts present) with a test; no content rewriting
- [x] T048 [US2] Extend `ShareDialog.tsx`: trail switch off by default with the explanation text, the "Left out automatically" list with counts, warnings for placeholders, the sentence about copies that cannot be taken back, and a "Preview what a visitor sees" button that opens the real viewer on the in-memory snapshot
- [x] T049 [P] [US2] Write `docs/src/configuration/sharing.md`: what is and is not included, how to review, what revoking can and cannot do
- [ ] T050 [US2] Playwright scenario 3 in `webui/e2e/share.spec.ts`: seed secrets via dummyplug, share with trail off and on, fetch the stored document through the API, assert none of the seeds appear; cancel from the preview and assert `GET /shares` is empty

**Checkpoint**: US1 + US2 = MVP, releasable.

---

## Phase 5: User Story 3 - Choose who can open the link (Priority: P2)

**Goal**: Specific people / anyone signed in / anyone, with the access table and neutral responses of R8.

**Independent Test**: [quickstart.md](./quickstart.md) §2.

### Tests for User Story 3 (write first)

- [ ] T051 [P] [US3] Route tests in `crates/vizier-http/tests/shares.rs`: the full audience × viewer table (author, named, other signed-in, no session); `people` required/forbidden by audience; 0, 51, duplicate, unknown, disabled user ids → 400; unknown id and non-public-for-anonymous return the **identical** 401 body; signed-in unknown/revoked/not-named return the identical 404 body; non-public responses are `private, no-store` with `Vary`
- [ ] T052 [P] [US3] Test the miss limiter in `crates/vizier-http/tests/shares.rs`: 31 misses from one address in a minute → 429; hits on real public links are never counted; the window recovers
- [ ] T053 [P] [US3] Service tests in `crates/vizier-core/src/service/shares.rs`: a named person who is later disabled or deleted is refused; the author is allowed on every audience; audience cannot change (there is no update route; assert 404/405 on `PUT /shares/{id}`)
- [ ] T054 [P] [US3] Vitest `webui/app/routes/share.test.tsx`: 401 → redirect to sign-in with `returnTo=/share/<id>`; after sign-in, 200 renders; 404 → neutral page without sign-in button; 429 → "try again later"

### Implementation for User Story 3

- [ ] T055 [US3] Add `GET /shares/people?q=` (signed in, needs a share permission, ≥ 2 characters, ≤ 20 results, enabled users only, returns `user_id` and `username` only) to `crates/vizier-core/src/service/shares.rs`, `crates/vizier-http/src/api/v1/shares.rs` and `contracts/share-api.md`; test that a person without the permission gets 403 and that no email or role leaks
- [ ] T056 [US3] Extend `create` in `crates/vizier-core/src/service/shares.rs` with `people` validation (exists, enabled, 1–50, dedupe) and store audience rows in the same transaction
- [ ] T057 [US3] Extend `read` with the R8 matrix using only `Audience::allows`/`cache_policy`, anonymous → 401 `sign_in_required` for anything that is not an active public share, signed-in non-allowed → 404, and count misses on `share_misses` keyed by client address
- [ ] T058 [US3] Add the audience selector and people picker (debounced `people` search) to `ShareDialog.tsx` per mockup 1; the choice is shown in the result and cannot be changed afterwards
- [ ] T059 [US3] Implement the 401 → sign-in → `returnTo` flow in `webui/app/routes/share.tsx` using `lib/returnTo.ts`
- [ ] T060 [US3] Playwright scenario 4 in `webui/e2e/share.spec.ts`: three shares of one conversation, opened as author, named person, other signed-in person and anonymous; assert the 3 × 4 outcomes and that the unknown and restricted anonymous pages are indistinguishable

**Checkpoint**: audiences work.

---

## Phase 6: User Story 5 - See and revoke what I have shared (Priority: P2)

**Goal**: A person lists and revokes their shares; an administrator lists all and revokes any; cleanup on account deletion.

**Independent Test**: [quickstart.md](./quickstart.md) §1 (revoke) and the My shares page.

### Tests for User Story 5 (write first)

- [ ] T061 [P] [US5] Route tests in `crates/vizier-http/tests/shares.rs`: `GET /shares` returns only the caller's, newest first, never content, with `removals` and `people_count`; `all=true` refused for a normal user, allowed for system and `all_agents:share_conversations`, adds `creator_name`; DELETE by author 204, by admin 204, by someone else 404, repeated 204, unknown 404; after DELETE an anonymous GET is 401 and a signed-in GET is 404 immediately at the origin
- [ ] T062 [P] [US5] Service/storage tests in `crates/vizier-core/src/service/shares.rs`: deleting a user deletes their shares (US5.6) and their audience rows; deleting the source topic or agent does not touch shares (US5.5); revoked shares do not count toward the 200 cap
- [ ] T063 [P] [US5] Cache-window test in `crates/vizier-http/tests/shares.rs`: public responses never carry `immutable` or `max-age` above 300; a revoked share served from a simulated shared cache stops within 300 s (assert the header contract)

### Implementation for User Story 5

- [ ] T064 [US5] Implement `list_mine`, `list_all` (paged) and `revoke` in `crates/vizier-core/src/service/shares.rs` and the handlers `GET /shares`, `DELETE /shares/{share_id}` in `crates/vizier-http/src/api/v1/shares.rs`, with routes under `require_auth`
- [ ] T065 [US5] Call `delete_shares_by_creator` from the user-deletion path in `crates/vizier-core/src/service/users.rs`
- [ ] T066 [US5] Create `webui/app/screens/settings/MySharesPage.tsx` (mockup 3: title, audience, trail, created, copy, revoke with confirm; admin "All shares" with author column), route and entry in `webui/app/screens/settings/SettingsSidebar.tsx` shown only with the permission
- [ ] T067 [US5] Playwright scenario 5 in `webui/e2e/share.spec.ts`: create, list, revoke, open from a new context; admin revokes someone else's; a non-admin cannot see the "All shares" toggle

**Checkpoint**: shares can be undone.

---

## Phase 7: User Story 4 - Export one file that opens offline (Priority: P2)

**Goal**: One `.vizier.html` with the same preview and cleaning, usable with the network off, safe when hand-edited.

**Independent Test**: [quickstart.md](./quickstart.md) §4 (offline) and §5.

### Tests for User Story 4 (write first)

- [ ] T068 [P] [US4] `webui/app/lib/share/embed.test.ts`: `embedJson` output never contains `</script`, `<!--`, `<script`, raw `<`, `>`, `&`, U+2028, U+2029; `JSON.parse` of the extracted text equals the input; 5 MiB random-Unicode round trip; hostile fixtures survive embedding
- [ ] T069 [P] [US4] `webui/app/lib/share/export.test.ts`: the produced file contains exactly one snapshot tag and one viewer script, no `http://`/`https://` URL outside text content, no `<link>`, `@import`, `url(http`, `<base>`, `<form>`; the title is HTML-escaped; the file name rules (slug, 60 chars, unicode title, empty title → `conversation`, path separators and reserved Windows names removed)
- [ ] T070 [P] [US4] `webui/app/viewer/main.test.tsx`: reading the embedded tag with damaged JSON, missing tag, schema failure, future `format_version`, and the hostile fixtures renders only the damaged / needs-newer / inert output and executes nothing (`window.__pwned` undefined)
- [ ] T071 [P] [US4] Playwright scenario 6 in `webui/e2e/share.spec.ts`: export in the WebUI, open the file by `file://` with the context offline, assert content, 3D interaction, zero requests (including none to `/api/v1/shares`, FR-031), zero console errors; scenario 7: open each file of `fixtures/hostile/` as a `.vizier.html` and assert nothing runs and no outbound request is made

### Implementation for User Story 4

- [ ] T072 [US4] Implement `webui/app/viewer/main.tsx`: read `#vizier-snapshot` with `textContent`, parse, validate with `schema.ts`, mount `SnapshotView`; damaged and needs-newer pages (mockup 2); CSP meta from T004
- [ ] T073 [US4] Finish `webui/vite.viewer.config.ts` and `package.json` so `npm run build` leaves `build/client/viewer-template.html` with a single marker; implement the two-template variant only if T004 showed the three bundle is too heavy
- [ ] T074 [P] [US4] Create `webui/app/lib/share/embed.ts` (`embedJson`) and `slug.ts`
- [ ] T075 [US4] Create `webui/app/lib/share/export.ts`: fetch the template, substitute marker with the escaped snapshot, escape the title, return a Blob; save through `transport.saveFile` (desktop) or an anchor download (browser)
- [ ] T076 [US4] Add the "Offline file" tab to `ShareDialog.tsx` with the same preview and removals, no audience controls, and a line saying nothing is stored on the server; wire `run` logic per T004's decision
- [ ] T077 [US4] Verify `crates/vizier-http` serves `viewer-template.html` from the embedded assets unchanged and that the binary build (`cargo build -p vizier`) includes it (manual run of the binary, per the constitution's build-pipeline rule)

**Checkpoint**: all five stories work.

---

## Phase 8: Polish and cross-cutting

- [ ] T078 [P] Accessibility: dialog focus trap, labels, keyboard path for the people picker, viewer landmarks and contrast; add cases to `webui/e2e/a11y.spec.ts` and `webui/e2e/mobile.spec.ts`
- [ ] T079 [P] Add a "Shared chat" paragraph to `CLAUDE.md` (architecture, where the pieces live, the security layers, the open import spec) — `AGENTS.md` is a symlink to it, so edit `CLAUDE.md` only
- [ ] T080 [P] Add the `docs/src/SUMMARY.md` entries for the two new pages and run `just landing-build`
- [ ] T081 Run the whole `specs/056-shared-chat/quickstart.md` (and time SC-001: Share → copied link under 30 s for a 200-message, 10-image chat) against a binary with a dummyplug agent (`just run-d`), including §7 on a copy of an existing workspace database and on PostgreSQL, and §8 with `oha`; record results at the bottom of the quickstart
- [ ] T082 In the repository root run `cargo clippy --workspace`, `cargo nextest run -p vizier-core -p vizier-http`, `cargo test -p vizier-core --no-default-features`, `cd webui && npm run typecheck && npm test && npx playwright test e2e/share.spec.ts`, and `cd app && cargo test` (desktop dispatcher parity); fix what they show
- [ ] T083 Review the open decisions in `spec.md` Clarifications with the maintainers and update the spec and docs for whatever they change; commit with `just commit "feat(share): …"` in order design → webui → vizier

---

## Requirement traceability

| Requirement | Tasks |
|---|---|
| FR-001 share from agent and project chats | T036, T039 |
| FR-002 preview contents | T032, T044, T048 |
| FR-003 frozen | T039, T062 |
| FR-004, FR-006 content, format version | T018, T030, T034 |
| FR-005 media inlined, placeholders | T031, T043 |
| FR-007, FR-008 empty / over size | T023, T032, T036 |
| FR-009 trail off by default | T042, T046, T048 |
| FR-010 to FR-014 what is left out | T019, T040–T045, T047 |
| FR-015, FR-016, FR-017 audiences, neutral answers, no widening | T010, T051, T053, T056, T057 |
| FR-018 permission | T008, T009, T021, T036 |
| FR-019, FR-020 revoke, lists, admin | T061, T064, T066, T067 |
| FR-021 unguessable, noindex | T015, T022, T052 |
| FR-022 rate limit | T016, T023 |
| FR-023 permission is the only control | no code; documented in T049 |
| FR-024 to FR-028 viewing safely | T018, T033–T035, T054, T068, T070 |
| FR-029 to FR-032 offline file | T068–T077 |
| FR-033 revocation window, cache | T022, T063, T081 |
| FR-034 many viewers | T022, T081 (§8 load) |
| FR-035 desktop | T036, T075, T076, T082 |
| FR-036 no change to existing data | T014, T081 |
| SC-001, SC-002 | T039, T081 |
| SC-003 | T043 |
| SC-004 | T005, T070, T071 |
| SC-005 | T061, T063 |
| SC-006 | T071 |
| SC-007 | T081 |
| SC-008 | T051, T060 |
| SC-009 | T014, T081 |

## Dependencies and order

- Phase 1 → Phase 2 → stories. T004 (spike) can run while Phase 2 starts and must finish before T037, T072, T073, T076.
- US1 needs Phase 2. US2 needs US1's `serialize.ts` (T030) and `ShareDialog.tsx` (T036). **US1 + US2 ship together.**
- US3 needs US1 (read path). US5 needs US1 (storage/service) and is independent of US3. US4 needs US1's viewer (T033, T034) and US2's serializer; it does not need US3 or US5.
- Within a story: tests, then storage/service, then HTTP, then WebUI, then e2e.

## Parallel opportunities

- Phase 1: T002, T003, T005, T006 in parallel.
- Phase 2: T008, T009, T012, T016, T018, T019, T020 in parallel; T010 → T011 → T013 → T014.
- US1: T021–T025 together; T030, T031, T032 together while T026–T029 proceed in Rust.
- US2: T040–T044 together.
- US3, US5, US4 can be staffed in parallel after US1+US2.

## Implementation strategy

1. Phases 1–2, spike T004 early.
2. US1 + US2 → release candidate for public links (MVP).
3. US5 (undo) before US3 if time is short: the ability to revoke matters more than narrower audiences, but the maintainer asked for audiences, so both ship before 1.0 of this feature.
4. US4 last; a failed spike changes only the export, not the link.

---

## Test plan (from a Vizier user's point of view)

Types: **P** positive, **N** negative, **A** anomaly. "Owner" task = the task whose tests or e2e step covers it.

### Sharing a link (US1)

| ID | Type | Who does what | Expected | Owner |
|---|---|---|---|---|
| TP-01 | P | Alice (owns the agent, has the permission) shares a 20-message chat with an image and a 3D scene, audience anyone | Preview, link created, copied; opens logged out with image, orbitable scene, no composer | T022, T039 |
| TP-02 | P | Alice then keeps chatting and deletes the original chat | The link shows the same content as before | T039 |
| TP-03 | P | Visitor opens a link twice; second request sends `If-None-Match` | 200 then 304; same content | T022 |
| TP-04 | N | Bob (no permission) looks for Share, then calls the API directly | No button; API 403 | T021, T036, T022 |
| TP-05 | N | Alice shares a chat with an agent she neither owns nor can manage, with only the owned tier | 403 | T021 |
| TP-06 | N | Visitor opens `/share/abc`, `/share/<22 chars>`, `/share/../x`, `/share/%00`, an unknown valid-looking id | One neutral page / one response shape every time; no stack, no hint | T015, T022, T051 |
| TP-07 | N | Alice opens Share in an empty chat | Button disabled, reason shown; nothing stored | T036, T044 |
| TP-08 | N | Alice presses "Create link" while the server is down / storage fails | Error shown, no link shown, `GET /shares` has no partial row | T026, T050 |
| TP-09 | A | Chat has a reply still streaming | Snapshot has the saved turns only; preview says so | T032, T044 |
| TP-10 | A | Chat refers to an image whose file was deleted | Labelled placeholder, share succeeds, preview lists it | T031 |
| TP-11 | A | Chat holds 150 MB of images / snapshot would exceed 5 MiB | Refused at preview with size, limit, and the options; nothing truncated silently; direct POST of 5 MiB + 1 → 413 | T023, T032 |
| TP-12 | A | One image is 3 MB | Re-encoded once; if still > 1 MiB a placeholder; listed in preview | T031 |
| TP-13 | A | Title empty, 5,000 characters, or `<img src=x onerror=…>` | Fallback or trimmed to 120; shown as text in tab, page, list and file name | T006, T033, T035 |
| TP-14 | A | Unicode: Arabic/Hebrew RTL, combining marks, ZWJ emoji, lone surrogates, 100 KB unbroken line | Stored and shown intact or harmlessly; layout does not break; JSON round-trips | T006, T018, T033 |
| TP-15 | A | Alice double-clicks Create / presses it in two tabs / two people share the same chat at once | Independent shares with different ids; none lost or overwritten; cap respected | T014, T026 |
| TP-16 | A | Alice creates 11 shares in an hour; 201 active shares | 429 with retry time; earlier shares unaffected | T023 |
| TP-17 | A | Desktop app user opens Share | Link tab explains it needs a reachable server; Offline file works | T036, T076 |

### What leaves (US2)

| ID | Type | Who does what | Expected | Owner |
|---|---|---|---|---|
| TP-20 | P | Share with default settings | Trail off; only dialogue, images and pages; preview says "not included" | T042, T048 |
| TP-21 | P | Switch the trail on | Tool calls/results appear after cleaning; preview says so | T042, T046 |
| TP-22 | N | Tool result contains an `sk-…`, an AWS key, a PEM block, a bearer token | Replaced by a visible marker with the trail on and off; counted | T040, T043 |
| TP-23 | N | Messages contain `/home/alice/.vizier/…` and `C:\Users\alice\…` | Directory part removed; counted | T041 |
| TP-24 | N | A recalled memory passage, reasoning steps, a sensitive-input ask | Never present with the trail on or off | T042, T043 |
| TP-25 | N | Alice cancels from the preview | Nothing stored (no request, no row) | T044, T050 |
| TP-26 | A | A secret is split across two lines or inside a JSON argument | Still replaced | T040 |
| TP-27 | A | Plain text with a git SHA, a UUID, base64 of an image, the word "token" | Not damaged by redaction | T040 |
| TP-28 | A | A new history kind added by a later version | Excluded by default | T019, T042 |
| TP-29 | A | Photo attachment with GPS EXIF | Metadata gone after re-encode | T031 |
| TP-30 | A | A client posts a document with trail parts but `include_trail: false`, or with extra top-level keys | 422 / keys not rendered | T023, T047 |

### XSS and weaponised content (US1, US4)

| ID | Type | Who does what | Expected | Owner |
|---|---|---|---|---|
| TP-40 | A | Message text with `<script>`, `<img onerror>`, `<iframe srcdoc>`, `<svg onload>` | Shown as inert text; nothing runs; no request | T005, T025, T033 |
| TP-41 | A | Markdown link `[x](javascript:alert(1))`, `data:text/html,…`, `vbscript:` | Not a link, or shown as text | T033 |
| TP-42 | A | Message containing `</script><script>alert(1)</script>` and `<!--<script>` in an exported file | Breakout impossible: only `\u003c` escapes in the file | T068, T071 |
| TP-43 | A | Media entry `image/svg+xml` with script, or MIME `text/html` | Placeholder; never rendered | T005, T018 |
| TP-44 | A | Interactive page with `sendMessage`, `run`, an `href` to `http://evil`, a remote image URL, CSS `url(http://…)` | `sendMessage` inert; link follows link rules; remote resources not fetched | T034, T070 |
| TP-45 | A | Interactive page with an infinite loop in `logic` | Worker limit applies; page stays responsive | T034 |
| TP-46 | A | A spec with 10,000 nested elements / 5,000 shapes beyond limits | Rejected by validation, placeholder shown | T005, T018 |

### Audiences, sign-in, revoke (US3, US5)

| ID | Type | Who does what | Expected | Owner |
|---|---|---|---|---|
| TP-50 | P | Alice shares to Bob only; Bob opens signed in | Bob sees it | T051, T060 |
| TP-51 | P | Alice shares to everyone signed in; Carol opens | Carol sees it | T051 |
| TP-52 | P | Bob opens Alice's link while signed out | Asked to sign in, returns to the link, sees it | T054, T059 |
| TP-53 | N | Carol (signed in, not named) opens Bob's link | Neutral "not available"; same as unknown id | T051, T060 |
| TP-54 | N | Anonymous opens a Specific link and an unknown id | Indistinguishable responses | T051 |
| TP-55 | N | Alice tries to name 51 people, an unknown user, a disabled user, nobody | 400 | T051 |
| TP-56 | N | Alice looks for a way to widen the audience | None; only revoke and re-share | T053 |
| TP-57 | N | Bob tries to revoke Alice's share | 404 | T061 |
| TP-58 | P | Alice revokes; admin revokes someone else's | New visitors refused (origin at once, caches ≤ 5 min) | T061, T063, T067 |
| TP-59 | A | Alice clicks Revoke twice | Both succeed; no error | T061 |
| TP-60 | A | Bob is disabled / deleted after being named | Refused | T053 |
| TP-61 | A | Alice's account is deleted | Her shares are revoked | T062 |
| TP-62 | A | Someone scripts 1,000 random ids | Throttled after 30 misses/min per address; real links unaffected | T052 |
| TP-63 | A | Source topic or agent deleted | Share still works | T062 |

### Offline file (US4)

| ID | Type | Who does what | Expected | Owner |
|---|---|---|---|---|
| TP-70 | P | Export a chat with a 3D scene; open by double-click offline | Same content as the link; interaction works; zero requests | T071 |
| TP-71 | P | Export, then the original is deleted and the server is stopped | File still opens | T071 |
| TP-72 | N | Open a file whose JSON was cut in half | "Damaged" page, nothing partially drawn | T070 |
| TP-73 | N | Open a file made by a newer Vizier (`format_version: 2`) | "Needs a newer version" or full render; never half a page | T070 |
| TP-74 | A | Someone edits the file to inject `<script>`, an `onerror`, a `javascript:` link, an SVG media entry | Same protection as a link; nothing runs | T071 (scenario 7) |
| TP-75 | A | Edit removes the `vizier-snapshot` tag, or adds a second one | Damaged page; the first valid tag is not silently preferred over a second | T070 |
| TP-76 | A | Title with emoji/RTL/`../` | Safe file name; correct `<title>` | T069 |
| TP-77 | A | Save a 5 MiB conversation | File size ≈ snapshot + viewer (≤ the measured budget from T004); opens in < 3 s | T004, T071 |
| TP-78 | A | Open the file in Safari / Firefox | Works, or logic disabled with the notice (per T004) | T004 |

### Existing installs

| ID | Type | Who does what | Expected | Owner |
|---|---|---|---|---|
| TP-90 | P | Upgrade a workspace with data | Boots; new tables created; old tables identical (SC-009) | T014, T081 |
| TP-91 | P | Same on PostgreSQL | Migration 0007 applies | T081 |
| TP-92 | A | Downgrade (old binary on new database) | Starts; the extra tables are ignored | T081 |
| TP-93 | A | Role editor opened | New permission appears under "Sharing"; no default role has it | T008, T009 |
