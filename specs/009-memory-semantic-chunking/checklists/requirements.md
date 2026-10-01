# Specification Quality Checklist: Semantic Chunking for Memory Recall

**Purpose**: Validate specification completeness and quality before proceeding to planning
**Created**: 2026-09-30
**Feature**: [spec.md](../spec.md)

## Content Quality

- [x] No implementation details (languages, frameworks, APIs)
- [x] Focused on user value and business needs
- [x] Written for non-technical stakeholders
- [x] All mandatory sections completed

## Requirement Completeness

- [x] No [NEEDS CLARIFICATION] markers remain
- [x] Requirements are testable and unambiguous
- [x] Success criteria are measurable
- [x] Success criteria are technology-agnostic (no implementation details)
- [x] All acceptance scenarios are defined
- [x] Edge cases are identified
- [x] Scope is clearly bounded
- [x] Dependencies and assumptions identified

## Feature Readiness

- [x] All functional requirements have clear acceptance criteria
- [x] User scenarios cover primary flows
- [x] Feature meets measurable outcomes defined in Success Criteria
- [x] No implementation details leak into specification

### Automatic-context risks surfaced in review

Four concerns found by reading the current implementation rather than the spec, all now covered by requirements:

- **The retrieval query is the raw user message** (`src/agents/process.rs:782`). On short or referential messages ("ok", "do that one") the embedding retrieves unrelated content. Harmless while only titles are surfaced; actively misleading once passages are. FR-029 declines to retrieve in that case, SC-010 measures it. Retrieving against a window of recent turns would likely beat any threshold tuning, but that changes what the query *is* and is left to a later feature.
- **`SilentRead` is the high-volume path and is easy to overlook** (`src/channels/discord/mod.rs:508` sends it for every non-mention message in a guild channel, injected at `process.rs:817`). Passage-level injection there scales with channel traffic. FR-030 gives it an independent budget; SC-011 requires it to default to zero.
- **The threshold, not the passage count, is the economic mechanism.** Because FR-026 omits the section when nothing qualifies, how often retrieval fires decides whether the feature saves or spends. Five caps the damage when precision is poor, but it does not create precision. The existing 0.5 was never derived from anything, and stored session history makes the distribution measurable before implementation — recorded in Assumptions as a plan-phase task.
- **Injected passages are third-party-derived data placed in the agent's own turn.** Memory content can originate from channel messages the agent chose to remember, and the block is prepended to the user message. FR-031 requires it be delimited and labelled as reference material, not instruction.

## Notes

- Items marked incomplete require spec updates before `/speckit-clarify` or `/speckit-plan`

### Validation log

**Iteration 1** — three issues found and fixed:

1. *All acceptance scenarios are defined* — User Story 4 was missing its **Independent Test** line. Added.
2. *No implementation details* — an edge case named the "embedding/relevance backend"; the Assumptions section named "relevance/embedding machinery". Both reworded to "relevance-matching service"/"relevance-matching capability", which states the dependency without naming the mechanism.
3. *Requirements are testable and unambiguous* — User Stories 1 and 2 were both marked P1, leaving their phasing ambiguous for `/speckit-plan` and `/speckit-tasks`. Re-ranked to P1–P4 so each story is a distinct, independently shippable slice.

**Iteration 2** — all items pass.

**Iteration 10** — user challenged a story they had not asked for: *"i don't remember defining P3."* Correct — it was extrapolated in the first draft, and review found the web interface has no memory search screen at all (`webui/app/services/vizier.tsx` defines `queryMemories`, nothing calls it), so the story proposed building a screen rather than adapting one. Cut it. What it was standing on is genuine and became FR-017: the HTTP search endpoint returns `Vec<MemoryDetail>` from the same `query_memory` call, so its response shape has to change regardless — with no UI work in scope. Renumbered FR-017 onward. 42 requirements, 13 criteria, 2 stories.

