# Research: Version History for CORE.md and Memories

**Feature**: `006-memory-version-history` | **Date**: 2026-09-15

All Technical Context items were resolvable from the codebase; no `NEEDS CLARIFICATION` remained after `/speckit-clarify`. The decisions below settle the "how" for each spec requirement.

## Decision 1 — Where revisions are recorded: inside the single write path per document kind

**Decision**: Record the revision inside `BundleMemoryStore::{write_memory, delete_memory, delete_bundle, import_bundle}` (`src/storage/memory_bundle.rs`) for memory documents, and inside `SqliteStorage::set_agent_core` (`src/storage/sqlite/agent.rs`) for CORE. Callers never call a "record revision" API themselves.

**Rationale**: Every save already funnels through exactly one implementation per kind — `BundleMemoryStore` is the sole `impl MemoryStorage for SqliteStorage` (CLAUDE.md), and `agent_core` is written only by `set_agent_core`. Recording there gives FR-001's "every save, regardless of who" for free and makes SC-001 (100% capture) structurally true: the ten call sites found (`src/agents/tools/workspace/mod.rs:49`, `src/agents/tools/vector_memory/mod.rs:351/818/865`, `src/agents/memory_ops.rs:39/85/93/101`, `src/channels/http/api/v1/agents/core.rs:105`, `src/agents/mod.rs:217`, `src/dependencies.rs:426/745`) cannot forget to record. This is Principle II applied: one implementation, not ten copies.

**Alternatives considered**: (a) Record at each call site — rejected, ten copies of the same logic and any future caller silently escapes history. (b) A wrapper `VizierStorage` decorator — rejected, `VizierStorage` is a hand-forwarding newtype; a decorator adds a layer for no second use case (Principle I).

## Decision 2 — Provenance is a `RevisionOrigin` value threaded through the save signatures

**Decision**: Add `origin: RevisionOrigin { actor: RevisionActor, trigger: RevisionTrigger }` as a parameter on `MemoryStorage::{write_memory, delete_memory, delete_bundle, import_bundle}` and `AgentStorage::set_agent_core`, and as a field on the corresponding `MemoryOpRequest` variants (the HTTP → agent memory-ops channel). Two constructors cover every caller:

- `RevisionOrigin::from_session(&VizierSession)` — for agent tools. `VizierChannelId::Dream(..)` ⇒ `trigger = Dream`; anything else ⇒ `trigger = Conversation`. `actor = Agent`. (`ToolContext.session` is already passed to every `VizierTool::call`, `src/agents/tools/mod.rs:71`.)
- `RevisionOrigin::from_user(&AuthenticatedUser)` — for HTTP handlers. `actor = User { user_id, username }`; `trigger = WebUi` when the request was authenticated with a JWT bearer token, `Api` when authenticated with an API key. This requires adding `auth_method: AuthMethod` to `AuthenticatedUser` (`src/channels/http/auth/mod.rs:112`), set in `auth/middleware.rs` where the two branches already exist (lines ~115 and ~130).
- `RevisionOrigin::system(RevisionTrigger::Baseline)` — startup migrations and agent-creation CORE seeding.
- Import handlers use `from_user(..)` then override `trigger = Import`; rollback handlers use `trigger = Rollback { restored_from: seq }`.

**Rationale**: The spec requires actor + trigger on every entry (FR-002). The information exists at each boundary (session for tools, auth for HTTP) but not inside storage, so it must be passed in. A single struct with a couple of constructors keeps the dispatch out of the tools (no per-tool `match` on channel type).

**Alternatives considered**: (a) Task-local / thread-local "current origin" — rejected: memory ops hop across a flume channel into the agent's memory-ops task, so ambient context would be lost or wrong. (b) Deriving trigger from session for HTTP too (`VizierChannelId::HTTP(user, id)`) — rejected: HTTP memory writes don't carry a session, and it can't distinguish WebUI from API key.

## Decision 3 — Snapshot format: one canonical text per kind, so diff/no-op are plain string operations

**Decision**: Store `content` as a single text column:
- **CORE**: the raw CORE markdown exactly as saved.
- **Memory**: a canonical markdown document = YAML frontmatter holding only the user-authored fields (`title`, `tags`, `attachments`) + the body, produced by the existing `serialize_markdown` helper with a small `RevisionFrontMatter` struct. `created_at`/`updated_at`/`read_count`/`keywords`/`relations` are deliberately excluded — they are derived or bookkeeping and would make every save look changed.

No-op detection (FR-003) is `new_canonical == latest.content`. Diff (FR-008/009) is a text diff of two canonical strings — title/tag changes show up as frontmatter lines in the same diff. Rollback parses the canonical text back with `parse_markdown_bytes::<RevisionFrontMatter>` and re-saves through `write_memory`.

**Rationale**: One representation, one diff path, one equality check for both kinds; human-readable in the API; round-trips losslessly for rollback. Storing the on-disk bytes verbatim was rejected because `updated_at`/`read_count` churn would defeat FR-003 and pollute diffs.

