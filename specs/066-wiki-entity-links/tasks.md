---

description: "Task list for feature 066-wiki-entity-links"
---

# Tasks: Entity Wiki Links for Artifacts, Functions, Skills, and Tasks

**Input**: Design documents from `/specs/066-wiki-entity-links/`

**Prerequisites**: plan.md, spec.md, research.md, data-model.md, contracts/link-grammar.md, contracts/link-targets-endpoint.md, quickstart.md

**Tests**: Included, because the constitution's quality gates (`cargo clippy`/`cargo test`, `just web-check`, the dummyplug e2e) and the spec's success criteria (fixture rendering, SC-005) require them — they are scoped to the units that carry the logic, not blanket coverage.

**Organization**: Tasks are grouped by user story so each story can be implemented, tested, and demoed independently.

## Format: `[ID] [P?] [Story] Description`

- **[P]**: Can run in parallel (different files, no dependencies)
- **[Story]**: Which user story this task belongs to (US1 = clickable links in chat, US2 = composer suggestions, US3 = uniform rendering + graceful breakage)
- Include exact file paths in descriptions

## Path Conventions

This feature spans the Rust workspace (`crates/`), the design catalogue (`design/`), the WebUI (`webui/`), and docs — all folders of this repository, one branch and one PR.

---

## Phase 1: Setup (Shared Infrastructure)

**Purpose**: The one constitution gate that must precede UI code: the reviewed mockup for the new composer suggestion row (research R7).

- [ ] T001 Draw the entity suggestion row mockup (icon + name + quiet kind note, matching `FileRow`/`MemberRow` structure) — FigCraft `Vizier Chat Panel.figcraft` composer sheet if the MCP/app is reachable, else a dummy `.html` or ASCII mockup saved to `specs/066-wiki-entity-links/mockup.md`; get it reviewed before T011 touches `design/`

---

## Phase 2: Foundational (Blocking Prerequisites)

**Purpose**: The grammar (both sides), the engine guard, the link-targets endpoint, and the client cache — every user story consumes these.

**⚠️ CRITICAL**: No user story work can begin until this phase is complete

- [ ] T002 [P] Engine guard: in `crates/vizier-core/src/storage/memory_bundle.rs`, make `parse_wikilinks`/`classify_relation` skip targets whose pre-colon word is `artifact|function|skill|task` (contracts/link-grammar.md precedence rule 2) so `[[kind:name]]` never creates or breaks a memory relation; add unit tests beside the existing wikilink tests (bare `[[slug]]`, `[[bundle/slug]]` behaviour must stay byte-identical)
- [ ] T003 [P] WebUI grammar: create `webui/app/lib/entityLinks.ts` (`parseEntityLinks`, `rewriteEntityLinks` string→string, code-span-aware, and the kind→href table per contracts/link-grammar.md: artifact→`/:agentId/artifacts?artifact=<name>`, function→`/:agentId/functions/<name>`, skill→`/:agentId/skills/<slug>`, task→`/:agentId/tasks/<slug>`) with fixtures in `webui/app/lib/entityLinks.test.ts` covering every kind, the `|label` alias, code-fence skip, unknown kind literal, unclosed `[[` literal, and memory-form passthrough (`[[bundle/slug]]` untouched)
- [ ] T004 [P] Link-targets service: create `crates/vizier-core/src/service/link_targets.rs` (register in `service/mod.rs`) with the three modes from contracts/link-targets-endpoint.md — index (artifacts capped latest 200, memory docs latest 500, functions/tasks/skills/bundles complete, `truncated` map), search (`kind`+`q`+`limit≤50`, case-insensitive substring), resolve (≤100 links, `kind:name` → bool) — composed from `service::artifacts::list`, `service::custom_tools`, `service::tasks::list`, `service::skills::list_agent`, `service::memory::list_bundles`/`list`, authorizing with load-agent → 404 → `can_view_agent` → 403 (`service/tasks.rs::view_agent` shape); returns `crate::Result`, `ServiceError` only
- [ ] T005 Link-targets routes: create `crates/vizier-http/src/api/v1/agents/link_targets.rs` (`GET` index, `GET ?kind=&q=&limit=`, `POST /resolve` with 422 on oversized/malformed body) and mount it in `crates/vizier-http/src/api/v1/agents/mod.rs`; add handler-level tests in `crates/vizier-http/tests/link_targets.rs` (three fixtures users pattern: manager 200 with all kinds, view-only 200, stranger 403/404, resolve exactness incl. out-of-cap artifact names)
- [ ] T006 Client fetch + cache: create `webui/app/api/endpoints/linkTargets.ts` (typed index/search/resolve calls against the v2 contract) and `webui/app/stores/linkTargets.ts` (Zustand, `byAgent` + `loaded` + `refresh(agentId)` mirroring `stores/projects.ts`, `truncated` surfaced); no test — thin delegation, exercised via US1/US2 fixtures

