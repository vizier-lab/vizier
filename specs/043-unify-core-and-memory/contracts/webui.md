# Contract: WebUI

The product-facing term is **Core** and it does not change (spec assumption 1). What changes is that Core no longer has an address of its own.

## Removed

| file | action |
|---|---|
| `routes/agent-core.tsx` | deleted (page) |
| `routes/agent-core-edit.tsx` | deleted (editor) |
| `routes.ts` | both `route(...)` entries removed |
| `screens/shell/pages.ts` | `core` removed from `PAGE_PATH` and the page union |
| `design/src/components/Shell/ContextSidebar.tsx` | `"core"` removed from `MenuPage`; the **`"The agent"` menu group is removed** because it held only Core |

**Rules**

1. No redirect from `/core`. FR-029 forbids it: a redirect keeps the idea that identity has its own address.
2. A stale bookmark or open tab lands on the app's existing unknown-route handling. Nothing special is added for it.
3. Behavior that lived on the page must be reachable in Memory before the page is deleted — that is the sequence, not an afterthought (see quickstart steps 5–7 for the checks).

## Memory explorer

The explorer already renders bundles as a tree and documents in a viewer with revision history. Two additions:

1. **Reserved bundle, protected.** `listBundles` returns `protected: true` for `core`; the tree shows a reserved badge on it and disables *delete bundle*. Deleting the document inside it stays available — it is an ordinary document.
2. **A document's history works on the identity document**, which it already does because it is a memory document.

**Rule**: the explorer must not special-case the *document* in any way. If a change is only needed because the document is identity, it belongs to the bundle's protection, not the document's.

## Dream surfaces

| component | change |
|---|---|
| `design/.../NightStrip/NightStrip.tsx` | the ring is shown for nights with identity changes; `aria-label` and the hover text say "identity" rather than naming a page |
| `design/.../NightStrip` stories | the fixture set of nights is unchanged in shape |
| `design/.../DreamJournalEntry/DreamJournalEntry.tsx` | the `core` change kind keeps its name internally, renders as an identity change |
| `webui/app/lib/dreams.ts` | `cycleSummary` / `nightHint` report identity changes as their own count and no longer double-count them as memories |
| `webui/app/routes/agent-dream.tsx` | the link on an identity change targets the document in **Memory**, not the deleted page |

**Rules**

4. The count is **excluded** from written/revised/removed (FR-032). The summary reads e.g. `2 written · 1 identity` rather than `3 written`.
5. Clicking an identity change opens the document in the memory explorer at `core/soul` (or whatever `bundle`/`path` the change carried — an agent may add other documents to the bundle, and those are identity changes too).

## Accessibility / behaviour preserved

6. The night strip's `aria-label` must keep describing what the ring means; it currently says how many nights had changes and must now distinguish identity changes.
7. Dialogs and popovers already follow the house dismissal rule and none are added here.
