# Feature Specification: Semantic Chunking for Memory Recall

**Feature Branch**: `009-memory-semantic-chunking`

**Created**: 2026-09-30

**Status**: Draft

**Input**: User description: "i want to implement semantic chunking for memories, so agent can recall only small snippets of a memory, instead of a whole document, for this feature we dont need backward compability"

## User Scenarios & Testing *(mandatory)*

### User Story 1 - Search returns focused passages, not whole documents (Priority: P1)

An agent searches its memory for something it needs right now ("what did the owner say about deployment windows?"). Instead of receiving the full text of every memory document that looked broadly relevant, it receives a handful of short, self-contained passages — each one the specific part of a document that actually answers the query — labelled with which memory it came from and where inside it.

**Why this priority**: This is the mechanism the whole feature rests on, and every other story consumes it. It also carries its own escape hatch: because every result is addressed, an agent that finds a passage insufficient can read that document in full without searching again. Today a single search returns ten complete documents, most of which is text the agent did not need; that text competes for working context, pushes out conversation history, and dilutes every subsequent decision. It also fixes a correctness problem: a long document is currently judged for relevance as one undifferentiated blob, so any single fact buried inside it is drowned out and effectively unsearchable. Ranking passages individually makes buried facts findable on their own merits.

**Independent Test**: Populate an agent with several multi-section memory documents, run a search whose answer lives in one paragraph of one document, and verify the returned text contains that paragraph and is a small fraction of the text the same query returned before the change — while still answering the question.

**Acceptance Scenarios**:

1. **Given** an agent with a 3,000-word memory document whose one relevant paragraph is about deployment windows, **When** the agent searches "deployment windows", **Then** the result contains that paragraph and not the document's unrelated sections.
2. **Given** an agent whose memories are all short (a few sentences each), **When** the agent searches a topic, **Then** each matching memory is returned in full, because the whole memory is already a single passage.
3. **Given** a search that matches passages in four different documents, **When** the results are returned, **Then** each passage is attributed to the memory document and bundle it came from, and to its position within that document.
4. **Given** two passages in the *same* document both match a query, **When** the results are returned, **Then** the agent receives both without the unrelated text between them repeated.
5. **Given** a single long memory covering ten unrelated topics, **When** the agent queries a narrow detail from the eighth topic, **Then** the passage containing that detail is among the returned results — a query that returns nothing useful today.
6. **Given** two memories — one short and entirely about the query topic, one long that mentions it once — **When** the agent queries that topic, **Then** both are represented, ranked by how well each passage matches rather than by which document it belongs to.
7. **Given** a query that matches nothing in the corpus, **When** the agent searches, **Then** it receives an empty result rather than loosely-related passages.
8. **Given** one document whose many passages all match a query, **When** results are returned, **Then** that document's contribution is capped so other memories still appear.
9. **Given** a returned passage that is not enough on its own, **When** the agent reads the document at the address the result carried, **Then** it receives that document in full, with no second search.
10. **Given** a passage whose document has since been deleted, **When** the agent reads that address, **Then** it receives a clear "no longer exists" outcome rather than an error or stale text.

---

### User Story 2 - Related memories arrive with substance, not just titles (Priority: P2)

Every conversational turn already quietly looks up memories related to what was just said and shows the agent a list of them. Today that list is ten document titles and addresses — a teaser that tells the agent "there might be something here, go fetch it", which costs a round-trip and is often ignored. After this change the agent sees up to five actual relevant passages inline, and usually needs no follow-up call at all.

**Why this priority**: This is the path that runs on *every single turn*, with no agent judgement in the loop, so it is where the feature's value and its risk both concentrate. It is also the only place where the current design actively wastes effort: the lookup already happens and already pays for a relevance query, then throws the useful part away. The flip side is that this path can only afford a few passages — unlike a search the agent chose to make, nobody asked for this text — which is why the count drops from ten to five while the payload per item grows.

**Independent Test**: Send an agent a message whose answer sits in one paragraph of one stored memory, and verify the agent answers correctly without making any memory tool call — then verify the injected block held at most five passages and stayed within its configured size budget.

