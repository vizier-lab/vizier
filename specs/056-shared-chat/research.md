# Research: Shared Chat Snapshots

Phase 0 of `plan.md`. Each entry: **Decision**, **Rationale**, **Alternatives considered**. "Repo" findings come from reading the code on `056-shared-chat`; "Web" findings come from the sources listed at the end.

## R1. Where the work happens: the WebUI builds and cleans the snapshot; Rust stores and serves it

**Decision**: The WebUI turns saved history into a snapshot, removes what must not leave, inlines media, and checks the size. The server accepts an already-built document, checks only what is cheap and structural (valid JSON, size, format version, title length, audience shape, permission, rate), stores it, and serves it. The server never rewrites a snapshot.

**Rationale**: Issue #187 and the maintainer's principle (heavy work in the WebUI, a passive backend). It is also what makes the preview honest: the person sees the exact bytes the server will keep. The cost is that the server cannot prove the document was cleaned, so the **viewer treats every snapshot as hostile** (R5), which it must do anyway for `.vizier.html` files that anyone can edit.

**Alternatives**: Server-side redaction (rejected: a second implementation of the same rules in Rust, and the preview would no longer be the stored thing). Server-side credential scan as a backstop (deferred: a cheap regex pass that *rejects* rather than rewrites is possible later without changing the contract).

**Repo**: Saved history is already redacted for secrets the turn knew (`SecretRedactor` arms `get_secret` and `ask_sensitive_input` values before a history row is written; CLAUDE.md "Asking the person"). So the WebUI starts from redacted rows. Its own pass adds the shapes the redactor cannot know (a key a user typed into a prompt, a token printed by a shell command).

## R2. What a snapshot is made of (from `SessionHistoryContent`)

**Repo**: `crates/vizier-core/src/schema/history.rs` has `Request`, `Response`, `AssistantMessage`, `ToolCall`, `ToolResult`, `Checkpoint`, `Command`, `ClientToolCall`, `Recall`, `QuestionSet`, `SensitiveAsk`. The generative UI is a `ClientToolCall` of tool `ui_render` whose `arguments` are the json-render spec and whose saved `state` is the page state (CLAUDE.md "Generative UI").

**Decision**: a three-way split by kind, enforced by one allow-list table in `webui/app/lib/share/kinds.ts` (a new kind is **excluded until listed**, so adding a history variant cannot leak by default):

| Kind | Trail off | Trail on | Notes |
|---|---|---|---|
| `Request` (person text, attachments) | in | in | attachments become media |
| `Response` / `AssistantMessage` text | in | in | |
| `ClientToolCall` `ui_render` | in | in | spec revalidated; state kept as initial state |
| `ClientToolCall` other | out | in (args+outcome, cleaned) | |
| `ToolCall` / `ToolResult` | out | in (cleaned) | credentials and paths removed |
| `QuestionSet` | in as a read-only card | in | answers kept, `answerer` removed |
| `SensitiveAsk` | out | out | even a purpose line may identify a secret |
| `Recall` | out | out | the issue and FR-010 |
| `Checkpoint`, `Command` | out | out | internal |
| reasoning / `thinking` | out | out | FR-010 |

**Rationale**: matches FR-004, FR-009, FR-010. The `QuestionSet` row is the one judgement call (a visible conversation turn, not trail).

**Alternatives**: put `QuestionSet` under the trail (rejected: a conversation with a choice card would read as having a hole in it).

## R3. Media: inline as data URIs, raster only, bounded

**Repo**: files are served by `GET /api/v1/files/{file_id}` (`crates/vizier-http/src/api/v1/files.rs`) **with no auth on the route**; generative UI refers to files by *name* (`lib/generativeUi/media.ts`: `findFile`, `fileUrl`, `fileReferences`, with `LIMITS.distinctFiles = 100`, `galleryFiles = 200`).

**Decision**: the serializer collects names through `fileReferences`, fetches each file, and stores a `media` map keyed by name: `{mime, size, data}` with `data` a base64 string (not a `data:` URI string; the viewer builds the URI, so the allow-list lives in one place). Allowed MIME: `image/png`, `image/jpeg`, `image/webp`, `image/gif`. A larger raster image (over 1 MiB) is re-encoded to WebP on a canvas at a lower quality once; if still over, it becomes a placeholder. SVG is **not** inlined as an image (R5). Audio, video and other files become labelled placeholders in this version.

