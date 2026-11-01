# Quickstart: Unify Core and Memory

Every check below is runnable. The constitution's e2e gate applies to this feature — identity is agent-observable — so the agent-facing steps are **dummyplug** steps: deterministic, offline, no credentials.

## 0. Prepare

```powershell
just checkout 043-unify-core-and-memory   # workspace + design + webui (constitution gate)
cargo test -p vizier-core --lib           # baseline: expect the 2 known Windows-only failures
cd webui; npm run typecheck; npm test; cd ..
cd design; npx tsc --noEmit; cd ..
```

## 1. Migration of an existing agent (manual, real binary)

The seed is a startup path, so it is verified by running the binary — not by a unit test alone.

1. Copy a workspace that has agents with `agent_core` populated, **including one whose CORE is over 4 KiB** (the over-ceiling path is the one that can silently fail).
2. Note each agent's CORE text: `GET /api/v1/agents/{id}/core` → `content`, saved to a file.
3. `just run`, let every agent start.
4. For each agent, `GET /api/v1/agents/{id}/memory/doc/core/soul`:
   - **Expect** `200` with the CORE text from step 2, unchanged and complete at the front of the body.
   - **Expect** a provenance line appended at the end of the body.
   - **Expect** the over-ceiling agent's document to be present in full — the upgrade must not be blocked by the limit (FR-025).
5. Restart the binary. `GET .../memory/doc/core/soul` returns the same body: **byte-identical**, no second provenance line (idempotent).
6. `GET /api/v1/agents/{id}/memory/bundles` → `core` appears with `protected: true`.

**Fail conditions**: any truncation, any refusal logged for the over-ceiling agent, a second provenance line after restart, or content differing from step 2.

## 2. The identity is injected and self-describing (dummyplug)

With an agent on the dummyplug provider, send any chat message. Inspect the request the model receives (dummyplug echoes the prompt shape).

- **Expect** a `<memory bundle="core" path="soul">` block containing the document body.
- **Expect** exactly one such block — nothing else is injected into the system prompt (FR-006).
- Send a second message; **expect** the block to be byte-identical to the first (FR-007: cacheable prefix).

## 3. The agent knows it is a memory (dummyplug)

Dummyplug's tool protocol: send a message whose reply is a tool call to `memory_read` with `{"bundle":"core","path":"soul"}`.

- **Expect** the document to come back, title and content intact.
- **Expect** the agent's category list to contain **no** `identity` category and **no** `read_core`/`write_core` tool (contract `tool-surface.md`).
- Send a `memory_write` to `{"bundle":"core","path":"soul"}` with changed content; **expect** the next turn's injected block to show the change.

## 4. Soul is removable, core bundle is not (HTTP)

| step | expect |
|---|---|
| `DELETE /memory/doc/core/soul` | `200` |
| next turn (dummyplug) | injected block carries the **default** template text plus a note that it can be written at this address — never an empty block (FR-008) |
| `DELETE /memory/bundles/core` | `409`, message says the bundle is reserved |
| `POST /memory` with `{"bundle":"core","path":"values"}` | `422`, message names `soul` as the only accepted path |
| `POST /memory/bundles/import` targeting `core` | `409` |
| re-write `core/soul` | `200`; next turn injects it again |

## 5. Size limits (dummyplug + HTTP)

Write memories of increasing size into a scratch bundle.

| content size | expect |
|---|---|
| 2 KiB | succeeds, output is the plain string, **no warning field** |
| 6 KiB | succeeds, output carries `warning` as a **separate field**, message unchanged |
| 11 KiB | `422`, message starts `memory too large:`, names the overage, the ceiling, and gives a split pattern with a sibling path and link syntax |
| 11 KiB, then the same memory shortened to 3 KiB | succeeds (a shorter write to an over-ceiling document is accepted) |
| write the identity document past 4 KiB | refused with the identity ceiling quoted, not the ordinary one |
| write exactly at the ceiling | accepted |

**Fail conditions**: any truncated document, a warning embedded in the message text, a refusal that leaves the memory modified, or any refusal for an unchanged/legacy document being read.

## 6. Reviewing identity through Memory (WebUI, manual)

1. Open the agent's **Memory**. **Expect** `core` in the tree with a reserved marker and its delete action disabled.
2. Open `core/soul`. **Expect** the document and its history.
3. Compare two revisions, restore one. **Expect** the next turn's injected block to be the restored text.
4. Open a stale editor tab, save. **Expect** a refusal showing the newer content, not an overwrite.
5. Navigate the agent's menu. **Expect** no Core entry, and `/…/core` not to resolve to a page.

## 7. Dream journal (dummyplug)

1. Run a dream cycle whose consolidation writes to `core/soul` **and** to an ordinary bundle.
2. `GET` the cycle/night.

- **Expect** an identity change reported for the soul write, with its `bundle`/`path` pointing at it.
- **Expect** the identity change **not** counted in written/revised/removed (FR-032).
- **Expect** `core_changes` to equal the number of identity writes.
3. In the WebUI dream view, **expect** the night strip's ring and the summary to read identity changes; clicking one opens the document in Memory.

## 8. Deprecated API stays honest

1. `PUT /core` with new content → `200`.
2. `GET /memory/doc/core/soul` → the same content (one document, not a copy) — SC-013.
3. Reverse it: write through Memory, read through `GET /core` → same content.
4. `/swagger` → all four `/core` operations marked deprecated, each naming the memory endpoint that replaces it.