**Acceptance Scenarios**:

1. **Given** a stored memory containing the answer to the user's message, **When** the agent takes its turn, **Then** the relevant passage is present in its context with its source address, and the agent can answer without a follow-up memory call.
2. **Given** more than five memories are related to the message, **When** context is assembled, **Then** at most five passages are injected, the best-ranked ones, and the total stays within the configured size budget.
3. **Given** an injected passage is not enough, **When** the agent wants more, **Then** the passage's address is sufficient to read the whole document, without searching again.
4. **Given** several passages of one document would qualify, **When** context is assembled, **Then** that document's contribution is capped so it cannot consume all five slots.
5. **Given** nothing stored is sufficiently related to the message, **When** context is assembled, **Then** no related-memory section appears at all, rather than weakly-related filler.
6. **Given** the relevance lookup fails or times out, **When** the agent takes its turn, **Then** the turn proceeds without related-memory context rather than failing.
7. **Given** a message that is only "ok" or "thanks", **When** the agent takes its turn, **Then** no retrieval is attempted and no passages are injected.
8. **Given** an agent observing a busy channel without being addressed, **When** messages arrive, **Then** the budget for that path applies independently of direct conversation and can be set to zero.
9. **Given** an injected passage, **When** the agent reads its context, **Then** the passage is delimited and labelled as retrieved reference material that may be irrelevant, not as part of what the user said.
10. **Given** two consecutive turns in one session, **When** prompts are sent, **Then** the cacheable portion of the prompt is unchanged by the differing related-memory content between them.

---

### Edge Cases

**Dividing documents**

- A memory shorter than the minimum useful passage length: it is kept whole as a single passage, never padded or merged with another document.
- A memory with no internal structure — one unbroken wall of text with no headings or blank lines: it is still divided at a bounded size, so it cannot become one giant passage that defeats the feature.
- A structure that must not be cut mid-way (a fenced code block, a table, a list): it stays intact within one passage even when that makes the passage larger than the normal target, up to a hard ceiling.
- A single structure larger than the hard ceiling: it is split, and each part is marked as a continuation, so the agent is not misled into thinking it received a complete block.
- Attachments and non-text content: not divided; they stay with their document and are reached by reading it.

**Searching**

- Two adjacent passages of the same document both match: they are returned as one continuous run of text, not as duplicates overlapping each other.

**Reading**

- A read of an enormous document: the agent gets it in full. There is no narrower read to offer, so the cost is accepted as the price of having an escape hatch at all.

**Automatic context**

- A single qualifying passage larger than the whole context budget: it is truncated to fit, with its address and the fact of truncation stated, rather than dropped silently or allowed to blow the budget.
- More than five qualifying passages: the best five by rank are injected and the rest omitted, with no partial passage at the tail.
- Several passages of one document all qualify: that document's share is capped, so a single long memory cannot fill every slot.
- A message too short or too referential to be a query ("ok", "thanks", "do that"): no retrieval is attempted, so no passages are injected and no relevance query is paid for.
- A high-traffic channel where the agent observes every message without being addressed: that path's budget is independent of direct conversation and may be zero, so channel volume does not multiply into memory cost.
- An injected passage whose content reads as an instruction: it is delimited and labelled as retrieved reference material, so it is not mistaken for something the user said.

**Staying in step**

- A memory is edited: passages from the previous version stop being retrievable and new ones become retrievable, with no window in which both are returned.
- A memory or a whole bundle is deleted: none of its passages are retrievable afterward.
- A bundle is imported from an archive: its documents become retrievable at passage level like any other, with no separate manual step.
- A memory is edited directly on disk, outside the application: passage-level retrieval for it is allowed to be stale until reconciliation runs, consistent with how direct on-disk edits are already treated.
- The relevance-matching service is unavailable while a memory is being saved: the save itself must still succeed, and the memory must become retrievable once the service returns.
- Very large corpus: the one-time conversion of all existing memories runs unattended on first start and does not block agents from serving requests.

## Requirements *(mandatory)*

### Functional Requirements

**Dividing memories into passages**

