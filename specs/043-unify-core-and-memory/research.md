# Research: Unify Core and Memory

Phase 0 decisions. Each resolves an unknown left open by the plan or a rule the design could get wrong in more than one way.

---

## Decision 1 — The rules live in `write_memory_inner`, not in the tools

**Decision**: the reserved-bundle guard, the content cap, the soft-limit warning and the migration bypass are all enforced in `BundleMemoryStore::write_memory_inner` (`storage/memory_bundle.rs:1099`), the only function that writes a concept document.

**Rationale**: every write path already funnels through it — the agent's `memory_write` tool, the HTTP create/update service, revision rollback and bundle import. FR-022 requires the limits to apply "from any path", and enforcing them at one choke point is the only way to be sure of that without auditing each caller. It is also Principle II: one implementation, no per-call-site branching.

**Alternatives rejected**:
- *Validate in the `MemoryWrite` tool.* HTTP writes, imports and rollbacks would bypass it, silently — the worst possible failure for a limit, because it would look enforced.
- *Validate in `service::memory`.* Same problem, plus the agent's own writes do not go through the service layer at all; they call storage directly.

**Consequence**: `write_memory_inner` grows a `WritePolicy` argument carrying the hard ceiling, the soft limit and a `bypass_cap` flag. One struct rather than three positional booleans.

---

## Decision 2 — The cap is measured on content bytes, and only on content

**Decision**: the ceiling and soft limit compare `content.len()` — the markdown body, after the caller's content is received and before frontmatter is serialised.

**Rationale**: the frontmatter carries title, tags, relations, timestamps and attachments, all of which the system writes and rewrites. Counting them would make the same document's size fluctuate with how many links it has, which is not what "one memory is one thing" means. FR-016 says "excluding metadata such as title, tags, and links" for exactly this reason.

**Consequence**: a document whose *body* is at the ceiling will be slightly larger on disk. That is fine and must not be "fixed" later by counting the file.

---

## Decision 3 — Legacy over-cap documents are grandfathered; the bypass is one flag, used once

**Decision**: `WritePolicy::migrating()` sets `bypass_cap`, and only the migration constructs it. For any later write, if the stored document is already above the ceiling, the write is accepted when the new content is **shorter** than the stored content, and refused otherwise.

**Rationale**: FR-019 and FR-021 require an over-cap legacy memory to stay readable and shorten-able, or the agent can never bring it under the limit — the limit would become a permanent trap on exactly the documents it exists to fix. FR-025 requires the migration itself never to be blocked, so a legacy identity larger than 4 KiB still migrates.

**Alternatives rejected**:
- *Refuse all writes to an over-cap document.* The document becomes unmaintainable; the only escape is deletion.
- *Let the bypass apply forever.* The cap stops meaning anything the first time someone imports a bundle.

---

## Decision 4 — The warning travels as a separate field on the write result, not in the message text

**Decision**: `MemoryOpResponse::Memory` becomes `MemoryOpResponse::Written { memory: Memory, warning: Option<WriteWarning> }`, where `WriteWarning` carries `{ bytes, limit }`. The `memory_write` tool renders the success message as today and appends the warning in a distinguishable form.

**Rationale**: FR-018 requires the warning to be distinguishable from the success, because an agent that reads "written … over comfortable size" as one sentence may treat the write as failed and retry it. A separate field makes that impossible in the data, not in a rendering convention that a future caller can flatten.

**Alternatives rejected**:
- *A single string with a sentence appended.* The first caller that reformats the string loses the distinction, and the agent's own reading of it is the thing at stake.
- *A log line only.* The agent never sees it; that is the defect being fixed.

**Consequence**: the HTTP `update`/`create` response shapes gain an optional `warning` field. That is additive, so no client breaks.

---

## Decision 5 — The refusal is recognised structurally, not by matching its prose