**Rationale**: the issue says "< 1 MB base64". Raster `data:` images in an `<img>` cannot run script. Re-encoding strips EXIF (GPS) from photos, which is a privacy gain.

**Alternatives**: Keep `/api/v1/files/<id>` links (rejected: breaks when the author's storage is cleared, and exposes the file route to the viewer's origin). Store media in a separate table (rejected: a second moving part; 5 MiB cap makes one blob fine). Sanitise SVG with DOMPurify (rejected for v1: attack surface; see Web W5).

**Side finding**: since `/files/{file_id}` needs no auth, file ids are bearer secrets already. Sharing does not change that; it is noted in the risks.

## R4. Storage: one table, one trait, both SQL backends

**Repo**: `AVAILABLE_PERMISSIONS` is in `crates/vizier-core/src/storage/user.rs`; schemas are created by `init_*_schema` calls from `SqlStorage::init_schema` in `storage/sqlite/mod.rs` (CREATE TABLE IF NOT EXISTS), with a PostgreSQL twin in `storage/sql/postgres/migrations/NNNN_*.sql` (latest is `0006_workspace_shown.sql`, so ours is `0007`). Every storage trait has a single implementation written against the backend-neutral `Connection` (spec 045) and is added to the `VizierStorageProvider` supertrait list. Project LSP (054) is the template: `storage/project.rs` trait, `storage/sqlite/project.rs` impl, `service/project_lsp.rs`, migration `0005`.

**Decision**: `storage/share.rs` (`ShareStorage` trait), `storage/sqlite/share.rs` (impl), `init_share_schema`, `0007_shared_snapshot.sql`, `service/shares.rs`. Two tables: `shared_snapshot` and `shared_snapshot_audience` (named people, `ON DELETE CASCADE`). `snapshot_json` is `TEXT` on both backends, never `JSONB`, so the stored bytes are the bytes the person previewed and an ETag is stable. List queries never select `snapshot_json`.

**Rationale**: constitution II/III (trait, SQLite default, Postgres additive). The issue's "single dedicated table" is kept for content; named recipients need a second small table, which is the normal shape rather than a JSON array that cannot be indexed or cascaded.

**Alternatives**: audience list as a JSON column (rejected: no cascade when a user is deleted, no index for "shared with me"). Blob store for content (rejected: forces S3 decisions into the default path).

## R5. Viewer safety: layers, not one filter