**Checkpoint**: `cargo test -p vizier-core -p vizier-http` green on T002/T004/T005; grammar fixtures green; endpoint serves a real agent's names — user story work can begin

---

## Phase 3: User Story 1 — Agent replies contain clickable entity links (Priority: P1) 🎯 MVP

**Goal**: A reply (or person message) containing `[[kind:name]]` renders clickable links that land on the right page; agents are taught the syntax.

**Independent Test**: Send a dummyplug agent a message, get a reply containing one link of each kind (quickstart step 3); every link in the rendered message navigates to the correct existing page.

### Implementation for User Story 1

- [ ] T007 [US1] Apply `rewriteEntityLinks` in `webui/app/screens/chat/MessageList.tsx` to agent reply text and person message text (alongside the existing `mentionsAsCode` pre-transform) so internal hrefs flow through the existing `<Link>` branch in `webui/app/screens/shared/Markdown.tsx`; extend the chat-screen vitest coverage with a message containing all four kinds + alias + a code fence
- [ ] T008 [P] [US1] Artifact deep-link: make `webui/app/routes/artifacts.tsx` read `?artifact=<name>` and preselect that artifact in `webui/app/screens/artifacts/ArtifactsGallery.tsx` (name is unique per agent; fall back to plain gallery when absent) — the target of `[[artifact:…]]` links
- [ ] T009 [P] [US1] Agent guidance: extend the memory-link bullet in `crates/vizier-core/src/agents/agent/system_prompt/boot.rs` (`boot_md`) to name the four entity kinds with one example, and the `[[slug]]` hint in `crates/vizier-core/src/scheduler/dream/prompts.rs` the same way; update any boot/dream prompt assertions in existing tests

**Checkpoint**: US1 fully functional alone — quickstart steps 3–4 pass against a running binary (links survive a durable turn; engine guard creates no phantom relations)

---

## Phase 4: User Story 2 — The person writes links with suggestions in the composer (Priority: P2)

**Goal**: Typing `[[` opens the suggestion popover over the agent's entities; picking inserts a complete link; sent text renders per US1.

**Independent Test**: With an agent owning one entity of every kind, type `[[`, filter, pick each kind, send — every inserted link renders and navigates.

### Implementation for User Story 2

