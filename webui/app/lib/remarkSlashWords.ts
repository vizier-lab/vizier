// A person's `/word` (a command, a skill, or a name nothing answers to) is drawn in the live
// colour wherever it stands in their message. Nothing here knows which commands or skills
// exist: the highlight is about what was typed, not what it did.
//
// A tree walk like `remarkBreaks`: only `text` nodes are touched, so inline code, code blocks,
// links' targets and raw HTML keep their slashes as they are.

/** The parts of an mdast node this plugin reads and writes. */
type Node = { type: string; value?: string; children?: Node[]; data?: Record<string, unknown> }

/** A slash word: at the start or after a space or `(`, and ending before a space, punctuation or the end, so a path (`src/app`, `/usr/bin`) is left alone. */
export const SLASH_WORD = /(^|[\s(])(\/[A-Za-z][\w-]*)(?=$|[\s.,;:!?)])/g

export const SLASH_WORD_CLASS = 'text-live-text'

/** The text cut into plain runs and slash words. */
export function splitSlashWords(text: string): { text: string; slash: boolean }[] {
    const out: { text: string; slash: boolean }[] = []
    let last = 0
    for (const m of text.matchAll(SLASH_WORD)) {
        const start = (m.index ?? 0) + m[1].length
        if (start > last) out.push({ text: text.slice(last, start), slash: false })
        out.push({ text: m[2], slash: true })
        last = start + m[2].length
    }
    if (last < text.length) out.push({ text: text.slice(last), slash: false })
    return out
}

function walk(node: Node): void {
    if (!node.children) return
    const out: Node[] = []
    for (const child of node.children) {
        if (child.type !== 'text' || typeof child.value !== 'string' || !child.value.includes('/')) {
            walk(child)
            out.push(child)
            continue
        }
        for (const run of splitSlashWords(child.value)) {
            out.push(
                run.slash
                    ? {
                          type: 'slashWord',
                          children: [{ type: 'text', value: run.text }],
                          data: { hName: 'span', hProperties: { className: [SLASH_WORD_CLASS] } },
                      }
                    : { type: 'text', value: run.text }
            )
        }
    }
    node.children = out
}

/** A remark plugin: every `/word` becomes a span in the live colour. */
export function remarkSlashWords() {
    return (tree: Node) => {
        walk(tree)
    }
}