**Decision**: `service::memory::op_error` currently maps error text to a status. The cap refusal gets a **prefix marker** it can match (`memory too large:`), in the same style as the existing `"already exists"` / `"linked in the knowledge graph"` cases, and maps to **422 Unprocessable Entity** with the full message preserved.

**Rationale**: Principle V wants one error type, and the storage layer speaks `anyhow`. A typed error would mean widening `VizierError` or threading a new error enum through storage for one case — the abstraction the constitution says to add at the second use, not the first. A stable prefix is honest about being string-based and is already the established pattern in this file.

**Alternatives rejected**:
- *A new `MemoryTooLarge` error variant threaded from storage to the service.* More type safety, but it forces every storage signature to change for one case and duplicates what the prefix already achieves.
- *500 Internal.* Wrong: the caller's input is the problem, and the agent must be able to tell a refusal from a server fault.

---

## Decision 6 — Identity is injected as its own instruction block, at the position the CORE block holds today

**Decision**: the soul's `<memory bundle="core" path="soul">…</memory>` block is pushed as a `Message::system(…)` at the same index in `prepare_system_prompts` that the CORE string occupies today (`agents/agent/mod.rs:488`) — after the boot/sandbox/agent-prompt block, before project identity and configured documents.

**Rationale**: FR-007 requires the wrapper to be byte-stable across turns. Building it from the document alone achieves that (no timestamp, no `updated_at`, no per-turn data). Keeping today's position means the cacheable prefix's *shape* is unchanged, which is the simplest way to satisfy "the wrapper is byte-stable" without inventing a new ordering to reason about.

**Alternatives rejected**:
- *Put it in the per-turn context block with the retrieved excerpts.* That block is deliberately outside the cacheable prefix and varies per turn; identity must not, and it would also make identity compete with retrieved memory for the same budget.
- *A new prompt section with its own heading.* Renames a thing the agent already reads without adding capability.

**Note**: the spec deliberately does not *require* this position (FR-007 names no slot); it is a planning choice that may be revisited without a spec change.

---

## Decision 7 — The migration runs at agent spawn, beside the passage reconcile, not in `dependencies.rs`

**Decision**: the `core/soul` seed runs in `supervisor.rs` where `reconcile_agent_passages` and `reconcile_agent_artifact_references` already run, as a detached task with a per-agent marker, while `migrate_agent_cores` in `dependencies.rs` keeps only its existing moves.

**Rationale**: writing `core/soul` needs the **agent's indexer** to write passages — and the indexer is built per agent from that agent's embedding config (`VizierAgents::build_indexer`), returning `None` when the agent has no embedding. `dependencies.rs` has no indexer at all, which is exactly why the two existing memory migrations there pass `VizierIndexer::build(NoopIndexer)` and leave semantic search to catch up later (spec 009 research Decision 8). Migrating identity is a memory write, so it belongs where the memory infrastructure exists.

**Alternatives rejected**:
- *Do it in `dependencies.rs` with a noop indexer.* The soul would exist but be unsearchable until some later reconcile, which is a silently degraded state for the one document that matters most.
- *Block agent start until it finishes.* Violates the "agent serves requests throughout" property the passage conversion deliberately preserves.

**Consequence**: the #82 entry says `migrate_agent_cores` "gains a third source", which is slightly off — the seed lives at spawn. The issue text is corrected in this change to name the spawn path.

---

## Decision 8 — Dream changes are classified by origin address, computed in the query

**Decision**: `ChangeKind::Core` is assigned to a `memory_revision` row whose `bundle` is the reserved bundle, and the separate `core_revision` query is dropped from `list_dream_revisions`. `DreamRevision` keeps `bundle`/`path` for these rows instead of `None`.

**Rationale**: FR-031 says classification follows the bundle a change lives in, not the record it was written to. After the migration there is exactly one place identity changes are recorded — `memory_revision` — so the second query has nothing to read, and the first query already returns the right rows with `bundle = "core"`. Implementing FR-031 therefore means *deleting* a query and adding a predicate, not adding a mechanism.