**Alternatives considered**: Separate `title`/`tags`/`body` columns with a structured diff — rejected as more code for the same user-visible result; a JSON blob — rejected, not diff-friendly.

## Decision 4 — Line diff computed server-side with the `similar` crate

**Decision**: Add `similar = { version = "2", default-features = false, features = ["text"] }`. A single `fn diff_lines(from: &str, to: &str) -> Vec<DiffHunk>` in `src/storage/revision.rs` normalizes `\r\n` → `\n`, runs `TextDiff::from_lines`, and groups ops into hunks (`grouped_ops(3)`) of `{ op: "equal"|"insert"|"delete", old_line, new_line, text }`. Both the CORE and memory diff endpoints call it; the WebUI only renders.

**Rationale**: FR-008/FR-009 need a correct diff on arbitrary documents; the API must expose it (FR-016), so the computation belongs server-side. See Complexity Tracking in `plan.md` for why this passes the Principle I "hand-rolled few lines" test. `similar` is pure Rust — no impact on `Cross.toml` targets (Principle IV).

**Alternatives considered**: (a) Hand-rolled LCS — O(n·m) memory, unacceptable for larger CORE files. (b) Hand-rolled Myers — ~150 lines plus hunking plus tests; more code to own than the dependency. (c) npm `diff` in the WebUI — a second dependency and no API diff.

## Decision 5 — Baseline version created lazily on first access (FR-018)

**Decision**: `ensure_baseline(conn, agent_id, kind, bundle, path, current_content: Option<&str>)` is called (a) at the start of `record(..)` before inserting a new revision, and (b) by `list_revisions` when a document currently exists but has zero revisions. If there are no revisions and `current_content` is `Some`, it inserts seq 1 with `actor = System`, `trigger = Baseline`. For memory, "current content" is the canonical text of the existing on-disk document (already read by `write_memory` as `existing`); for CORE it's the current `agent_core.content` read in the same transaction.

**Rationale**: Guarantees the first real change after upgrade has something to diff against, without a startup migration walking every document of every agent (which would also fight the DocumentStore reconciliation model). Cheap: one indexed `SELECT COUNT` per save on an empty history, none afterwards.

**Alternatives considered**: Eager startup migration — rejected; touches every document on every deployment for a benefit only realized on first access anyway.

## Decision 6 — Memory history reads and rollback go through the memory-ops channel; CORE goes direct

**Decision**: Memory rollback is `MemoryOpRequest::Rollback { bundle, path, seq, origin }` dispatched in `src/agents/memory_ops.rs` to a new `MemoryStorage::rollback_memory(agent_id, bundle, path, seq, origin, indexer)`. Memory history *reads* (`ListRevisions`, `GetRevision`, `DiffRevisions`) are also routed through the channel for symmetry with every other memory route in `memory.rs` (they all use `state.transport.send_memory_op`), even though they don't need the indexer. CORE history reads and rollback call `state.storage` directly, like `get_core`/`update_core` do today.

**Rationale**: `rollback_memory` re-saves content and must update the embedding index owned by the agent's memory-ops task (FR-012: "identical to a manual save"), so it has to run there. Keeping the read routes on the same channel means `memory.rs` keeps one pattern instead of two.

**Alternatives considered**: Read directly from `state.storage` for memory history — works, but introduces a second access pattern into a file that consistently uses the channel; not worth the inconsistency for a few ms.

## Decision 7 — Rollback is a re-save, not a history rewrite

**Decision**: Rollback = load the target revision's `content` → for CORE call `set_agent_core(agent_id, content, origin{trigger: Rollback{restored_from}})`; for memory parse the canonical text and call `write_memory(.., origin{trigger: Rollback{restored_from}})` with `create_only = false`. If the target content equals the current content, `record` returns `None` and the endpoint answers `200 { no_change: true }` (spec US3 scenario 6). Restoring a deleted memory is the same path — `write_memory` recreates the document (spec US3 scenario 5).

**Rationale**: FR-011/FR-012 — rollback must be attributed, must append, must never mutate history, and must behave like a manual save (index, links, `index.md`/`log.md`, graph cache all refreshed by the normal path).

## Decision 8 — Transactionality and failure semantics

**Decision**: CORE: `set_agent_core` does the `agent_core` upsert and the `core_revision` insert inside one `rusqlite` transaction; either both commit or the save fails. Memory: the document `put` to the `DocumentStore` happens first (as today); the `memory_revision` insert follows in the same `conn.lock()` scope as the `memory_node` upsert. If the revision insert fails, the save returns `Err` (the document is on disk; the next successful save records it, and `ensure_baseline` covers the zero-revision case) — same durability story the graph index already has (it is "derived, reconcilable"). A failed document write records nothing (spec edge case).

**Rationale**: The DocumentStore is not transactional with SQLite, so perfect atomicity is impossible for memory; the chosen ordering never produces a revision without a matching document write, which is the invariant that matters for history integrity.

## Decision 9 — Bundle deletion records one deletion entry per concept

