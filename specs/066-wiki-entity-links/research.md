# Research: Entity Wiki Links (066-wiki-entity-links)

Phase 0 output. Every NEEDS CLARIFICATION from the spec is resolved by a decision here; the spec itself carried none, so these are the design decisions the plan depends on. Grounding: two codebase explorations (WebUI rendering/composer/routes/endpoints; server list endpoints/storage/fills) plus targeted checks of `MentionMenu` and the artifact gallery.

## R1 — Link grammar and its collision with memory wikilinks

**Decision**: Entity links are the kind-prefixed forms `[[kind:name]]` and `[[kind:name|label]]` with kind ∈ {`artifact`, `function`, `skill`, `task`}. Everything else in `[[...]]` keeps its existing memory meaning (`[[bundle/slug]]`, `[[bundle]]`, bare `[[slug]]`). An entity link requires the colon form with a known kind; an unknown kind (`[[flavour:vanilla]]`) is left literal by both renderers.

**Rationale**: Bare `[[name]]` already classifies as a memory bundle reference in `parse_wikilinks` (`crates/vizier-core/src/storage/memory_bundle.rs:181`) and in the WebUI's `linkWikiLinks` (`webui/app/lib/memoryLinks.ts:13`) — reusing it for entities would reinterpret existing content. The colon form is unparseable as a bundle path today (bundle/doc names don't carry `:`), so no existing memory document changes meaning.

**Consequence (engine guard)**: `parse_wikilinks`/`classify_relation` must skip targets whose text before the first `:` is one of the four reserved kind words, so a reply or memory containing `[[artifact:report]]` does not create a broken `artifact:report` bundle relation in the memory graph. This is the one engine-side parse change; it widens the existing skip behaviour, it does not reclassify.

**Alternatives considered**: bare-name resolution order (rejected: ambiguous with bundles and across kinds, and the model must guess); separate delimiters like `{{artifact:report}}` (rejected: two notations for one concept, and the user asked for the `[[...]]` family); namespace via path (`[[entity/artifact/report]]`) (rejected: collides with nested bundle paths).

## R2 — Where resolution and broken-link status live

**Decision**: The WebUI resolves links **client-side** against a cached per-agent entity-name index, backed by one new route family (R3) with three modes: a bounded index (Mode 1), a server-side search (Mode 2, the composer's fallback — the same debounced pattern as `suggestProjectFiles`), and a batch exact-existence resolve (Mode 3, the renderer's). Parsed links whose target is absent from the cache — or that a Mode 3 resolve says is gone — render in the broken state; present ones render as internal links. The parser and the kind→href table live in one module (`webui/app/lib/entityLinks.ts`).

**Rationale**: The alternative shapes were measured against the codebase:
- *Server-side fill* (`fill_reactions` pattern, `storage/sqlite/history.rs:186`): message text lives inside the `data` JSON blob, so a fill would regex-scan every history entry on every read path and then existence-check each reference per kind. Consistent with the pattern but strictly heavier, and it wouldn't cover memory documents rendered client-side anyway.
- *Per-surface list composition* (call the four existing list endpoints when rendering): N requests, four caches to keep fresh, and it couples message rendering to full list payloads (pagination, content fields) built for their own pages.
- *Always-clickable without status*: fails spec FR-10/US3-2 (visible broken state; following must not land on a broken page).

One small endpoint + one client cache serves **both** consumers (renderer and composer suggestions), which is why it wins on Principle I.

## R3 — One route family: `GET /api/v1/agents/{id}/link-targets` in three modes

**Decision**: A single route file in `vizier-http/src/api/v1/agents/link_targets.rs` (mounted in `agents/mod.rs`), names only, composed from existing service calls — `service::artifacts::list`, `service::custom_tools::list`/`tools()`, `service::tasks::list`, `service::skills::list_agent` (agent + global scopes), `service::memory::list_bundles` + `list`. **No kind ships unbounded**: artifacts (the one genuinely unbounded kind — 5k entries ≈ 0.5 MB) are capped to the latest 200 and memory docs to the latest 500, with a `truncated` map telling the client which kinds were cut. Three modes on the same family, full shape in `contracts/link-targets-endpoint.md`:

1. **Index** (no params): the bounded map above — powers the client cache.
2. **Search** (`?kind=&q=&limit≤50`): server-side substring filter — the composer's fallback when the local index is exhausted or truncated (mirrors the existing `@` file-mention debounce, one request per suggestion pass, not per keystroke).
3. **Resolve** (`POST …/resolve`, ≤100 links/call): exact-existence map keyed `kind:name` — the renderer's batch check for out-of-cap or stale names.

Authorization: the standard agent read path — load agent, 404 if missing, `can_view_agent` else `Forbidden` (`service/tasks.rs:201` shape) — on all three modes.

**Rationale**: A full unbounded index loses to scale (artifacts accumulate for the life of an agent); a search-only API loses to the renderer, which needs per-name existence, not ranking, and would pay a round trip per link. Bounded index + search + resolve keeps every response small while both consumers stay off the per-keystroke and per-link request paths. Every underlying list call already exists and is indexed; one route family = one permission check.

**Uniqueness finding (relevant to spec FR-11)**: artifact names are `UNIQUE (agent_id, name)` (`storage/sqlite/mod.rs:667`), custom-tool names are `UNIQUE (agent_id, name)` plus the `name_taken` validator (`custom_tools/validate.rs:24`), and task ids are `{agent_id}/{slug}` — so per kind, per agent, names are unique today. The documented "newest first" fallback order therefore never triggers in practice; it is kept as the contract's tie-break rule should that ever change.

## R4 — Skills: workspace scopes only in v1

**Decision**: `[[skill:slug]]` targets and suggestions cover the **agent** and **global** workspace scopes — the scopes with pages (`/:agentId/skills/:slug`) and an endpoint (`service::skills::list_agent`). Project-scope skills (`.agents/skills`, `.claude/skills`, per-turn filesystem per `skill/scopes.rs`) are out of v1: they have no HTTP list, no page, and a link to one could only ever render broken.

**Rationale**: The spec's FR-5 said "the skills in the scopes it uses"; this narrows v1 to the scopes that are addressable in the UI. A typed-by-hand `[[skill:x]]` that only matches an uninstalled/project skill renders broken per spec — correct, not a defect. Recorded as a deliberate scope note, not a spec change.

## R5 — Rendering seam

**Decision**: One string→string rewrite module, `webui/app/lib/entityLinks.ts`: split out fenced/inline code (the `linkWikiLinks` split pattern), parse `[[...]]`, emit ordinary markdown links with **internal** hrefs. Applied at three call sites: `MessageList.tsx` (agent reply text, and person message text), `MemoryDocument.tsx` (extending its existing rewrite call), and nowhere else in v1. Internal hrefs flow through the existing `<Link>` branch in `shared/Markdown.tsx:56` — no renderer change.

**Rationale**: Mirrors two proven precedents (`linkWikiLinks`, `mentionsAsCode`); a pre-transform keeps ReactMarkdown untouched and makes the whole grammar unit-testable as pure functions.

**Kind → target table** (the single dispatch table, in `entityLinks.ts`):

| kind | href | note |
|---|---|---|
| `artifact` | `/:agentId/artifacts?artifact=<name>` | gallery deep-link (R8) |
| `function` | `/:agentId/functions/:name` | existing route |
| `skill` | `/:agentId/skills/:slug` | existing route (workspace scopes, R4) |
| `task` | `/:agentId/tasks/:slug` | existing route; runs listed on the page |
| memory (existing forms) | `docHref`/`bundleHref` | unchanged (`memoryTree.ts`) |

## R6 — Composer suggestions: a second trigger on the existing flow

**Decision**: `Composer.tsx` grows a `[[` trigger beside `@`: the same `suggest` prop of design's `Composer` and the same `MentionMenu` popover (keys, debounce, stale-drop, dismissal all already owned by those components). Token scanner: `/(^|\s)\[\[([a-z]*)?:?([^\]\n|]*)$/`-shaped — `[[` at token start, optional kind prefix before `:`, free text after. Picking inserts the complete `[[kind:name]] ` form. Unlike `@` mentions there is **no post-send tracking**: an entity link is plain text, nothing is stripped or resolved into the request (spec FR-9), so `pickedMentions` has no counterpart.

