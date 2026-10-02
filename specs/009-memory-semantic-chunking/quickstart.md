# Quickstart: Semantic Chunking for Memory Recall

**Feature**: `009-memory-semantic-chunking` | **Date**: 2026-10-01

Scripted as dummyplug steps per the constitution's end-to-end gate. Each step gives the message to
send and what to expect.

## Setup

Chunking needs an embedder. Agent creation already guarantees one: `CreateAgentRequest::into_config`
forces `embedding: Some(..)` defaulting to local fastembed `all_mini_lml6_v2`, and
`indexer: Some(Sqlite)` (`src/channels/http/api/v1/agents/mod.rs:472-481`). So no extra setup is
needed, and leaving `embedding` out of the create request is the realistic default to test against.

1. `just run` (or `just run-d`).
2. Create an agent via the WebUI or `POST /api/v1/agents` with `provider: "dummyplug"` (no
   credentials) and no `embedding` — the local default is what most agents will actually run.
3. Run with `RUST_LOG=vizier=debug` — Steps 6, 7, 8 and 10 read tracing output, because the
   automatic-context block is deliberately not visible in dummyplug's reply (see Step 6).

> **Offline caveat**: `fastembed` downloads its model once on first use. After that the loop is
> fully offline. This is the only embedder that needs no API key, so it is the one honest choice
> for this harness — but the first run is not network-free, and the constitution's "no credentials,
> no network" claim for dummyplug covers the *provider*, not the embedder.

---

## Step 1 — The renamed tools are the ones present

**Send**: `tools`

**Expect**: the list contains `memory_search` and `memory_read`, and **no** `memory_detail`.
`memory_list`, `memory_write`, `memory_follow`, `memory_graph`, `memory_delete` and
`memory_delete_bundle` are unchanged. (FR-009, contracts/memory-tools.md)

**Send**: `memory_search`

**Expect**: the sample request takes `query` and optional `bundle`, and the description says results
are passages and that `memory_read` fetches the whole document.

---

## Step 2 — Write a long, multi-section memory

**Send**: `memory_write`, then the returned JSON with a document of at least ten headed sections,
roughly 3,000 words, with the string `deployment windows are Tuesday and Thursday mornings` in the
**eighth** section, plus one fenced code block of about 5 KB. Make **one** section ~4,000 bytes
covering two clearly different topics with no sub-heading — that is the section pass 2 must split on
an embedding-chosen seam rather than at a byte offset.

**Expect**: `**Tool result** (`memory_write`)` with the saved memory.

**Then verify** against the data store:

```sh
sqlite3 "$VIZIER_DATA_DIR/vizier.db" \
  "SELECT ordinal, line_start, line_end, continues FROM memory_passage ORDER BY ordinal;"
```

- More than one row (FR-001).
- Rows contiguous: each `char_start` equals the previous `char_end`.
- At least one row with `continues = 1`, from the oversized code block (FR-004).
- Matching row count in the index:
  `SELECT count(*) FROM document_index WHERE path LIKE '%ops/deploys#%';`
- `content_hash` identical on every row of the document (data-model §3).
- The oversized two-topic section split **between** its topics, not at `max_size`: check that the
  boundary line in `memory_passage` falls at the topic change rather than at a round byte count
  (FR-001, research D1b). Compare against the same document chunked with embeddings disabled —
  size-packing cuts at the budget, so a differing boundary is the evidence the seam logic ran.

---

## Step 3 — Search returns the buried passage, not the document

**Send**: `memory_search`, then
`{"tool":"memory_search","arguments":{"query":"deployment windows"}}`

**Expect** in the echoed tool result:
- A passage containing `Tuesday and Thursday mornings` (FR-008, SC-002) — the case that returns
  nothing useful today, because one embedding for a 3,000-word document drowns it out.
- `bundle`, `path`, `title`, `ordinal`, `line_start`, `line_end`, `score` on every result (FR-010).
- Total text far smaller than the document (SC-001). Compare against `memory_read` in Step 5.
- Unrelated sections absent.

---

## Step 4 — Short memories come back whole

**Send**: `memory_write` with a two-sentence memory, then search for its topic.

**Expect**: one result whose text is the entire memory, `ordinal` 0 (FR-003, SC-007). A short memory
must not become unreachable.

---

## Step 5 — Reading the whole document still works

**Send**: `{"tool":"memory_read","arguments":{"path":"ops/deploys","bundle":"default"}}`

**Expect**: the complete document (FR-018). Note its size against Step 3 to evidence SC-001.

**Then send**: `{"tool":"memory_read","arguments":{"path":"does/not/exist"}}`