- **FR-001**: The system MUST divide every memory concept document into one or more passages ("snippets") whose boundaries follow the document's own meaning and structure — section headings, paragraph breaks, list and block boundaries — rather than cutting at arbitrary offsets.
- **FR-002**: The system MUST keep each passage within a configured target size, and MUST NOT exceed a configured hard maximum size except as permitted by FR-004.
- **FR-003**: The system MUST NOT emit passages below a configured minimum useful size by splitting content that small; content too small to stand alone MUST be merged with its neighbour within the same document.
- **FR-004**: The system MUST keep an indivisible structure (fenced code block, table, or equivalent) inside a single passage where possible; where such a structure alone exceeds the hard maximum, the system MUST split it and mark each resulting passage as a continuation of the previous one.
- **FR-005**: Each passage MUST carry enough identity to be traced back to its source: the owning agent, bundle, document path, and its ordinal position among that document's passages.
- **FR-006**: Each passage MUST be retrievable in the context of its document's title and tags, so that a query matching only a document's metadata still surfaces that document.
- **FR-007**: The system MUST record, for each passage, both its line span and its character span within its document, so a passage can be located in the full document text.

**Searching memory**

- **FR-008**: Memory search MUST return passages, never whole document bodies; retrieving a whole document is the job of a read, chosen after seeing which passages matched.
- **FR-009**: Each search result MUST carry everything needed to act on it without searching again: source bundle, document path, document title, the passage's ordinal position, its line span, and its relevance.
- **FR-010**: Search MUST rank results by how well the individual passage matches the query, independently of which document the passage belongs to.
- **FR-011**: When two or more passages returned for one query are adjacent within the same document, the system MUST merge them into a single continuous result rather than returning overlapping or duplicate text.
- **FR-012**: The system MUST cap how many passages a single document may contribute to one search, so that one long document cannot displace all other memories from the results.
- **FR-013**: The system MUST expose configurable limits for the number of results returned and the relevance threshold below which a passage is not returned.
- **FR-014**: Search MUST return an empty result when no passage meets the relevance threshold, rather than returning the least-bad matches.
- **FR-015**: Search MUST continue to support both searching one named bundle and searching across all of an agent's bundles at once.
- **FR-016**: The system MUST record a read against the source document when that document or one of its passages is returned, preserving the existing notion of how often a memory gets used.
- **FR-017**: The HTTP memory-search endpoint MUST return passage results carrying the same addresses as the agent-facing search (FR-009), rather than whole-document records. No user-interface work is in scope: nothing in the shipped web interface calls that endpoint today, so this requirement is about the response being correct and useful for whoever builds that screen later, not about building it.

**Reading a memory**

- **FR-018**: Callers MUST be able to read a memory in full by its address (bundle and document path) as carried by a search result, with no second search required.
- **FR-019**: A read of a memory that no longer exists MUST produce an explicit "no longer exists" outcome rather than an empty result or an opaque error.

**Automatic related-memory context**

- **FR-020**: The related memories surfaced automatically to an agent each turn MUST carry the matching passage's text, not titles and addresses alone.
- **FR-021**: Each automatically surfaced passage MUST carry the same address information as a search result (FR-009), so the agent can read its document without searching again.
- **FR-022**: Automatic context MUST inject at most five passages per turn by default, down from today's ten documents, and MUST also be bounded by a configurable cap on their total size. Both the count and the size cap MUST be configurable.
- **FR-023**: Automatic context MUST have its own configurable relevance threshold, independent of the search threshold, and the default MUST be stricter than the search default.
- **FR-024**: The system MUST cap how many passages a single document may contribute to one turn's automatic context, so that one long memory cannot fill every slot.
- **FR-025**: Automatic context MUST remain outside the portion of the prompt that is cacheable across requests, so that varying it per turn does not invalidate the cached prefix.
- **FR-026**: When nothing meets the automatic-context threshold, the system MUST omit the related-memory section entirely rather than including weakly-related filler.
- **FR-027**: A failure or timeout while assembling automatic context MUST NOT fail the agent's turn; the turn MUST proceed without that context.
- **FR-028**: Where a single qualifying passage exceeds the whole automatic-context size budget, the system MUST include it truncated, stating its address and that it was truncated, rather than dropping it silently.
- **FR-029**: The system MUST NOT retrieve automatic context when the triggering message is not a usable query on its own — too short, or purely referential ("ok", "do that one", "thanks") — because an embedding of such a message retrieves content unrelated to it, and substantive passages presented as relevant are worse than no passages at all.
- **FR-030**: Automatic context MUST be budgeted separately for each kind of request that triggers it, so that a path which fires on every observed message can be held to a smaller budget, or disabled entirely, independently of direct conversation.
- **FR-031**: Automatic context MUST be presented to the agent as clearly delimited reference material that may be irrelevant, and MUST NOT be presented as instruction or as part of what the user said. Memory content can originate from third-party messages the agent chose to remember, so injecting it verbatim into the agent's turn requires that its provenance and status as data be explicit.
- **FR-032**: The system MUST record, for each turn where automatic context was injected, whether the agent subsequently searched memory anyway, so that the threshold and budget can be tuned against observed hit and miss rates rather than estimates.