**Rationale**: The `@` flow (`mentionQuery`/`insertMention`, `Composer.tsx:142-170`) already owns every interaction detail spec FR-4 demands (debounce, Escape, keyboard, dismiss/re-open); reusing it is both leaner and exactly the "MUST match the existing @ file-mention suggestions" requirement. Suggestions come from the same cached link-targets store as the renderer (R2) — zero extra requests while typing beyond the cache refresh.

**Coexistence**: exactly one popover at a time; the trigger whose token ends the input wins (`@` and `[[` cannot both match the same suffix); after a pick the scanner can't re-match the completed form (`]]` terminates the token).

## R7 — Design pass (constitution: design before UI code)

**Decision**: `MentionMenuItem` is a closed union (`file|dir|agent|person`, `design/src/components/MentionMenu/MentionMenu.tsx:10`), so entity suggestions are a **new row variant** of `MentionMenu`: icon (per kind, from the existing icon set used by tool/nav icons) + name + quiet kind note, matching `FileRow`/`MemberRow` structure. This requires the mockup-first pass: draw the composer deck row in the `Vizier Chat Panel.figcraft` composer sheet if the FigCraft MCP/app is reachable, else an ASCII/dummy-HTML mockup in this repo — reviewed before implementation; then a story in `design/`, then webui use (one branch/PR since `design/` is a repo folder).

**Not a new variant**: the chat-message rendering of entity links (plain internal links in `Prose` body text) and the popover's placement/behaviour — both reuse existing looks unchanged. The memory-document sidebar reuses `DocLink`'s existing shape if entity links are listed there (out of v1 scope unless trivial).

## R8 — Artifact links need a gallery deep-link

**Decision**: There is no per-artifact route (only `/:agentId/artifacts` with detail inside `ArtifactsGallery`, state-driven). Artifact links navigate to `/:agentId/artifacts?artifact=<name>`; `routes/artifacts.tsx` reads the param and preselects that artifact. `get_artifact_by_name` exists, and names are unique per agent (R3), so name is a stable enough handle.

**Alternatives considered**: a new `/:agentId/artifacts/:name` route (rejected in v1: heavier routing change, gallery already owns detail presentation); linking by artifact id (rejected: ids are opaque; links written by agents/people read better with names).

## R9 — Teaching the agents, and the docs

**Decision**: Extend the BOOT.md memory-link guidance line (`boot.rs:31`) to name the four entity kinds with one example, keeping it one bullet; extend the dream prompt's `[[slug]]` hint (`scheduler/dream/prompts.rs:51`) similarly. The `memory_*` tool descriptions stay memory-only (they describe memory semantics, not chat typography). Docs: extend the link-syntax table in `docs/src/configuration/memory.md` and add `docs/src/configuration/entity-links.md` (syntax, targets, permissions, suggestions); cross-link from the sharing docs only if the snapshot edge case needs a sentence.

**Rationale**: The prompt line is the only channel agents have (R6 of the spec's assumptions); keeping it one bullet avoids prompt bloat while SC-005 measures whether it is *sufficient* (live-provider check; escalate to an example block only if the scripted session falls below the bar).

## Residual risks

- **Prompt adherence**: models may emit links imperfectly (missing kind, bare `[[name]]`). The grammar treats those as memory links / literal text — degraded but never wrong; SC-005 measures real adherence on a live model.
- **Cache staleness**: a link created after the cache refresh could render "broken" until the next refresh. Mitigated: names absent from a truncated kind, and any link the renderer is about to mark broken, are confirmed by a Mode 3 resolve batch before the broken state is shown; refresh also happens on conversation open and on pick.
- **Capped kinds**: old artifacts (beyond 200) and memory docs (beyond 500) are absent from suggestions until the composer falls back to Mode 2 search, and from the index until a Mode 3 resolve runs — the caps bound payload, and bundles (always complete) remain the primary memory suggestion target.
