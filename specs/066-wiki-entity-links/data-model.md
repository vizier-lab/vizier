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

## LinkTargets (endpoint response DTO — `GET /agents/{id}/link-targets`)

| Field | Shape | Source (existing call) | Notes |
|---|---|---|---|
| `artifacts` | `[{ name, updated_at }]` | `service::artifacts::list` | names unique per agent; ordered `updated_at` desc |
| `functions` | `[{ name }]` | `service::custom_tools` (`tools()`) | names unique per agent; `enabled` ones only? **No — all**, a disabled tool still resolves (its page explains the state) |
| `tasks` | `[{ slug, title }]` | `service::tasks::list` | slug unique per agent; active and inactive |
| `skills` | `[{ slug, scope }]` | `service::skills::list_agent` (agent + global) | workspace scopes only (research R4); `scope` ∈ `agent`\|`global` |
| `memory.bundles` | `[name]` | `service::memory::list_bundles` | complete |
| `memory.docs` | `[{ bundle, path }]` | `service::memory::list` | capped to latest 500 |

**Client cache**: one entry per agent in a Zustand store (`stores/linkTargets.ts`, mirroring `stores/projects.ts`'s `byAgent` + `loaded` shape); refreshed on conversation open and before a suggestion pass if stale; consumers: renderer (broken-state decisions) and composer suggestions.

## Existing uniqueness constraints relied on

- `artifact`: `UNIQUE (agent_id, name)` (`storage/sqlite/mod.rs:667`); promote refuses duplicates.
- custom tool: `UNIQUE (agent_id, name)` + `name_taken` validation against reserved/known names.
- task: PK `{agent_id}/{slug}`.
- memory bundle/doc: existing rules unchanged; entity-link parsing never creates bundle rows (engine guard, research R1).

Tie-break if uniqueness ever relaxes: resolve to the most recently updated entity (documented contract; FR-11).
