# Data Model: Entity Wiki Links (066-wiki-entity-links)

No database entities are added or changed. The "data model" is two derived shapes (a parsed link, the link-targets index) plus the existing uniqueness rules the resolution relies on.

## ParsedWikiLink (derived at render time — not stored)

| Field | Type | Rules |
|---|---|---|
| `kind` | `entity(kind)` or `memory` | `entity` iff the text before the first `:` is exactly one of `artifact`/`function`/`skill`/`task`; `memory` for `bundle/slug`, `bundle`, bare `slug` |
| `target` | string | entity: the text after the first `:`, up to `|`, trimmed, non-empty; memory: the whole target, existing classification unchanged |
| `alias` | string, optional | text after the first `|`, trimmed; display label when present |
| `href` | string (renderer output) | built from the kind→target table (research R5); memory targets keep `docHref`/`bundleHref` |

**Validation rules**: a link is *malformed* (rendered literally, never an error) when: unknown kind word, empty target, target containing `]]` or a newline, or the whole token inside a fenced/inline code span. A *well-formed* link with a target missing from the agent's link-targets index renders in the **broken state** (styled, non-navigating or leading to the page's own not-found state — never a crash).

**State transitions**: none (no lifecycle; links are inert text).

## LinkTargets (endpoint response DTO — `GET /agents/{id}/link-targets`, three modes)

| Field | Shape | Source (existing call) | Notes |
|---|---|---|---|
| `artifacts` | `[name]` (latest **200**, `updated_at` desc) | `service::artifacts::list` | names unique per agent; the unbounded kind — capped, never shipped whole |
| `functions` | `[name]` | `service::custom_tools` (`tools()`) | names unique per agent; all tools, disabled included (a disabled tool still resolves — its page explains the state) |
| `tasks` | `[{ slug, title }]` | `service::tasks::list` | slug unique per agent; active and inactive |
| `skills` | `[{ slug, scope }]` | `service::skills::list_agent` (agent + global) | workspace scopes only (research R4); `scope` ∈ `agent`\|`global` |
| `memory.bundles` | `[name]` | `service::memory::list_bundles` | complete |
| `memory.docs` | `[{ bundle, path }]` (latest **500**) | `service::memory::list` | capped like artifacts |
| `truncated` | `{ kind: bool }` | — | true wherever a cap bit; client falls back to search for those kinds |

**Mode 2** (`?kind=&q=&limit≤50`) returns the same per-kind array shape, server-filtered (case-insensitive substring). **Mode 3** (`POST …/resolve`, ≤100 links) returns `{ resolved: { "kind:name": bool } }` — exact existence for out-of-cap or stale names.

**Client cache**: the Mode 1 index cached per agent in a Zustand store (`stores/linkTargets.ts`, mirroring `stores/projects.ts`'s `byAgent` + `loaded` shape); refreshed on conversation open and before a suggestion pass if stale; consumers: renderer (broken-state decisions, confirmed by Mode 3 batches) and composer suggestions (local filter, Mode 2 fallback).

## Existing uniqueness constraints relied on

- `artifact`: `UNIQUE (agent_id, name)` (`storage/sqlite/mod.rs:667`); promote refuses duplicates.
- custom tool: `UNIQUE (agent_id, name)` + `name_taken` validation against reserved/known names.
- task: PK `{agent_id}/{slug}`.
- memory bundle/doc: existing rules unchanged; entity-link parsing never creates bundle rows (engine guard, research R1).

Tie-break if uniqueness ever relaxes: resolve to the most recently updated entity (documented contract; FR-11).
