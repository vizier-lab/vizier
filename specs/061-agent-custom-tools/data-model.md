# Data model: Agent custom tools

Four tables behind `CustomToolStorage` (`storage/custom_tool.rs`), one implementation on
`SqlStorage` for SQLite (`init_custom_tool_schema`) and PostgreSQL (`0010_custom_tools.sql`).
Times are epoch milliseconds (no text-sorted time columns, so the UTC rule of `time::stored`
does not apply). JSON columns are `TEXT` on SQLite and `JSONB` on PostgreSQL.

## `custom_tool` — one row per tool

| Column | Type | Notes |
|---|---|---|
| `id` | TEXT PK | UUID; stable across renames |
| `agent_id` | TEXT NOT NULL | owner (FR-002) |
| `name` | TEXT NOT NULL | `^[a-z][a-z0-9_]*$`; called by this name; `UNIQUE(agent_id, name)`; refused when any other tool has it |
| `category` | TEXT NOT NULL | stored category name; FK-like to `custom_tool_category(agent_id, name)` |
| `current_seq` | INTEGER NOT NULL | the version callers run |
| `enabled` | INTEGER NOT NULL DEFAULT 1 | |
| `disabled_by` | TEXT | JSON `RevisionActor` of whoever disabled it; a person's disable blocks the agent re-enabling (FR-023) |
| `last_called_at` | INTEGER | best-effort, written after a call |
| `created_at` / `updated_at` | INTEGER NOT NULL | |

Index: `(agent_id, enabled, category)`.

## `custom_tool_version` — immutable snapshots

| Column | Type | Notes |
|---|---|---|
| `id` | INTEGER PK AUTOINCREMENT | |
| `tool_id` | TEXT NOT NULL | → `custom_tool.id` |
| `agent_id` | TEXT NOT NULL | for `delete_agent` |
| `seq` | INTEGER NOT NULL | 1, 2, 3 …; `UNIQUE(tool_id, seq)` |
| `name` | TEXT NOT NULL | name at this version (renames are versions) |
| `category` | TEXT NOT NULL | category at this version (moves are versions, FR-010e) |
| `description` | TEXT NOT NULL | |
| `input_schema` | JSON NOT NULL | R6 subset |
| `output_description` | TEXT | optional |
| `script` | TEXT NOT NULL | Python; `input` global, last expression is the result |
| `calls` | JSON NOT NULL | sorted list of function names the script calls (R4), derived at save |
| `sample_input` | JSON | as given; kept for the record |
| `actor_kind` / `actor_id` / `actor_name` | TEXT | `RevisionActor` (agent / person) |
| `trigger` | TEXT NOT NULL | `RevisionTrigger`-style: `create`, `update`, `rollback`, `category_rename` |
| `dream_cycle_id` | TEXT | set when a dream made it |
| `restored_from` | INTEGER | seq a rollback copied |
| `reason` | TEXT | optional, from the saver |
| `created_at` | INTEGER NOT NULL | |

A category rename moves every tool in it; it records **no** per-tool version (the tool's
definition did not change) — it is one `custom_tool_event` of kind `category_renamed` per tool, so
the history still shows it. Moving one tool *is* a version (FR-010e).

## `custom_tool_category`

| Column | Type | Notes |
|---|---|---|
| `agent_id` | TEXT NOT NULL | |
| `name` | TEXT NOT NULL | stored name after the clash rule (R12); `PRIMARY KEY(agent_id, name)` |
| `description` | TEXT NOT NULL | one line; the default `custom` gets a fixed text |
| `updated_by` | TEXT | JSON `RevisionActor` |
| `updated_at` | INTEGER NOT NULL | |

Lifecycle: created by the first save naming it (description required, FR-010b); deleted in the
transaction that removes or moves away its last tool (FR-010c); renaming onto an existing name is
refused (FR-010d).

## `custom_tool_event` — lifecycle actions that are not versions

| Column | Type | Notes |
|---|---|---|
| `id` | INTEGER PK AUTOINCREMENT | |
| `agent_id` | TEXT NOT NULL | |
| `tool_id` | TEXT | null for category-only events |
| `tool_name` | TEXT | kept so a delete event stays readable after the row is gone |
| `kind` | TEXT NOT NULL | `enabled`, `disabled`, `deleted`, `category_renamed`, `category_described` |
| `detail` | JSON | e.g. `{ "from": "reports", "to": "status_reports" }` |
| `actor_kind` / `actor_id` / `actor_name` / `dream_cycle_id` | | as on versions |
| `at` | INTEGER NOT NULL | |

A deleted tool's rows in `custom_tool` and `custom_tool_version` are removed (Story 5 #3); its
`deleted` event remains in the agent's event log (FR-025). `delete_agent` removes all four.

## Derived, never stored

| Value | Computed from | Where |
|---|---|---|
| `needs_attention.missing` | current version's `calls` − published `agent.{id}.tool_names` − the agent's enabled custom tool names (R8) | every read (agent and HTTP) |
| `model_backed` | `calls` ∩ model-backed tools, transitively through custom tools (R7) | every read |
| `callers` | other custom tools whose current `calls` contain its name | rename/delete checks (spec edges) |
| category in prompt | categories with ≥ 1 enabled tool, snapshotted at turn start | `ToolContext.custom_categories` |

## Validation (FR-005), all problems reported at once

| Rule | Code | Applies to |
|---|---|---|
| name matches the rule and is free for the agent (the tool's own name excepted on update) | `name_invalid`, `name_taken` | create, update (rename) |
| category given → exists, or `category_description` present | `category_description_required` | create, update |
| `input_schema` is `type: object` and uses only the R6 subset | `schema_not_object`, `schema_keyword_unsupported` (with path) | all saves |
| script parses | `script_syntax` (line, column, message) | all saves |
| every call resolves to a callable name (turn catalogue, or published view for people) | `unknown_tool` (with nearest names) / warning `tools_unchecked` | all saves |
| no cycle through custom tools | `cycle` (the path) | all saves |
| `sample_input` matches the schema | `sample_invalid` (paths) | saves with a sample |
| expected version (when given) equals `current_seq` | `out_of_date` | update, rollback, HTTP edit |
| a person disabled it and the actor is the agent | `disabled_by_person` | update, set_enabled by agent |
| rename/delete while other tools call it (agent actor) | `has_callers` (names) | update (rename), delete |

Sample run (FR-006) happens after every other check passes and before commit, only when the
context carries a script router and is not a dream (R9); a failed run is `sample_failed` with the
report, and nothing is stored.

## State transitions

```
            create (validated)                update / rollback / move (validated)
  (none) ───────────────────────▶ enabled ◀──────────────────────────────┐
                                    │  ▲                                    │
              set_enabled(false)    │  │ set_enabled(true)  (not by the agent if a person disabled it)
                                    ▼  │                                    │
                                  disabled ─────────────────────────────────┘ (updates allowed by people)
                                    │
                     delete (agent: no callers; person: confirmed) ──▶ (gone; event kept)
```
