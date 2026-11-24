import type { DocLink } from '@vizier/design'
import { bundleHref, docHref } from './memoryTree'

/** Where a relation points: `other.md` is in this bundle, `bundle/slug` in another. */
export function relationLink(agentId: string, bundle: string, relation: string): DocLink {
    const clean = relation.replace(/\.md$/, '')
    const [b, ...rest] = clean.includes('/') ? clean.split('/') : [bundle, clean]
    const path = rest.join('/') || clean
    return { label: clean, href: docHref(agentId, b, path), note: b !== bundle ? b : undefined }
}

/** `[[bundle/doc]]` and `[[bundle]]` become links into Memory, outside code. */
export function linkWikiLinks(agentId: string, bundle: string, markdown: string): string {
    return markdown
        .split(/(```[\s\S]*?```|`[^`\n]*`)/g)
        .map((part, i) =>
            i % 2 === 1
                ? part
                : part.replace(/\[\[([^\]\n|]+?)(?:\|([^\]\n]+))?\]\]/g, (_m, target: string, label?: string) => {
                      const t = target.trim()
                      const to = t.includes('/') ? relationLink(agentId, bundle, t).href : bundleHref(agentId, t)
                      return `[${(label ?? t).trim()}](${to})`
                  })
        )
        .join('')
}
