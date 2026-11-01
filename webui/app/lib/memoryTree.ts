import type { FileTreeNode } from '@vizier/design'
import type { Bundle, MemorySummary } from '../api/endpoints/memory'
import { count } from './format'

/** A document's own page: `/agent/memory/bundle/dir/doc`, with no `.md`. */
export const docHref = (agentId: string, bundle: string, path: string) =>
    `/${agentId}/memory/${encodeURIComponent(bundle)}/${path.replace(/\.md$/, '').split('/').map(encodeURIComponent).join('/')}`

/** A document opened from a conversation carries it as `?from=`, so going back returns there. */
export const docFromChat = (agentId: string, bundle: string, path: string, chat: string) =>
    `${docHref(agentId, bundle, path)}?from=${encodeURIComponent(chat)}`

/** The conversation a document was opened from, if `from` names one of this agent's. */
export const chatFrom = (agentId: string, from: string | null) => (from?.startsWith(`/${agentId}/chat/`) ? from : null)

export const bundleHref = (agentId: string, bundle: string) => `/${agentId}/memory/${encodeURIComponent(bundle)}`

/** Tree ids: a bundle is its name, a folder ends in `/`, a document is `bundle/path`. */
export const docId = (bundle: string, path: string) => `${bundle}/${path.replace(/\.md$/, '')}`

export type LoadedDocs = { items: MemorySummary[]; total: number }

/** One bundle's documents as folders and files, folders first, each level by name. */
export function bundleChildren(bundle: string, docs: LoadedDocs, moreId: string): FileTreeNode[] {
    type Dir = { dirs: Map<string, Dir>; files: MemorySummary[] }
    const root: Dir = { dirs: new Map(), files: [] }
    for (const d of docs.items) {
        const parts = d.path.replace(/\.md$/, '').split('/')
        let at = root
        for (const p of parts.slice(0, -1)) {
            if (!at.dirs.has(p)) at.dirs.set(p, { dirs: new Map(), files: [] })
            at = at.dirs.get(p)!
        }
        at.files.push(d)
    }
    const build = (dir: Dir, prefix: string): FileTreeNode[] => [
        ...[...dir.dirs.entries()]
            .sort(([a], [b]) => a.localeCompare(b))
            .map(([name, sub]) => ({
                id: `${bundle}/${prefix}${name}/`,
                label: name,
                kind: 'folder' as const,
                mono: true,
                children: build(sub, `${prefix}${name}/`),
            })),
        ...[...dir.files]
            .sort((a, b) => a.path.localeCompare(b.path))
            .map((d) => ({ id: docId(bundle, d.path), label: d.title || d.path.split('/').pop()!, kind: 'file' as const })),
    ]
    const nodes = build(root, '')
    const left = docs.total - docs.items.length
    if (left > 0) nodes.push({ id: moreId, label: `Load ${count(Math.min(left, 200))} more of ${count(left)}`, kind: 'action' })
    return nodes
}

export function bundleNode(b: Bundle, docs: LoadedDocs | undefined, moreId: string): FileTreeNode {
    return {
        id: b.name,
        label: b.name,
        kind: 'folder',
        icon: 'memory',
        mono: true,
        // A reserved bundle is marked here so the person can see why its delete action is
        // disabled, rather than finding out from a refusal after trying (webui contract rule 1).
        meta: b.protected ? `${count(b.concept_count)} · reserved` : count(b.concept_count),
        children: docs ? bundleChildren(b.name, docs, moreId) : undefined,
    }
}