- [ ] T010 [US2] Design implementation (after T001 review): extend `design/src/components/MentionMenu/MentionMenu.tsx` — new entity item variant in the `MentionMenuItem` union (`kind: "artifact"|"function"|"skill"|"task"|"memory"`, `name`, optional `note`) with an `EntityRow` (per-kind icon, name strong, kind note quiet) and add/extend its Storybook story in `design/` following the component's existing story pattern; `just web-check` stays green (token lint included)
- [ ] T011 [US2] Composer trigger: in `webui/app/screens/chat/Composer.tsx` add the `[[` scanner beside `mentionQuery` (token `(^|\s)\[\[([a-z]*)?:?([^\]\n|]*)$`), wire the same `suggest` popover (`MentionMenu` via design `Composer`'s `suggest` prop) fed from `stores/linkTargets.ts` — local filter first, debounced Mode 2 search fallback when `truncated` or no local hits; picking inserts the complete `[[kind:name]] ` form; no `pickedMentions`-style tracking (links are plain text, FR-9); extend `webui/app/screens/chat/Composer.test.tsx` (trigger open, filter, kind-prefix narrowing via `[[skill:`, keyboard pick, Escape dismiss, insert format)

**Checkpoint**: US1 + US2 both work independently; composer adds no request per keystroke beyond the debounced search fallback

---

## Phase 5: User Story 3 — Uniform rendering and graceful breakage (Priority: P3)

**Goal**: The same links render in memory documents; deleted/renamed targets show a recognizably broken state; shared snapshots stay inert; docs exist.

**Independent Test**: Put all kinds in a memory document and a chat message; delete one target — rendering stays uniform, the stale link shows broken, nothing crashes or leaks.

### Implementation for User Story 3

- [ ] T012 [US3] Memory-document rendering: compose `rewriteEntityLinks` into the existing `linkWikiLinks` call in `webui/app/screens/memory/MemoryDocument.tsx` so entity and memory links render identically in one pass
- [ ] T013 [US3] Broken-state rendering: add the broken style + resolve flow — links absent from the cached index (or in a `truncated` kind) are confirmed by a batched `POST /resolve` (≤100/call, once per visible message list / document) before rendering broken, in `webui/app/lib/entityLinks.ts` + the `MessageList`/`MemoryDocument` paths; fixtures: deleted artifact name, stale cache, `truncated.artifacts=true` path
- [ ] T014 [P] [US3] Shared/offline inertness: verify the share viewer path (`webui/app/viewer/`) renders entity links as plain text (no navigation to live agent pages) and add a fixture; adjust `webui/app/lib/share/` only if the snapshot pipeline needs the exclusion
- [ ] T015 [P] [US3] Docs: extend the link-syntax table in `docs/src/configuration/memory.md` (entity kinds vs memory forms, precedence) and add `docs/src/configuration/entity-links.md` (syntax, targets table, suggestions, permissions, caps/truncated behaviour)

**Checkpoint**: All three stories independently functional

---

## Phase 6: Polish & Cross-Cutting Concerns

**Purpose**: The constitution's gates and the full offline verification.

- [ ] T016 [P] `cargo clippy --workspace` and `cargo test -p vizier-core -p vizier-http` green; also `cargo test -p vizier-core --no-default-features` (the touched code is in the default path)
- [ ] T017 [P] `just web-check` green (design + webui typecheck, token lint, vitest) — covers T003/T007/T010–T013 fixtures
- [ ] T018 Run the full `quickstart.md` dummyplug e2e against a running binary (constitution e2e gate): entity creation via API, link-targets index/search/resolve responses, durable-turn link round-trip, no phantom memory relations, broken/permission edges (steps 1–5); note SC-005 as the recorded live-provider follow-up

---

## Dependencies & Execution Order

### Phase Dependencies

- **Setup (Phase 1)**: T001 has no dependencies; it gates only T010 (design code)
- **Foundational (Phase 2)**: T002/T003/T004 run in parallel; T005 needs T004; T006 needs T005; **Phase 2 blocks all user stories**
- **US1 (Phase 3)**: needs Phase 2 (T003 grammar, T006 store); T007 after T003+T006; T008/T009 parallel with T007
- **US2 (Phase 4)**: T010 needs T001 + Phase 2; T011 needs T006 + T010
- **US3 (Phase 5)**: T012 after Phase 2; T013 after T012 (same render paths); T014/T015 parallel anytime after Phase 2
- **Polish (Phase 6)**: after all desired stories

### User Story Dependencies

- **US1**: independent of US2/US3 once Foundational lands — the MVP increment
- **US2**: reuses US1's rendering for verification but does not modify it; testable alone (insert → send → US1 rendering)
- **US3**: extends US1's render paths (MessageList) and shares the grammar module; broken-state needs T006's `truncated`/resolve surface

### Parallel Opportunities

- Phase 2: T002, T003, T004 simultaneously (engine / webui-lib / service — disjoint files)
- Phase 3: T008 (routes/artifacts) and T009 (boot/dream prompts) parallel with T007
- Phase 5: T014 and T015 parallel with T012/T013
- Stories themselves: after Phase 2, US1 and (mockup done) US2 can proceed in parallel — different files, no cross-story edits

---

## Parallel Example: Foundational Phase

```bash
# Launch together (disjoint files):
Task: "T002 Engine guard in crates/vizier-core/src/storage/memory_bundle.rs"
Task: "T003 Grammar module webui/app/lib/entityLinks.ts + fixtures"
Task: "T004 Service crates/vizier-core/src/service/link_targets.rs"
# Then sequentially: T005 (needs T004) → T006 (needs T005)
```

---

## Implementation Strategy

### MVP First (User Story 1 Only)

1. T001 mockup → Phase 2 foundational (T002–T006)
2. Phase 3: T007–T009
3. **STOP and VALIDATE**: quickstart steps 1–4 against `just run` — links in a dummyplug reply render and navigate
4. Ship-worthy increment: agents can cite work, people can click it

### Incremental Delivery

1. Foundational → US1 (MVP) → validate
2. US2 (T010–T011) → validate with the composer flow
3. US3 (T012–T015) → validate breakage + docs
4. Phase 6 gates → PR

### Single-Developer Order

T001 → T002 → T003 → T004 → T005 → T006 → T007 → T008 → T009 → T010 → T011 → T012 → T013 → T014 → T015 → T016 → T017 → T018

---

## Notes

- [P] tasks = different files, no dependencies
- [Story] label maps task to spec user story for traceability
- Commit after each task or logical group in conventional-commit format (`feat(core): …`, `feat(http): …`, `feat(webui): …`, `docs: …`); submodules `landing`/`dummyplug` are untouched, so plain workspace commits suffice
- The dummyplug protocol is used, not changed — no submodule branch work
- Stop at any checkpoint to validate the story independently
- Avoid: putting the grammar in two places (engine guard and webui module are the only two, pinned by fixtures); no new dependencies (constitution Principle I)
