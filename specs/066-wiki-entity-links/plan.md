# Implementation Plan: Entity Wiki Links for Artifacts, Functions, Skills, and Tasks

**Branch**: `066-wiki-entity-links` | **Date**: 2026-11-25 | **Spec**: [spec.md](./spec.md)

**Input**: Feature specification from `/specs/066-wiki-entity-links/spec.md`

## Summary

Generalize the `[[...]]` wiki-link notation beyond memories: agents and people write `[[artifact:name]]`, `[[function:name]]`, `[[skill:slug]]`, `[[task:slug]]` (alias `[[kind:name|label]]`) as plain text in replies, memory documents, and composer messages; the WebUI renders them as clickable links to the entity's existing pages and offers `[[`-triggered suggestions in the composer, mirroring the existing `@` file-mention flow. Resolution is derived at render time from one new lightweight per-agent read endpoint (`GET /agents/{id}/link-targets`, composing existing service calls — no schema changes, no migrations); the engine's only changes are the prompt guidance line and a guard so kind-prefixed links are not misread as memory bundle relations.

## Technical Context

**Language/Version**: Rust (workspace, edition 2021; `cargo build --workspace` builds the server) + TypeScript (React 19, React Router v7 SPA, Tailwind v4) for `webui/` and `design/`

**Primary Dependencies**: axum (vizier-http routes), existing storage/service layer in vizier-core, react-markdown pipeline (`webui/app/screens/shared/Markdown.tsx`), design `Composer`/`MentionMenu` (suggest popover), Zustand stores; **no new third-party dependencies**

**Storage**: none new — entity names are read through existing `ArtifactStorage`, `CustomToolStorage`, `TaskStorage`, `SkillManager` (filesystem), and memory bundle/doc storage calls; links themselves stay inline in message/memory text (derived, never indexed)