**Keeping passages in step with documents**

- **FR-033**: Saving a memory MUST replace all of that document's passages with passages derived from the new content, such that no search can return a passage from the superseded version.
- **FR-034**: Deleting a memory, or deleting a bundle, MUST remove all associated passages from retrieval.
- **FR-035**: Importing a bundle MUST produce passages for every document it contains, with no additional manual step.
- **FR-036**: A failure to build or store passages MUST NOT cause the underlying memory save to fail or be lost; the document MUST remain readable, and MUST become retrievable at passage level once the failure clears.
- **FR-037**: The system MUST be able to detect that a document's stored passages no longer match its content, and rebuild them.

**Transition (no backward compatibility required)**

- **FR-038**: On first start after the upgrade, the system MUST convert every existing memory document to passage-level retrieval, without any operator action.
- **FR-039**: Whole-document *relevance data* MUST be retired: passages become the only thing the system matches queries against, and per-document relevance records MUST NOT be maintained in parallel. Whole-document *results* remain available only through a read, which is addressed by path and needs no relevance data of its own.
- **FR-040**: The conversion in FR-038 MUST NOT prevent agents from serving requests while it runs, and MUST resume rather than restart if interrupted.

**Observability and configuration**

- **FR-041**: Passage sizing limits (target, minimum, maximum), search limits and threshold, and the automatic-context count, size cap and threshold MUST all be configurable, with documented defaults that work without any configuration.
- **FR-042**: The system MUST log the outcome of passage building and of the one-time conversion — counts processed, counts failed, and the reasons for failures.

### Key Entities

- **Memory Concept Document**: Unchanged as the unit a person or agent authors, edits, links, versions, and reads — a titled, tagged markdown document addressed by agent, bundle, and path. It remains the unit of authorship and of truth.
- **Memory Snippet (Passage)**: A contiguous span of one document's content, bounded by the document's own structure, sized to be independently meaningful. Derived, never authored: it holds no content that is not in its document, and it can always be rebuilt from the document. Knows its document, its ordinal position among that document's passages, its line and character spans, and whether it continues a split structure.
- **Passage Index**: The derived, rebuildable collection of all passages for an agent, and the only thing queries are matched against. Replaced wholesale for a document whenever that document is written, and reconcilable against the documents at any time.
- **Search Result**: What a caller receives for a query — matched passage text plus its full address (bundle, path, title, passage position, line span) and relevance. Sufficient on its own to read the whole document.
- **Automatic Context Block**: The related-memory passages assembled for one agent turn without the agent asking: a rank-ordered set of at most five search results, bounded by a size cap, placed outside the cacheable prompt prefix and never persisted to session history.
- **Bundle**: Unchanged as the organizational container for documents, and still the optional scope of a search.

## Success Criteria *(mandatory)*

### Measurable Outcomes

