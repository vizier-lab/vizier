# Contract: WebUI

## Types — `webui/app/interfaces/types.ts`

```ts
export type RevisionActor = { type: 'agent' } | { type: 'user'; user_id: string; username: string } | { type: 'system' }
export type RevisionTrigger =
  | { type: 'conversation' } | { type: 'dream' } | { type: 'webui' } | { type: 'api' }
  | { type: 'import' } | { type: 'rollback'; restored_from: number } | { type: 'baseline' }
export interface CoreRevisionSummary { seq: number; is_current: boolean; size_bytes: number;
  actor: RevisionActor; trigger: RevisionTrigger; created_at: string }
export interface CoreRevision extends CoreRevisionSummary { content: string }
export interface PaginatedCoreRevisions { revisions: CoreRevisionSummary[]; total: number; offset: number; limit: number }
export interface MemoryRevisionSummary extends CoreRevisionSummary { deleted: boolean }
export interface MemoryRevision extends MemoryRevisionSummary { content: string | null; title: string | null; tags: string[] }
export interface PaginatedMemoryRevisions { revisions: MemoryRevisionSummary[]; total: number; offset: number; limit: number }
export interface DiffLine { op: 'equal' | 'insert' | 'delete'; old_line: number | null; new_line: number | null; text: string }
export interface DiffHunk { old_start: number; old_lines: number; new_start: number; new_lines: number; lines: DiffLine[] }
export interface RevisionDiff { from_seq: number; to_seq: number; additions: number; deletions: number; hunks: DiffHunk[] }
export interface RollbackResponse { no_change: boolean; new_seq: number | null; restored_from: number }
```

## Service functions — `webui/app/services/vizier.tsx`

```ts
getCoreHistory(agentId, offset?, limit?)            // GET  /agents/{id}/core/history?offset&limit
getCoreRevision(agentId, seq)                       // GET  /agents/{id}/core/history?seq=
diffCoreRevisions(agentId, to, from?)               // GET  /agents/{id}/core/history?to=&from=
rollbackCore(agentId, seq)                          // POST /agents/{id}/core/history {seq}

getMemoryHistory(agentId, bundle, path, offset?, limit?)   // GET  /agents/{id}/memory/history/{bundle}/{path}?offset&limit
getMemoryRevision(agentId, bundle, path, seq)              // GET  ...?seq=
diffMemoryRevisions(agentId, bundle, path, to, from?)      // GET  ...?to=&from=
rollbackMemory(agentId, bundle, path, seq)                 // POST ... {seq}
```
Memory paths are encoded with the existing `encodeMemoryPath` helper (same as `getMemory`).

## Component — `webui/app/components/VersionHistory.tsx`

The panel itself is presentational — list rows, a content viewer, a diff viewer, a confirm dialog — and is the same UI for both kinds, so it is one component fed by a small normalized row shape. Each route maps its own API types into that shape (CORE rows get `deleted: false`, no title/tags):

```ts
export interface HistoryRow { seq: number; is_current: boolean; deleted: boolean; size_bytes: number;
  actor: RevisionActor; trigger: RevisionTrigger; created_at: string }
export interface HistoryVersion extends HistoryRow { content: string | null; title?: string | null; tags?: string[] }

interface VersionHistoryProps {
  source: {
    list: (offset: number, limit: number) => Promise<{ rows: HistoryRow[]; total: number }>
    get: (seq: number) => Promise<HistoryVersion>
    diff: (to: number, from?: number) => Promise<RevisionDiff>
    rollback: (seq: number) => Promise<RollbackResponse>
  }
  label: string                    // "CORE" or the memory title — used in copy only
  onRolledBack?: (res: RollbackResponse) => void
}
```

`routes/agent-core.tsx` builds `source` from the `*Core*` service functions; `routes/memory.tsx` from the `*Memory*` ones. If the two panels ever need to diverge (e.g. memory-only features), split the component then — not pre-emptively.

**Behavior (maps to spec US2/US3):**
- On mount: `list(0, 50)`; renders newest-first rows: `v{seq}` · actor label (`Agent` / `@username` / `System`) · trigger label (`conversation` / `dream cycle` / `WebUI` / `API` / `import` / `restored from v{n}` / `baseline`) · relative + absolute time · `Current` badge on `is_current` · `Deleted` badge on `deleted`. "Load more" appends the next page while `offset + limit < total` (FR-017).
- Click a row ⇒ `get(seq)` ⇒ read-only content pane (memory: title + tags header above the body). Deletion entries show "This version records a deletion" and no content.
- "Changes" button on a row (disabled on seq 1 and on deletion entries) ⇒ `diff(seq)` ⇒ diff pane vs previous (FR-008).
- "Compare" toggle ⇒ pick two rows ⇒ `diff(max, min)` (FR-009). Order is normalized so the newer version is `to`.
- "Restore this version" on a non-current, non-deleted row ⇒ confirm dialog showing `v{seq}` + timestamp + actor ("This will save v{seq}'s content as a new version v{next}. Nothing is deleted.") ⇒ `rollback(seq)` ⇒ toast (`Restored v{seq} as v{new_seq}` or `Already identical to current — nothing changed`) ⇒ refresh list ⇒ `onRolledBack(res)` (FR-010/011/015).
- Diff rendering: mono font, line numbers, `insert` rows tinted with a green-ish background and `delete` with red-ish, both defined as CSS variables that already respect the theme toggle (add `--diff-insert-bg`/`--diff-delete-bg` to `app.css` light + dark blocks). Hunks separated by a `@@ -a,b +c,d @@` header row. Container `overflow-x: auto`.
- Errors surface via the existing `useToastStore`.

## Route wiring

- `routes/agent-core.tsx`: add a "History" ghost button in `.main-header` (always visible, independent of `hasChanges`). Opens the existing `SlideOver` with `<VersionHistory kind="core" source={…core services bound to agentId…} onRolledBack={reload} />`. `reload` re-fetches CORE and resets `content`/`original`. If the editor has unsaved changes when a restore is confirmed, the confirm dialog additionally warns they will be discarded.
- `routes/memory.tsx`: in the `modalMode === 'view'` slide-over body, add a "History" button next to Edit/Delete. It toggles a local `showHistory` state which swaps the body to `<VersionHistory kind="memory" source={…bound to agentId, selectedMemory.bundle, selectedMemory.path…} onRolledBack={…} />`. `onRolledBack` re-fetches the memory detail (`getMemory`) and refreshes the current list/graph (existing `loadMemories`); if the memory had been deleted and is now restored, it is reloaded into `selectedMemory`.
- No new route; no change to `routes.ts`.

## Typecheck

`cd webui && npm run typecheck` must pass; no new npm dependencies.
