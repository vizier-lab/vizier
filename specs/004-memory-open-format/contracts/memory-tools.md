# Contract: agent-facing memory tools

Location: `src/agents/tools/vector_memory/mod.rs`. Existing tools: `memory_list`,
`memory_read`, `memory_write`, `memory_detail`, `memory_follow`, `memory_graph`, `memory_delete`.

**New tool: `memory_delete_bundle`** — deletes a bundle's `index.md`/`log.md` (the bundle itself),
but only once it holds zero concept documents; `Err` otherwise, naming how many remain. Its one
input field, `bundle: String`, is **required** with no default (unlike every other tool's
`bundle: Option<String>`) — a whole-bundle deletion is deliberately never triggerable by an
omitted/default argument. Included in `VizierTools::DREAM_TOOL_NAMES` alongside `memory_delete`,
since the empty-only precondition already bounds its blast radius to "removing two already-inert
housekeeping files," not content loss.

## Input schema changes

Every tool's `Input` struct gains an optional field:

```rust
#[serde(default)]
pub bundle: Option<String>,
```

**`bundle: None` does not mean the same thing on every tool — there are three shapes, not two**,
matching what each tool actually does (research.md §12):

| Tool | Shape | `bundle: None` means | Passing `bundle` means |
|---|---|---|---|
| `memory_write` | addresses **one** concept (create/update) | The agent's default bundle (FR-008). | Write into that bundle (created automatically if new). |
| `memory_detail` / `memory_delete` / `memory_follow` | addresses **one** existing concept, by `(bundle, path)` | The agent's default bundle — same resolution rule as `memory_write`, so an unqualified reference always means "the thing in my default bundle," consistently across read and write. | Address the concept in that specific bundle. |
| `memory_read` | **searches** (semantic, via `query_memory`) | Search across **all** of the agent's bundles, ranked by relevance regardless of which bundle a match lives in — this is the most useful default for search specifically, not a "top level," because a bundle has no content of its own to match against; only its concepts do. | Narrow the same semantic search to just that one bundle. |
| `memory_list` / `memory_graph` | **browses**, at a chosen zoom level | The **top-level view**: `memory_list` returns a summary row per bundle (name, concept count, last-updated); `memory_graph` returns the bundle-level graph (bundles as nodes, cross-bundle links as edges, FR-019's top level). | **Focus** on that bundle: `memory_list` returns that bundle's concepts (flattened across nesting, with pagination); `memory_graph` returns that bundle's concept-level graph — its concepts as nodes, same-bundle links as edges, plus a synthetic *boundary* node (and edge) for every other bundle it links out to (never a silently-dropped outward link). Exact field-by-field graph output shape: `contracts/memory-storage-trait.md`. |

`memory_list`'s and `memory_graph`'s `None` deliberately match each other (both "top level") —
list and graph are the two ways of *browsing*, so they zoom the same way. `memory_read` is a
genuinely different shape (search, not browse) and keeps a different default on purpose: a
bundle is an organizational container with no embeddable content of its own, so "the top-level
search result" isn't a meaningful thing to return — the useful default is "search everywhere,"
narrowing to one bundle only when the agent already knows that's where the answer lives.

This is a **presentation choice each tool makes**, not a value forwarded unchanged into a
same-named storage parameter — see `contracts/memory-storage-trait.md`'s note on why
`get_all_agent_memory`'s own `None` (used internally for reranking, migration, and reconciliation)
stays "every bundle" even though the `memory_list` *tool* now presents a top-level bundle view by
default. Nothing here changes what an agent can accomplish (FR-001): everything reachable via a
single flat `memory_list()` call today is still one additional, obvious step away — call
`memory_list(bundle)` for each bundle the top-level call named (for the common case of an agent
with only its `default` bundle, that's exactly one extra call, and the agent doesn't need to
change anything on its own — the updated tool description and `BOOT.md` doctrine teach the new
shape the moment this ships, since there's no compiled call site to go stale).

`memory_write`'s existing `slug` field becomes documented as accepting a multi-segment path
(e.g. `"friends/bred"`) to place the concept in a nested subdirectory (FR-007); its `content`
field's description is updated to teach both link forms (FR-004, FR-013, FR-022):

- Same-bundle: `[label](path/to/concept.md)`
- Cross-bundle, specific concept: `[[bundle/slug]]`
- Cross-bundle, whole bundle: `[[bundle]]`

`memory_write`'s `visibility`/`shared_to` fields are **removed** — every memory is private to
the writing agent (FR-015, spec clarification). Any existing tool-call sites or serialized agent
few-shot examples referencing them must be updated or dropped.

## Exploration recipe: how an agent finds its way around bundles + the graph

No new tool is introduced for this (Principle I — the existing seven already cover it once
`bundle` is wired through per the table above). Two independent entry points, since browsing and
searching answer different questions:

**Browsing (no idea yet what exists, or which bundle something is in):**

1. **See what bundles exist and how they connect** — call `memory_list` or `memory_graph` with
   no `bundle`. `memory_list()` gives a flat summary table (name, concept count, last updated);
   `memory_graph()` gives the same set as a graph with cross-bundle links as edges. Either is the
   tool-level equivalent of the WebUI's top-level view (FR-019). There's no separate
   `memory_bundles`/`list_bundles` tool because these two already answer "what bundles do I
   have," each in the shape suited to what you're about to do next (skim a list vs. see how
   things connect).
2. **Look inside one bundle** — call `memory_list(bundle)` (flat, paginated listing) or
   `memory_graph(bundle)` (link structure) *with* that bundle named. Both return every concept in
   that bundle regardless of nesting depth (subdirectories are an addressing detail only, not a
   navigation level — there is no "descend one directory at a time" step; a nested concept like
   `friends/bred` simply appears in its owning bundle's listing/graph with that path). This is
   the tool-level equivalent of "opening a bundle" in the WebUI, and of opening that bundle's
   `index.md` by hand.
3. **Follow a specific link, or open a specific concept** — call `memory_follow` on a known
   `(bundle, path)` to traverse its same-bundle and cross-bundle links (FR-003/FR-013), or
   `memory_detail` to fetch one concept's full content directly once you know where it lives.

**Searching (you have a question, not a browsing target):**

- Call `memory_read` with a query, no `bundle`, to search everywhere at once — this is the
  default because relevance, not bundle organization, is what a search is for. Only narrow with
  `bundle` once you already suspect (from context, or from a prior browse) that the answer lives
  in one particular bundle and want to exclude cross-bundle noise.

An agent that already knows exactly which bundle and path it wants can skip straight to step 3,
or straight to `memory_read` if it just has a question — this recipe describes progressive
discovery from "no idea what exists" down to "read this one memory," matching the index
document's own stated purpose (FR-017: "so a developer *or agent* can see what's available
without opening every concept document individually").

## Discovery channel (FR-022)

Two places must be updated together, per the spec's clarification that no new discovery channel
is introduced:

1. `src/agents/agent/system_prompt/boot.rs` — the `## Memory` section (see
   `contracts/boot-doctrine.md`).
2. Each tool's `description()` and `Input` field-level doc comments in
   `src/agents/tools/vector_memory/mod.rs`.

Both must teach: bundles and how a write lands in one, the same-bundle markdown-link form, the
two cross-bundle wikilink forms, and that nested subdirectories are addressed by multi-segment
paths.