- **SC-001**: For a benchmark set of at least 20 representative searches over a corpus containing long documents, the total text returned per search drops by at least 70% compared to the current whole-document behaviour, while the passage answering each query is still returned.
- **SC-002**: For facts deliberately placed in the final third of long documents, at least 90% of narrowly-worded queries return the passage containing that fact among the top five results — measured against a baseline where the same queries frequently return nothing useful.
- **SC-003**: Search feels as immediate as before: 95% of searches over a corpus of 1,000 documents complete within the time the current whole-document search takes for the same corpus, plus no more than 20%.
- **SC-004**: 100% of returned results can be traced to their source document and, using only the information in the result, used to read that document in full.
- **SC-005**: After a memory is edited, the next search reflects the new content and can no longer return any text removed by that edit — verified with 0 stale results across a test set of edits, deletions, and bundle deletions.
- **SC-006**: The one-time conversion of an existing 1,000-document corpus completes unattended within 10 minutes, with agents answering requests throughout, and reports per-document success or failure.
- **SC-007**: Documents short enough to be a single passage are returned in full, and no memory becomes unretrievable as a result of this change — 0 documents in the corpus end up with no passages.
- **SC-008**: An agent working with the same task and the same memory corpus spends measurably less of its working context on memory text pulled by its own searches: at least a 70% reduction per turn, with no regression in whether it finds what it needed.
- **SC-009**: Automatic context earns its cost: for a benchmark set of at least 20 messages whose answer is stored in memory, at least 60% are answered correctly with no memory tool call at all, versus a baseline of approximately 0% where only titles are surfaced.
- **SC-010**: Automatic context stays selective: replayed against a sample of at least 200 real stored messages, it injects nothing on at least 70% of them, and on messages that are purely referential or under a few words it injects nothing in 100% of cases.
- **SC-011**: The high-volume observation path is contained: for an agent observing a channel it is not addressed in, memory text injected per observed message is zero under the default configuration.
- **SC-012**: Automatic context stays bounded: across all benchmark turns it never exceeds five passages or its configured size cap, and its 95th-percentile size is no more than one third of what injecting five whole documents would cost.
- **SC-013**: Prompt caching is preserved: across consecutive turns in one session, the cacheable prompt prefix is byte-identical despite differing automatic-context content, verified for 100% of sampled turn pairs.

## Assumptions

- **Chunking is mechanical, not model-driven.** Passage boundaries are derived from document structure and size bounds. Using a language model to decide boundaries is out of scope; it would add cost and nondeterminism to every memory save.
- **The relevance-matching capability already in place is reused** at passage granularity rather than replaced. No new external service is assumed.
- **Thresholds must be re-derived, not carried over.** Passage-level similarity scores do not distribute like whole-document scores, so the existing numbers (0.1 for the search tool, 0.5 for automatic context) are starting points to re-tune, not values to preserve.
- **Five is a budget decision, and the threshold still does the harder work.** Ten titles cost on the order of a hundred tokens; five passages of a few hundred words each cost one to two thousand. So this path gets materially more expensive per firing even at half the count, and it only nets out by displacing the follow-up search it currently forces — a search that costs a full model round-trip plus its own returned passages. Because FR-026 omits the section entirely when nothing qualifies, *how often it fires* is what decides whether the feature saves or spends: firing on most turns is a regression at any count, firing on a minority with good precision is a clear win. Five caps the damage when precision is poor; the threshold is what makes precision good.
- **The automatic-context threshold is derivable rather than guessable.** The existing 0.5 was not derived from anything. Stored session history and the existing index make the score distribution measurable before any code is written, by replaying real prompts and plotting what would have been injected; the plan is expected to set the default from that rather than from estimation.
- **Using the message alone as the retrieval query is kept, with a gate.** Retrieving against a wider window of recent conversation would likely match better than any threshold tuning, because "do that one" only becomes answerable when the previous turns are part of the query — but changing what the query *is* goes beyond this feature. FR-029 instead declines to retrieve when the message alone is not a usable query. Widening the query window is a candidate for its own feature later.
- **Automatic context continues to live outside the cacheable prefix and outside session history**, as it does today, so growing it neither invalidates the cache nor accumulates across a conversation.
- **Search returns passages only; reads return documents.** An earlier draft gave search an opt-in whole-document mode. It was dropped because it only ever saved one round-trip, and saved it by making the agent commit to whole documents *before* seeing which passages matched. Searching, then reading the one or two documents that turned out to matter, is both cheaper and better-judged. Because the agent loop already executes every tool call in one assistant response before returning to the model, several reads in one turn cost one model round-trip anyway.
- **Three capabilities, named to avoid confusion.** Search, read, and automatic context. Today the tool named `memory_read` is in fact the *search* tool and `memory_detail` is the document reader, which inverts both names. Two tools change name and swap roles; the other six are untouched:

  | Today | What it does | Becomes |
  | --- | --- | --- |
  | `memory_read` | semantic search | `memory_search` |
  | `memory_detail` | read one document by path | `memory_read` |
  | `memory_list`, `memory_write`, `memory_follow`, `memory_graph`, `memory_delete`, `memory_delete_bundle` | unchanged | unchanged |

  `memory_detail` is retired rather than kept as an alias, since an alias reintroduces the "two tools, one job" ambiguity the rename exists to remove. The `memory_*` prefix is kept over `search_memory` / `read_memory` for two reasons: tool definitions are sorted by name for a stable cache prefix, so the shared prefix groups the family in the list the model sees — whereas `read_memory` would sort adjacent to the unrelated `read_image`, and `search_memory` would separate from `memory_write` and `memory_list` entirely. And `memory_read(path)` matches the file-read idiom models already hold a strong prior about, which `memory_get` or `memory_open` would not. The cost of reusing `memory_read` for a new meaning is accepted because it fails loudly: a stale `memory_read{query}` call hits a schema requiring `path`, fails validation, and the agent corrects within the turn.
