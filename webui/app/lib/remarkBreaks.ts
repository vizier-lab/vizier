// CommonMark calls a single line ending inside a paragraph a *soft break*: the parser keeps it as
// a "\n" inside the text node, and HTML collapses that to a space, so `one\ntwo` draws on one
// line. A reply typed into a chat reads as lines, so the WebUI turns that "\n" into a `<br>`
// instead, everywhere and always: one rule for a reply, a document and interface text alike.
//
// The `remark-breaks` package does the same job through a micromark extension; this is the tree
// walk it would take, kept here so the WebUI needs no dependency for it. Only `text` nodes are
// touched: a code block, inline code and raw HTML hold their newlines in `value` on nodes of
// another type, and pass through untouched.

/** The parts of an mdast node this plugin reads and writes. */
type Node = { type: string; value?: string; children?: Node[] }

/** Split every text node's newlines into `break` nodes, which draw as `<br>`. */
function splitBreaks(node: Node): void {
    if (!node.children) return
    const out: Node[] = []
    for (const child of node.children) {
        if (child.type !== 'text' || typeof child.value !== 'string' || !child.value.includes('\n')) {
            splitBreaks(child)
            out.push(child)
            continue
        }
        child.value.split('\n').forEach((line, i) => {
            if (i > 0) out.push({ type: 'break' })
            if (line !== '') out.push({ type: 'text', value: line })
        })
    }
    node.children = out
}

/** A remark plugin: every soft break becomes a hard one. */
export function remarkBreaks() {
    return (tree: Node) => {
        splitBreaks(tree)
    }
}