**Iteration 9** — user reinstated automatic retrieval with a lower count: *"let's proceed with auto-retrieval, but instead of 10 recommendation, can we limit it to 5 so it will be more budget friendly."* Restored User Story 2 and the automatic-context requirements, with FR-021 pinning the default at five passages (down from today's ten documents) plus a configurable total size cap, and FR-023 capping any one document's share so a single long memory cannot fill all five slots. Restored the four review-derived requirements (query gate, per-path budget, data framing, hit/miss logging) and SC-009 through SC-013. The spec was rewritten whole at this point rather than patched, for clean numbering: 41 requirements, 13 criteria, 3 stories. The open budget question is now closed. All items still pass.

**Iteration 8** — user deferred automatic retrieval: *"let's leave auto-retrieval as is for now, we will push it to another story another time."* Removed User Story 2 and all twelve automatic-context requirements, four edge cases, and five success criteria; User Story 3 became User Story 2. Replaced them with FR-019 and FR-020, which preserve the lookup's present output while re-sourcing it from passage matches — necessary because retiring per-document relevance data removes the index it currently queries — plus SC-009 as a regression guard. Renumbered FR-031-040 to FR-021-030. The review findings were kept as a handoff section for the follow-up feature. 30 requirements, 9 criteria, 2 stories. All items still pass.

**Iteration 7** — reviewed the automatic-context design against the implementation. Added FR-027 (no retrieval when the message is not a usable query), FR-028 (per-request-kind budget, so the high-volume observation path is independent), FR-029 (injected passages are delimited reference data, not instruction), FR-030 (record whether an injected turn searched anyway, so thresholds are tuned on evidence). Renumbered FR-027-036 to FR-031-040. Added SC-010 (selectivity, replayed against stored messages) and SC-011 (observation path defaults to zero), renumbering the rest. 40 requirements, 13 criteria. All items still pass.

**Iteration 6** — user deferred partial reads: *"let's drop partial read for now."* The reading story collapsed from eight acceptance scenarios to two requirements — read a memory in full by its address, and report clearly when it no longer exists — so it no longer carries a slice of its own and was folded into User Story 1 as that story's escape hatch. User Story 4 became User Story 3. Removed the line-range, passage-range, heading, multi-path, span-reporting and widening requirements, the Read Selection entity, three edge cases, and the partial-read success criterion. Renumbered FR-025-042 to FR-019-036 and SC-012 to SC-011. 36 requirements, 11 criteria, 3 stories. All items still pass.

**Iteration 5** — user dropped the opt-in whole-document search mode: *"i don't think we need whole_documents."* Removed the requirement, its two edge cases, the cross-mode consistency criterion, and the whole-document-usage criterion; simplified FR-008 and FR-012 to a single search behaviour; repointed FR-039 at the read path. Renumbered FR-018-043 to FR-017-042 and SC-011-014 to SC-009-012, cross-references included. Recorded the reasoning as an assumption so the decision is not relitigated. All items still pass.

**Iteration 4** — user proposed the concrete capability surface: search returning passages with sources, a read that can take whole or partial documents (including explicit line ranges), and automatic related-memory recommendations behaving like search instead of returning bare titles. All three adopted. Spec restructured around those three capabilities: the old "buried facts" story merged into User Story 1 (its distinct work was per-passage ranking and per-document caps, not a separate slice); automatic context promoted to User Story 2 at P2 because it runs every turn with no agent judgement; "widening a snippet" generalised into User Story 3, reading whole or in part. Added requirements for reading and for automatic context, FR-007 extended to record both line and character spans, FR-009 extended to a full address. Added four success criteria for automatic context, partial reads and cache preservation. All items still pass.

**Iteration 3** — user revision: *"i think we could still keep the whole document recall if agent needs it."* FR-026 rewritten so that only whole-document *relevance data* is retired, not whole-document *results*; added FR-008a–008e for the opt-in mode (per-call selection, ranked by best-matching passage, deduplicated, lower default result count, tool-description guidance); scoped FR-012–016 across both modes; extended User Story 3 with three acceptance scenarios; added two edge cases and SC-009 (the modes must agree on which documents matched) and SC-010. All items still pass.

### Deliberate judgement calls

These passed validation but are worth a reviewer's attention, since they were decided here rather than asked about:

- **Search returns passages only; whole documents come from a read** (FR-008, FR-018, FR-039). This went back and forth: the first draft removed whole-document results entirely, the user asked for them back, and then dropped them again once the read capability made the search mode redundant. Final position is the simplest of the three — search never returns a document body, and a read returns the complete document. Breaking change to both the agent-facing search tool and the HTTP memory-search response shape.
- **Automatic context inverts the cost direction, and that is the main risk in the feature.** Verified in the code: `src/agents/process.rs:782` and `:817` fetch the top 10 memories at threshold 0.5 every chat turn, and `src/agents/agent/system_prompt/context.rs:33` renders only title + slug. So today that path is cheap and nearly useless. At the agreed five passages the one unconditional path still costs roughly 15x what ten titles do, so the count alone does not make it affordable. FR-022 (five passages plus a size cap), FR-023 (stricter threshold than search) and FR-024 (per-document share cap) bound it, and SC-009/SC-012 measure whether it pays for itself by displacing tool calls. If the threshold is set loosely, this feature increases total token spend whatever the count.
- **Prompt caching was checked, not assumed.** Commit 52a8487 moved per-request context out of the system prompt and into the current user message, so growing the automatic-context block does not invalidate the cached prefix. FR-025 and SC-013 pin that property down so a later refactor cannot quietly undo it.
- **Tool naming is a correctness concern, not cosmetics.** `memory_read` is currently the *search* tool and `memory_detail` the document reader — both names inverted. Resolved in Assumptions with a full before/after mapping: search becomes `memory_search`, `memory_read` becomes the literal document reader, `memory_detail` is retired rather than aliased, and the other six tools are untouched. The `memory_*` prefix is kept over `search_memory`/`read_memory` because name-sorted tool definitions would otherwise place `read_memory` beside the unrelated `read_image` and split the family across the alphabet. Reusing `memory_read` for a new meaning is accepted on the grounds that the schema change makes a stale call fail validation loudly rather than misbehave quietly.
- **Partial reads were specified, then deferred.** Reading by passage range, line range, or heading is out of scope for this feature; the reasoning is recorded in Assumptions. The decision is reversible at no cost — partial reads add no stored data and change no response shape anything depends on.
- **The escape hatch is now unmitigated, and that is the known cost of deferring partial reads.** An agent whose passage is insufficient has exactly one option: pull the entire document. There is no cheaper intermediate and no success criterion holding that behaviour in check, because with partial reads gone there is nothing to measure against. The bet is that good chunking (FR-001-FR-004) makes most passages self-sufficient and that automatic context answers the common case with no read at all. Worth watching once this ships: if searches are routinely followed by a full read, the feature is returning its savings and partial reads should come back.
- **Chunk boundaries are mechanical, not model-decided.** Recorded as the first assumption. If the intent was LLM-determined boundaries, the spec needs revisiting before planning — it changes cost and determinism on every memory save.
- **Documents remain the unit of versioning and linking.** Passages are derived and rebuildable; nothing in this feature versions a passage or attaches links to one.
- **Scope is memory concept documents only** — CORE identity documents, session files, dream journals, and conversation history are excluded (final assumption).