- **Partial reads are deferred, not rejected.** Reading part of a memory — by passage range, line range, or heading — was specified and then cut from this feature's scope. The consequence is accepted deliberately: an agent that finds a passage insufficient has one escape hatch only, reading the whole document, so that fallback costs full document size every time. It is tolerable because good chunking makes most passages self-sufficient and automatic context now answers the common case without any read at all, and because partial reads are purely additive — adding them later changes no stored data, no response shape callers depend on, and no part of this feature's migration. If agents are observed habitually reading whole documents after a search, that is the signal to pick this back up.
- **Documents stay the unit of authorship, versioning, and linking.** Passages are a derived read-path artifact: version history, links between concepts, the knowledge graph, and the human-readable markdown files on disk all continue to operate on whole documents. Nothing in this feature versions a passage.
- **No backward compatibility is required**, per the request. Existing whole-document relevance data may be discarded and rebuilt, and the response shapes of the agent-facing tools and the HTTP memory API may change in breaking ways. This does not mean whole-document results disappear — reading a memory by its address still returns the complete document. What is dropped is search ever returning one, and the parallel per-document relevance data behind that.
- **The one-time conversion is a rebuild, not a migration.** Passages are recomputed from the documents on disk, which are the source of truth, so no old index data needs translating.
- **Default sizing targets a passage of a few hundred words** — large enough to stand alone when read without its document, small enough that five of them cost a fraction of five documents. Exact defaults are a planning decision, constrained by FR-002 and FR-041.
- **Direct on-disk edits remain unversioned and may be stale** until reconciliation runs, matching how the existing system already treats edits made outside the application.
- **Agent-facing tool descriptions will be rewritten** to say that search returns passages, and that reading a document in full is the fallback for when a passage is not enough; agents will otherwise keep asking for whole documents out of habit.
- **No memory-search user interface is in scope.** An earlier draft carried a story about showing matched passages to a person browsing memory. It was cut on review: the web interface has no memory search screen at all — `webui/app/services/vizier.tsx` defines a `queryMemories` client function that nothing calls — so that story was proposing to build a screen rather than adapt one. What remains is FR-017, which keeps the endpoint's response correct. The case for eventually showing passages there is debugging: seeing the passage an agent actually retrieved, and spotting passages cut in the wrong place. That is a reason for a future screen to show passages, not a reason to build the screen now.
- **Passage-level retrieval applies to agent memory concept documents only** — not to CORE identity documents, session files, dream journals, or conversation history.
