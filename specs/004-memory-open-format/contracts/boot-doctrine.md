# Contract: `BOOT.md` operating doctrine — `## Memory` section

Location: `src/agents/agent/system_prompt/boot.rs`, the `## Memory` block (currently lines
~32-35).

## Current text

```markdown
## Memory

- **Link** — Use `[[slug]]` syntax to create relationships between memories (e.g., "See [[project-architecture]] for details")
- **Discover** — Use `memory_follow` to traverse links and `memory_graph` to visualize clusters and gaps
```

## Required replacement content (FR-022)

Must teach, at minimum:

1. Memory is organized into named **bundles**; a write that doesn't name one goes to the
   default bundle; naming a new bundle creates it automatically (FR-006/FR-008).
2. A bundle can have nested subdirectories; write a multi-segment path (e.g.
   `friends/bred`) to file a memory under one (FR-007).
3. Link syntax now has two forms:
   - Same-bundle: `[label](path/to/concept.md)` — an ordinary markdown link.
   - Cross-bundle: `[[bundle/slug]]` for one concept in another of the agent's bundles, or
     bare `[[bundle]]` to reference that bundle as a whole (FR-004, FR-013).
4. How to actually explore, as a concrete recipe (`contracts/memory-tools.md`'s Exploration
   recipe) rather than just "these tools exist" — **browsing and searching are different, each
   with its own default**:
   - `memory_list`/`memory_graph` with no bundle → see your bundles (a summary list, or the
     bundle-level graph with cross-bundle links as edges).
   - `memory_list`/`memory_graph` naming a bundle → focus on everything inside it (nesting is
     flattened; a path like `friends/bred` just shows up as one entry).
   - `memory_follow` on a known concept → jump along its links; `memory_detail` → open one
     directly once you know where it lives.
   - `memory_read` with a query, no bundle → search across *every* bundle at once (this is its
     default because relevance, not bundle organization, is what search is for); name a bundle
     only to narrow it once you already suspect where the answer is.

This is one of exactly two discovery channels for the new conventions (the other is
`contracts/memory-tools.md`'s tool descriptions) — no separate documentation/onboarding surface
is introduced (spec clarification).

## Suggested replacement text

```markdown
## Memory

- **Organize** — Memory lives in named bundles (e.g., one per project or person); a write with
  no bundle named goes to your default bundle, and naming a new one creates it automatically.
  Nest concepts into subdirectories with a multi-segment path (e.g. `friends/bred`).
- **Link** — Same bundle: an ordinary markdown link, `[label](path/to/concept.md)`. A different
  bundle: `[[bundle/slug]]` for one concept there, or bare `[[bundle]]` for that bundle as a
  whole.
- **Browse** — `memory_list`/`memory_graph` with no bundle show your bundles; naming a bundle
  focuses either one on everything inside it. Use `memory_follow` to jump along a specific link,
  and `memory_detail` to open a concept you already know the location of.
- **Search** — `memory_read` searches across all your bundles by default — that's usually what
  you want. Name a bundle only to narrow the search once you already suspect where the answer
  lives.
```

This is a draft for the implementer to refine against actual tool description wording once
built — the contract's requirement is the four numbered points above, not this exact prose.