**Testing**: `cargo test -p vizier-core -p vizier-http`; `just web-check` (design + webui typecheck, token lint, vitest); end-to-end: dummyplug steps in [quickstart.md](./quickstart.md) (agent-observable: reply text with links round-trips, link-targets endpoint serves an agent's entities; the SC-005 prompt-adherence check needs a live provider and is marked as such)

**Target Platform**: the single server binary (Linux/macOS/Docker; desktop app inherits) + the embedded WebUI in any modern browser

**Project Type**: feature spanning one engine crate (vizier-core), one HTTP crate (vizier-http), the design catalogue (design/), and the WebUI (webui/)

**Performance Goals**: link-targets endpoint answers in well under 100 ms for an agent with hundreds of entities (names only, existing indexed queries); render-time transform is one regex pass per message/document, skipping code fences

**Constraints**: `SANDBOX.md` stays byte-stable (guidance goes in BOOT.md, not the sandbox prompt); no changes to the stored history JSON or memory document bodies; no new Cargo/npm dependencies; existing memory link forms keep byte-identical behaviour

**Scale/Scope**: ~6 engine/HTTP files, ~8 WebUI files, 1 design component variant, 2 docs pages; no database migration

## Constitution Check

*GATE: Must pass before Phase 0 research. Re-checked after Phase 1 design — **still passing**.*

| Principle | Verdict | Notes |
|---|---|---|
| I. Lean by Default | PASS | One new read endpoint composed of existing service calls; no index, no table, no migration, no new dependency; the spec itself defers backlink graphs and link inventories |
| II. DRY via Trait-Based Extensibility | PASS | Kinds vary by **data in one table** (kind → href builder, kind → display label), not scattered matches; the grammar lives in one module per side (engine guard in `memory_bundle.rs`, webui `lib/entityLinks.ts`); the endpoint composes existing `service::*` fns rather than re-querying storage |
| III. Self-Contained Default | PASS | Pure read path over embedded storage/filesystem; no external service, no network at runtime beyond the existing WebUI API |
| IV. Portability by Default | PASS | No OS/filesystem assumptions beyond `SkillManager`'s existing workspace scan (already trait-shaped); no new `cfg` |
| V. Unified Errors & Observability | PASS | Endpoint goes through `service::*` returning `crate::Result`/`ServiceError` → existing HTTP envelope; `tracing` only; no new fallible paths elsewhere |
| E2E via dummyplug | PLANNED | quickstart.md scripts the agent-observable checks (reply text with `[[...]]` round-trips through a durable turn; link-targets serves a dummyplug agent's artifacts/functions/tasks/skills/memory); SC-005 marked live-provider-only |
| Design before UI code | PLANNED | Entity suggestions are a **new row variant of design's `MentionMenu`** → mockup + review before code (FigCraft preferred; ASCII/dummy-HTML fallback), then a story in `design/`, then `webui/` use. Chat-message links reuse the existing internal-link rendering (no new visual variant); composer popover placement reuses the existing `suggest` prop unchanged |
| Feature flags | PASS | Core read/render capability, not an optional channel/addon/provider/gateway — no flag; no API shape change |
| Conventional commits / submodule branches | PLANNED | Tasks use conventional commits; `landing`/`dummyplug` submodules untouched (dummyplug protocol unchanged — the e2e only *uses* it), so no submodule branch work beyond the existing one |

## Project Structure

### Documentation (this feature)

```text
specs/066-wiki-entity-links/
├── plan.md              # This file
├── research.md          # Phase 0 output: decisions R1–R9
├── data-model.md        # Phase 1 output: parsed link, link-targets DTO, uniqueness rules
├── quickstart.md        # Phase 1 output: dummyplug e2e script
├── contracts/
│   ├── link-grammar.md            # The [[kind:name]] text grammar (both sides)
│   └── link-targets-endpoint.md   # GET /api/v1/agents/{id}/link-targets
└── tasks.md             # Phase 2 output (/speckit-tasks — NOT created here)
```

### Source Code (repository root)

```text
crates/vizier-core/
├── src/storage/memory_bundle.rs            # guard: skip kind-prefixed [[...]] in parse_wikilinks/classify (no broken memory edges)
├── src/agents/agent/system_prompt/boot.rs  # extend the memory-link guidance line with the entity kinds
└── src/scheduler/dream/prompts.rs          # mention entity links beside the [[slug]] memory hint

crates/vizier-http/src/api/v1/agents/
└── link_targets.rs                         # GET /agents/{id}/link-targets (new; router entry in agents/mod.rs)

webui/app/
├── lib/entityLinks.ts                      # grammar: parse + rewrite [[kind:name]] outside code; kind→href table (single source)
├── lib/entityLinks.test.ts                 # fixtures: all kinds, alias, code-fence skip, unknown kind, broken target
├── lib/linkTargets.ts                      # fetch + narrow per-agent cache of the endpoint
├── stores/linkTargets.ts                   # Zustand store (mirrors stores/projects.ts byAgent shape)
├── screens/chat/MessageList.tsx            # apply the rewrite to agent reply + person message text
├── screens/memory/MemoryDocument.tsx       # extend the existing rewrite call with entity links
├── screens/chat/Composer.tsx               # second trigger [[ reusing the suggest/MentionMenu flow (no server-side mention tracking)
└── routes/artifacts.tsx                    # gallery deep-link: ?artifact=<name> preselects (target of artifact links)

design/src/components/MentionMenu/
└── MentionMenu.tsx (+ story)               # new entity row variant (icon + name + kind note) in the item union

docs/src/configuration/
├── memory.md                               # extend the link-syntax table: entity kinds vs memory forms
└── entity-links.md                         # new page: syntax, targets, suggestions, permissions
```

**Structure Decision**: one feature, four layers, each touched at its existing seam — engine (prompt text + a parser guard), HTTP (one read route in the existing `agents/` module), design (one new MentionMenu row variant), WebUI (one grammar module + the existing composer/markdown seams). No new crate, no new module tree.

## Complexity Tracking

> No constitution violations — table intentionally empty.

| Violation | Why Needed | Simpler Alternative Rejected Because |
|-----------|------------|-------------------------------------|
| — | — | — |
