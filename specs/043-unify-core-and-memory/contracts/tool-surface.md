# Contract: agent tool surface

## Removed

| tool | replacement |
|---|---|
| `read_core` | `memory_read(bundle="core", path="soul")` |
| `write_core` | `memory_write(bundle="core", path="soul", title=…, content=…)` |

The `identity` tool category is removed from `ToolCategory` entirely (name, description, registration and the `ALL` array). An agent that still asks for `category="identity"` gets the ordinary "no such category" path.

**Dangling references that MUST be updated in the same change** — none of these are tool definitions, so deleting the tools does not break them and they would silently rot:

| site | today | after |
|---|---|---|
| `agents/tools/mod.rs` dream allow-list | `read_core`, `write_core` | the memory tools only (already listed) |
| `agents/tools/mod.rs:2141` | `&["read_core"]` | the memory read tool |
| `crates/vizier-core/src/sandbox/docs.rs:367` | `"read_core"` in the docs allow-list | `"memory_read"` (already present) |
| `utils/mod.rs` progress labels | `"write_core" => "🧠 Updating core"` | removed with the tool |

## `memory_write` — result contract

**Input**: unchanged, plus the `core` bundle is now a legal `bundle` value with exactly one legal `path`.

```jsonc
// under the soft limit — unchanged from today
"memory 'vizier/workqueue' written"

// over the soft limit, still below the ceiling — the write succeeded
{
  "message": "memory 'vizier/workqueue' written",
  "warning": "6144 bytes — over the 4096-byte comfortable size. One memory, one topic: consider splitting it and linking the parts."
}
```

**Rules**

1. The warning is a **separate field**, never concatenated into `message`, so an agent cannot read the success as a failure (FR-018, research Decision 4).
2. Below the soft limit the output is the plain string it is today — no shape change on the common path.
3. The warning is advisory only. Nothing is truncated, truncated writes do not exist (FR-017).

## `memory_write` — refusal contract

```
memory too large: content is 11240 bytes, 3048 over the 8192-byte limit.
One memory, one topic. Split it, then link the parts:
  - write the second subject to 'vizier/workqueue-landed' in the same bundle
  - replace the moved text in this memory with [landed](workqueue-landed.md)
Same-bundle links are ordinary markdown links; across bundles use [[bundle/slug]].
```

**Rules**

4. The first token is the stable marker `memory too large:` — the service maps it to **422** (research Decision 5).
5. The text MUST contain: actual size, ceiling, overage, and a concrete split pattern (FR-018). It MUST NOT prescribe the seam — that is the agent's judgement (spec assumption).
6. The tool returns this as a `VizierError`, so it surfaces as a Python `RuntimeError` inside a script, which is how every other tool failure already reaches the agent.

## `memory_list` — reserved bundle

With no bundle named, the bundle list includes `core` and marks it reserved/protected so a client can disable destructive actions:

```jsonc
{ "name": "core", "concept_count": 1, "protected": true, "updated_at": "…" }
```

## Doctrine additions (`boot.rs`)

The "Where things go" section loses its separate CORE line and gains identity as a memory, naming the address and the operations — this is the discovery fix for the tool that used to be called `read_core` (FR-010):

> **Core** — `core/soul` in your memory: who you are and how your people want you to work. It is a memory like any other; that is why it is the only one injected every turn. Read and write it with the memory tools.

No size figure appears in the doctrine (FR-024).