The viewer reads a document that may have been edited by anyone (a link's author, or a person who opened a `.vizier.html` in a text editor). Layers, from outside in:

1. **Parse and validate** against a strict zod schema (`share/schema.ts`, `format_version: 1`): unknown fields are dropped, unknown part kinds become a labelled placeholder, string lengths and counts are capped. Failure shows the "damaged file" page and renders nothing (FR-032).
2. **Text is text.** Message bodies go through the existing `react-markdown` pipeline (already in `package.json`: `react-markdown`, `remark-gfm`, `remark-math`, `rehype-katex`) **without** `rehype-raw`, so raw HTML is inert text. Links go through `lib/generativeUi/links.ts` (https/http/mailto only) and open `rel="noopener noreferrer nofollow"` in a new tab.
3. **Images** only from the `media` map, allow-listed MIME, `<img>` only. No `data:` URIs from message text; no remote `src`.
4. **Interactive specs** are revalidated by the existing `generativeUi/validate.ts` against `LIMITS` before drawing; the actions table in `share/actions.ts` makes `sendMessage` inert (visible disabled state) and keeps `setState`/`pushState`/`removeState`/`validateForm`. `run` (logic) keeps working because logic already runs in a worker inside a sandboxed iframe with no DOM, cookies or storage (CLAUDE.md "Generative UI"), but is disabled in an exported file until the spike in R9 shows workers start from a `file://` page.
5. **Embedding**: in the exported file the snapshot sits in `<script type="application/json" id="vizier-snapshot">`. Serialization replaces `<`, `>`, `&`, U+2028 and U+2029 with `\u003c`, `\u003e`, `\u0026`, `\u2028`, `\u2029` (R6). The viewer reads it with `textContent` + `JSON.parse`, never `innerHTML`.
6. **Page policy**: the exported file carries `<meta http-equiv="Content-Security-Policy">` starting from `default-src 'none'; script-src 'unsafe-inline'; style-src 'unsafe-inline'; img-src data: blob:; connect-src 'none'; form-action 'none'; base-uri 'none'` and tightened or loosened only as far as spike T004 shows the viewer needs (inline script is unavoidable in a single file; `worker-src blob:` and `frame-src` are added only if the logic runner stays). The server sends `Referrer-Policy: no-referrer`, `X-Content-Type-Options: nosniff`, `X-Robots-Tag: noindex, nofollow` on `GET /shares/{id}`, and a CSP on the `/share/*` HTML response (R8).

**Rationale**: OWASP's XSS and DOM-XSS cheat sheets say to encode for the context (JavaScript context for the data block, HTML context for text) and not to rely on one sanitiser; a `<script>` data block needs JavaScript-string encoding of `<`; Claude's and ChatGPT's share pages are "snapshot, unlisted, noindex" for the same reasons (Web W1).

**Alternatives**: DOMPurify over message HTML (rejected: there is no message HTML to sanitise if raw HTML is never enabled). Render the whole viewer inside a sandboxed iframe (considered for the exported file; deferred: the generative UI already sandboxes the only code-bearing part, and an outer iframe doubles every height/scroll problem).

## R6. Safe embedding of JSON in HTML

**Decision**: one function `embedJson(value)` in `share/export.ts`: `JSON.stringify` then replace `[<>&\u2028\u2029]` by `\uXXXX` escapes; unit-tested against `</script>`, `<!--`, `<script`, `-->`, `\u2028`, lone surrogates (stringified as escapes by `JSON.stringify` on current engines; the test asserts it) and a 5 MiB random-Unicode round trip. Base64 media contains none of these characters.

**Rationale**: `<!--` followed by `<script` is the other classic breakout in a script data block, besides `</script>`; escaping `<` covers both. `&` is escaped so that a polyglot or an XML-ish consumer does not misread. The issue lists only `<` and `>`; adding `&` and the two line separators costs nothing.

## R7. Caching and revocation

**Web**: Cloudflare does not cache JSON by default (it caches by file extension); an origin `Cache-Control` is honoured only when the response is eligible, so a "cache everything" rule for `/api/v1/shares/*` is an operator step. `immutable` stops browsers revalidating on reload for the whole `max-age`. A response cached with `max-age=31536000` cannot be recalled by `DELETE`.

**Decision**: contradiction in the issue between "revoke a share" and "immutable for a year" resolved in clarify (FR-033): 
- audience *Anyone with the link*: `Cache-Control: public, max-age=300, s-maxage=300, stale-while-revalidate=0` plus a strong `ETag` (`"<share_id>"`, valid because content never changes) and `304` on `If-None-Match`. Revocation is felt within 5 minutes everywhere, within 0 s at the origin.
- other audiences: `Cache-Control: private, no-store`, `Vary: Authorization, Cookie`.
- `404`/`401`: `no-store`.
- No `immutable`.

**Rationale**: SC-005 (5 min), SC-007 (1,000 viewers/min). 1,000 viewers a minute is ~17 req/s, which one SQLite primary-key read per request serves without a cache; the cache is a bonus, not a requirement. The 5 MiB body is the real cost, so the handler streams the stored text without parsing it and without building a `Value`.

**Alternatives**: a year-long immutable cache with revocation by cache purge (rejected: needs an edge API token, an external dependency by Principle III). A versioned URL per share (rejected: does not make revocation faster).

**Flag for Darma**: departs from the issue.

## R8. Public routes, optional sign-in, and what an anonymous caller learns

**Repo**: `require_auth` (`vizier-http/src/auth/middleware.rs`) is all-or-nothing and has a desktop branch. There is no optional-auth middleware. Public routes in `api/v1/mod.rs` (`/ping`, `/auth/login`, `/files/{file_id}`, catalogue logos) simply skip the layer. The embedded WebUI falls back to `index.html` for any unknown path (`webui/mod.rs::assets`), so `/share/<id>` already reaches the SPA with no server change.

**Decision**: add `optional_auth` beside `require_auth` (same token parsing, inserts `Option<Principal>`, never rejects). `GET /shares/{id}`:
- active + audience public: 200 for everyone.
- caller has no session and the share is *not* active-public, **or does not exist**: `401 sign_in_required`, identical body either way. The WebUI sends the person to sign in with `returnTo`, then asks again.
- caller signed in and not allowed, revoked, or unknown: `404`, one body.

**Rationale**: FR-016/FR-028: a probe cannot tell "unknown" from "restricted" or "revoked" without credentials, and with credentials cannot tell them apart either. A named person who opens their link while signed out is asked to sign in instead of being told "not found", which was the failure the first draft of the table had. The spec table is corrected in `/speckit-analyze` (see `analysis.md`).

**Alternatives**: always 404 for anonymous (rejected: a named teammate who is signed out sees "not found"). Per-audience status codes (rejected: leaks the audience of any id that exists).

**Rate limit on misses**: `share_reads` limiter on `HTTPState` keyed by the client address (`ConnectInfo` or the leftmost trusted `X-Forwarded-For`), counted only on 401/404, so viewing a real link is never throttled.

## R9. One viewer, two carriers

**Repo**: the WebUI is a React Router SPA with `ssr: false` (`react-router.config.ts`); `build/client` is ~11 MB with lazily loaded chunks, and `three`/`@react-three/fiber` load through `React.lazy` only from `registry/three.tsx`. `crates/vizier-http/build.rs` runs `npm run build` in `webui/` and embeds `build/client`.

**Decision**: a *viewer* entry (`webui/app/viewer/`) renders `<SnapshotView snapshot={...}/>` and has no router, store, transport or auth. Two carriers feed it:
- the route `/share/:shareId` (`routes/share.tsx`, in a new public layout `layouts/ShareLayout.tsx`, outside `RequireAuth`) which fetches `GET /api/v1/shares/{id}`;
- `viewer.html`, a single-file build of the same entry, produced at `npm run build` into `build/client/viewer-template.html`, with a marker comment where the snapshot is inserted. The Export tab fetches that template, substitutes the escaped JSON, and downloads a Blob named `<title-slug>-<yyyymmdd>.vizier.html`.

Single-file build: `vite-plugin-singlefile` (devDependency of `webui`) with `build.rollupOptions.output.inlineDynamicImports = true`, `cssCodeSplit = false`, `assetsInlineLimit = 100_000_000`, driven by a second config `vite.viewer.config.ts` called from the `build` script. The `three` chunk is part of the single file (it cannot be lazy in a file with no server); the cost is ~1 MB gzip-able of inline script in every export (measured in T-spike, see tasks). If the export is too heavy, the template is built twice (with and without three) and the exporter picks by whether the snapshot contains a `Scene3D`.

**Rationale**: DRY (constitution II): the link and the file draw through the same component, so SC "both show the same content" holds by construction. The template is a static asset the WebUI already serves, so the Rust side learns nothing about export (FR-031).

**Alternatives**: generate the file server-side (rejected: Rust would need the bundle and templating; contradicts R1). Reuse the full SPA build as the file (rejected: 11 MB, router and auth assumptions). Service worker/PWA (rejected: needs a server).

**Open risk** → spike T004: a `file://` page starting a `Worker` from a Blob inside a sandboxed `srcdoc` iframe (the generative UI's logic runner). If it does not start in Chromium, Firefox and Safari, `run` is disabled in exports and the interface shows the existing "no logic" notice; static/stateful pages still work.

## R10. Permission

**Repo**: `AVAILABLE_PERMISSIONS` lists two-tier names (`all_agents:view_artifacts` / `owned_agents:view_artifacts`); `access::Caller::manages(config, all_perm, owned_perm)` already decides "system, all, or owner" for objects of one agent; the WebUI mirrors names in `webui/app/lib/permissions.ts` (`PERMISSION_GROUPS`).

**Decision**: `all_agents:share_conversations` and `owned_agents:share_conversations`, checked with `Caller::manages`. Add to `AVAILABLE_PERMISSIONS`, `PERMISSION_GROUPS` (new group "Sharing"), and the role editor. Default roles: **no existing role gains it** (an upgrade must not silently make every user able to publish); the first-run admin role is system and has it implicitly.

**Alternatives**: a single `conversations:share` (rejected for consistency; flagged for Darma because the issue wrote `:share_conversations`).

## R11. Identifiers

**Repo**: `nanoid = "0.5.0"` is already a workspace dependency of `vizier-core`.

**Decision**: `nanoid!(21)` with the default URL-safe alphabet (64 symbols, ~126 bits). Path parameter is validated against `^[A-Za-z0-9_-]{21}$` before any query; anything else is the same neutral response as an unknown id.

**Rationale**: unguessable and un-enumerable (FR-021); URL-safe for the link; no new dependency. Collision on insert is a primary-key conflict that the service retries once.

## R12. Limits (defaults; all constants in one module)

| Limit | Value | Where enforced |
|---|---|---|
| Snapshot body | 5 MiB (`DefaultBodyLimit` on the POST route, plus a check on the stored text) | server, WebUI preview |
| Per media item | 1 MiB raw after re-encode | WebUI |
| Media items | 100 (`LIMITS.distinctFiles`) | WebUI |
| Title | 120 chars | both |
| Named people | 50 | both |
| Create rate | 10 per hour per person | server (`RateLimiter`) |
| Active shares per person | 200 | server |
| Miss rate | 30 per minute per address | server |

**Rationale**: storage DoS (issue) with bounded numbers; revisited in tasks if the dummyplug run shows a typical conversation does not fit.

**Repo gotcha**: `lib.rs` and `embedded.rs` call `DefaultBodyLimit::disable()` on the whole router. The route-level layer on `POST /shares` is the innermost and wins in axum, but the plan requires a conformance test that sends 5 MiB + 1 and expects `413` rather than trusting it.

## R13. Desktop

**Repo**: the desktop app hosts the runtime in-process and dispatches requests with no socket (`embedded.rs`, `AuthMode::Desktop`); there is no address anyone else can open.

**Decision**: in the desktop app the Share dialog offers **Export** only for the audience-link tab (the link tab says why and points to a server). The same list/revoke code works if the person's server is reachable. This refines FR-035 and is raised in `analysis.md`.

## R14. Conformance suite

**Repo**: `crates/vizier-http/tests/conformance/` checks route-by-route parity between the network router and the desktop dispatcher, and keeps `pending.txt` for routes with no desktop twin (the websocket routes, `/catalogue/logos/{file}`, `/integrations/kinds/{id}/icon`).

**Decision**: `POST/GET(list)/DELETE /shares` get vectors in `groups/shares.rs`. `GET /shares/{id}` (optional auth) is added to `pending.txt` with the reason "public, has no desktop twin".

## R15. Forking later (kept open, not built)

Blinfoldking's import layout is `<system prompt/> <agent config prompts/> imported conversation … end of imported history <agent own history/>`. The snapshot carries `turns[].role`, ordered `parts[].kind`, and `source.agent_name`, which is what an importer needs to wrap the history as "from another agent" without claiming its memory. Nothing in this feature writes to a conversation. A later spec adds `POST …/topics/import` that takes a snapshot id (or an uploaded file) and a position in context.

## Sources

- W1: Claude Help Center "Public links for shared chats"; Breachroad "Public AI chat links: sharing can become publishing"; OWASP WrongSecrets challenge 68; Trend Micro on ChatGPT shared links. Takeaways: a share link is publication; allow removal; keep unlisted; tell the person exactly what is included.
- W2: OWASP XSS Prevention and DOM-based XSS Prevention cheat sheets: encode for the output context; JavaScript-string encoding is the correct one for data in a script block.
- W3: `vite-plugin-singlefile` (npm, GitHub): inlines JS/CSS into one HTML; requires one entry and dynamic imports inlined.
- W4: Cloudflare "Origin Cache Control" and "Default cache behavior": JSON is not cached by default; `private`/`no-store`/`max-age=0` or `Set-Cookie` prevent caching; `immutable` semantics in "Immutable and versioned asset caching".
- W5: MDN `iframe.srcdoc` warning; HackTricks "iframes in XSS, CSP and SOP"; SVGGenie and getsvgeditor posts on SVG in data URIs: SVG can carry script and SMIL tricks, so raster only.
- W6: web.dev/W3C Referrer Policy; nanoid collision notes: unguessable id plus `no-referrer` and noindex for capability URLs.