**Consequence**: `tally` currently counts `Core` only as a boolean (`core: bool`). FR-033 requires a count, so `DreamNight`/`DreamCycle` gain `core_changes: u32` beside the existing `core: bool`. The WebUI's `NightStrip` and `DreamJournalEntry` read that count.

**Alternatives rejected**:
- *Keep both queries and tag by table.* That is the behaviour FR-031 rejects: an agent adding `core/values` would not count as an identity change.

---

## Decision 9 — The identity document path is a constant, and `soul` is not a special case anywhere else

**Decision**: `schema::storage` gains `CORE_BUNDLE: &str = "core"` and `SOUL_PATH: &str = "soul"`. The bundle guard checks only `bundle == CORE_BUNDLE`, and accepts only `path == SOUL_PATH` (FR-013). Everything else — listing, search, links, revisions, delete — treats the document as ordinary.

**Rationale**: the fewer places know a path, the fewer can disagree. The prompt builder needs the address; the guard needs the pair; nothing else does. In particular the *deletion* path needs no special case at all, because FR-009 makes the soul removable.

**Consequence**: the prompt builder asks for `(CORE_BUNDLE, SOUL_PATH)` rather than reading a field, so a removed soul is simply `None` and the default block is rendered (FR-008).

---

## Decision 10 — The default identity is rendered from the embedded template, and the empty case is a template, not an empty message

**Decision**: when `core/soul` is absent or empty, the prompt gets the template text wrapped in the same `<memory>` block, and a note that the document is missing and can be written at that address.

**Rationale**: FR-008 forbids an empty identity section, and today's behaviour already falls back to `CORE_MD`. Keeping the fallback means a deleted soul is a *blank identity to fill in*, not a missing instruction — and the note is what tells the agent it can restore it, which is the discovery fix FR-010 asks for.

**Alternatives rejected**:
- *Omit the block entirely when the soul is missing.* That is precisely the empty-slot bug the fallback exists to prevent.

---

## Decision 11 — The deprecated API is a projection with no second store

**Decision**: `service::core` reads and writes through the **memory** operations (`memory_read`-equivalent and `write_memory` on `CORE_BUNDLE`/`SOUL_PATH`), not through `agent_core`/`core_revision`. Its history and rollback project `memory_revision` for that document, mapping `seq`, `actor` and `trigger` into the existing `CoreRevision` shapes so the responses are byte-compatible.

**Rationale**: FR-035 requires the deprecated surface and the memory surfaces never to disagree, and SC-013 measures it. A projection cannot disagree with its source; a copy can.

**Consequence**: `get_agent_core`/`set_agent_core` become thin wrappers over the document (kept for #82), `agent_core` becomes a read-only legacy source, and `core_revision` stops being written. All three are tracked in #82.

**Alternatives rejected**:
- *Keep `agent_core` as the store and make memory read from it.* Inverts the direction: then memory could disagree with itself, which is worse.

---

## Decision 12 — The WebUI loses two routes and gains nothing

**Decision**: `routes/agent-core.tsx` and `routes/agent-core-edit.tsx` are deleted along with their entries in `routes.ts` and `pages.ts`, and the `"The agent"` menu group (which holds only Core) goes with them. The reserved bundle appears in the memory explorer, protected from bundle deletion, linking to the existing document and revision surfaces.

**Rationale**: US4's premise — removing the second place to look — is only satisfied if the second place actually goes. The memory explorer already supports everything the Core page did: document view, revision history, diff, restore, stale-save refusal. The page had no behaviour the memory surfaces lack.

**Alternatives rejected**:
- *Redirect `/core` to the memory document.* Leaves the address alive and keeps the idea that identity has its own address. FR-029 names this explicitly as not what to do.
- *Keep the page, strip the editor.* Still a second place to look.

**Note**: the memory explorer must render the reserved bundle's protection (bundle delete disabled, "reserved" badge) or the person can attempt a deletion that will be refused at the API. That is the one UI addition.