**Expect**: an explicit "no longer exists" message, not an empty result and not a stack trace
(FR-019).

---

## Step 6 — Automatic context carries five passages

Dummyplug strips the injected context block before interpreting a command (commit 52a8487), and the
block is never written to session history, so **the reply cannot confirm this** — read the log.

**Send**: `what are our deployment windows?` (a normal sentence, not a command — dummyplug replies
with lorem ipsum, which is fine; the assembly is what is under test)

**Expect in the debug log**:
- At most five passages assembled (FR-022).
- Each with bundle, path and ordinal (FR-021).
- Total within the configured size cap (SC-012).
- No single document contributing all five (FR-024).

---

## Step 7 — Trivial messages retrieve nothing

**Send**: `ok`

**Expect in the log**: retrieval skipped, no relevance query issued, no block assembled (FR-029,
SC-010). This is the check that keeps a 15x-costlier payload from firing on turns that cannot use it.

**Send**: `thanks` and `do that one` — same outcome.

---

## Step 8 — The observation path injects nothing

Requires a Discord or Telegram channel configured, since `SilentRead` only originates there
(`src/channels/discord/mod.rs:508`, `src/channels/telegram/mod.rs:497`).

**Do**: post a message in a guild channel the agent is in **without** mentioning it.

**Expect in the log**: a `SilentRead` request with zero memory text injected under default config
(FR-030, SC-011). Channel traffic must not multiply into memory cost.

---

## Step 9 — Edits leave no stale passages

**Send**: `memory_write` to the same `path` as Step 2, with the deployment section deleted and
different content.

**Expect**:
- `memory_passage` rows fully replaced, not appended (FR-033).
- `SELECT count(*) FROM document_index WHERE path LIKE '%ops/deploys#%'` equals the new row count —
  no orphans from the longer previous version.
- Searching `deployment windows` no longer returns the removed text (FR-033, SC-005). **This is the
  one most likely to regress**: a document that shrinks from 12 passages to 4 leaves 8 orphaned
  vectors unless deletion is driven by the stored ordinals.

**Then send**: `{"tool":"memory_delete","arguments":{"path":"ops/deploys"}}`

**Expect**: no `memory_passage` rows and no `document_index` rows for that path (FR-034).

---

## Step 10 — Embedder failure still saves the memory

**Do**: point the agent's embedding config at an unreachable `base_url`, then send `memory_write`
with the same oversized-section document.

**Expect**: the write succeeds, `memory_passage` rows exist from pure size-packing, and a warning is
logged (FR-036, research D1b fallback). A save must never fail because seam selection could not
reach an embedder.

---

## Step 11 — Conversion of a pre-existing corpus

**Do**: against a data directory written by the current release (documents on disk, no
`memory_passage` table), start the new binary.

**Expect**:
- The agent answers requests immediately, while conversion proceeds (FR-040).
- Log lines reporting documents processed and failed, with reasons (FR-042).
- Every document ends with at least one `memory_passage` row (SC-007).
- Kill and restart mid-conversion: it resumes rather than restarting, because documents already
  holding rows are skipped (FR-040).
- Edit one document on disk without going through Vizier, then trigger reconcile: its
  `content_hash` no longer matches and only that document is rebuilt (FR-038, research D12). The
  check that matters here is that *no other* document is rebuilt — a span-diff implementation would
  have rebuilt the whole corpus.

---

## Step 12 — Prompt caching survives

**Send**: two different ordinary sentences in one session, with provider request logging on.

**Expect**: the cacheable prefix byte-identical across both, with only the user message differing
(SC-013). The property holds because `with_context` prepends to the user message
(`src/agents/agent/mod.rs:436`); this step exists so a later refactor cannot quietly move it into a
system message.

---

## Requires a live provider

| Check | Why dummyplug cannot cover it |
|---|---|
| SC-009 — 60% of answerable messages answered with no tool call | Needs a real model deciding whether an injected passage sufficed. Dummyplug never reasons. |
| Tool descriptions actually steer behaviour | Whether an agent reaches for `memory_read` after a thin passage is model judgement. |
| SC-008 — context spend per turn | Needs real token accounting. |

Run these against one cheap live provider and mark the results as provider-dependent.

## Threshold derivation (not a runtime check)

SC-010 needs the automatic-context threshold derived, not guessed (research Decision 10). Replay
stored session history through the passage index and pick the lowest value that yields an empty
block on at least 70% of real messages. `HistoryStorage` already persists the history, so this is a
one-off harness over data on disk — no new capture, and it can be done before the 0.6 provisional
default is committed to.