**Decision**: `delete_bundle(force = true)` already iterates each concept path; it records a `deleted` revision per concept (same helper as `delete_memory`) before removing the files. Non-force deletion only removes `index.md`/`log.md` and records nothing (those are not versioned documents).

## Decision 10 — Agent deletion cascades; history keyed by `agent_id`

**Decision**: `SqliteStorage::delete_agent` already deletes `agent_core`; add `revision::delete_agent(conn, agent_id)` there, which clears both `core_revision` and `memory_revision` (FR-019). Nothing else references revisions.

## Decision 11 — Pagination shape mirrors `PaginatedMemory`

**Decision**: `PaginatedCoreRevisions` / `PaginatedMemoryRevisions` = `{ revisions: Vec<…Summary>, total, offset, limit }`, newest-first, default `limit = 50`, max 200. Summary rows exclude `content` so listing 1,000 versions is a small payload (SC-005). Full content is fetched per version.

## Decision 12 — WebUI: one shared `VersionHistory` component, no new npm dependency

**Decision**: `webui/app/components/VersionHistory.tsx` takes a small adapter `{ list, get, diff, rollback }` (four async functions) plus `onRolledBack`, so the CORE route and the memory route pass their respective service functions. Internally: paginated list (newest-first, current highlighted, actor/trigger/timestamp), click → content viewer, "Changes" → diff vs previous, "Compare" mode → pick two → diff, "Restore" → confirm dialog (FR-015) → rollback → `onRolledBack`. Diff renders server hunks with existing CSS variables (`--text-*`, mono font) and two accent backgrounds for insert/delete (theme-aware via existing tokens). CORE page opens it in the existing `SlideOver`; memory view slide-over adds a "History" button that swaps its body to the panel.

**Rationale**: FR-016 (same surfaces as the documents), one implementation for both kinds (Principle II), no new dependency (Principle I / constitution "check whether an existing dependency already covers the need").

## Decision 13 — Known pre-existing limitation: running agent's system-prompt CORE is loaded at spawn

**Finding**: `VizierAgent::new` reads CORE once into `self.core` (`src/agents/agent/mod.rs:176`) and uses it for the system prompt; `WRITE_CORE` and the WebUI `PUT /core` both write storage only. `READ_CORE` reads storage live. Therefore a CORE rollback is visible to the agent immediately via `READ_CORE` and on next spawn via the system prompt — *exactly* the same as a manual WebUI save today (FR-012), which is the contract the spec asks for. Hot-reloading the system prompt after any CORE save is a separate, pre-existing gap and is **out of scope** here; noted so SC-004 ("agent's next read … no restart required") is verified against `READ_CORE`, not the system prompt.

## Decision 14 — Trigger vocabulary (final)

`conversation` · `dream` · `webui` · `api` · `import` · `rollback` (carries `restored_from`) · `baseline`. Actor: `agent` · `user { user_id, username }` · `system`. Serialized as snake_case strings/objects in JSON (`#[serde(tag = "type", rename_all = "snake_case")]`). No `external` actor/trigger (clarified: on-disk edits ignored).

## Decision 15 — CORE and memory history are two separate implementations (user decision, 2026-09-15)

**Decision**: Two append-only tables and two independent code paths:
- `core_revision (agent_id, seq, content NOT NULL, …)`, helpers in `src/storage/sqlite/core_revision.rs`, history methods on `AgentStorage` next to `get_agent_core`/`set_agent_core`.
- `memory_revision (agent_id, bundle, path, seq, content NULL-able, deleted, …)`, helpers in `src/storage/sqlite/memory_revision.rs`, history methods on `MemoryStorage` next to `write_memory`/`delete_memory`.

No new `RevisionStorage` trait, no `DocRef`/`kind` abstraction that selects a table at runtime. The only shared pieces are value types that describe a save or a diff independent of what was saved — `RevisionOrigin`/`RevisionActor`/`RevisionTrigger`, `RevisionDiff`/`DiffHunk`/`DiffLine`, `RollbackResponse` — and the pure `diff_lines` function (`src/storage/diff.rs`).

**Rationale**: CORE and memory are different things: one-per-agent vs `(bundle, path)`-addressed; never deleted vs delete/restore lifecycle; raw markdown vs a canonical frontmatter+body snapshot; stored in SQLite vs stored on disk with a derived graph index. Forcing them through one SQL-generating helper (the earlier `DocRef` draft) would have been an abstraction with exactly two cases, whose only purpose was to look DRY — the constitution's Principle I ("don't introduce an abstraction for a hypothetical second implementation") applies more than Principle II here, which targets *variants of the same kind* (a second storage backend, a second channel). Two straightforward modules with plain SQL are easier to read, and each can evolve on its own (e.g. a note field on memory revisions only).

**Alternatives considered**: (a) Single `document_revision` table with a `kind` column — rejected, two always-empty columns and a `deleted` flag meaningless for half the rows. (b) Two tables behind a shared `DocRef` enum — rejected as an invented abstraction for two cases.
