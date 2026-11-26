// Inline math in a reply: `$…$`. The dollars are ambiguous — a price, a shell variable and a
// formula all write them — so a pair of dollars is read as math only when it is written like
// math. Otherwise it is the prose it was: "the price is $5 and $10" is a sentence, not a
// formula.
//
// The rules are GitHub's, from the commonmark-hs math extension: the opening `$` must not be
// followed by whitespace, the closing `$` must not be preceded by whitespace, and the closing
// `$` must not be followed by a digit. So "let $x$ and $y$ be integers" is math, and "use
// $HOME and $PATH" is a sentence.
//
// `remark-math` alone pairs any two dollars, which is how a price became the formula "5 and".
// This plugin takes such a pair back: it is a tree walk over what the parser produced, the same
// shape as `remarkBreaks`, so the WebUI needs no dependency for it.
//
// Two dollars are left alone — `$$…$$` is written on purpose — and so is display math on its
// own line, which no prose can be mistaken for.

/** The parts of an mdast node this plugin reads and writes. */
type Node = {
    type: string
    value?: string
    position?: { start?: { offset?: number }; end?: { offset?: number } }
    children?: Node[]
}

/** The file the tree came from: its value is the markdown that was parsed. */
type Source = { value?: unknown }

/** Whitespace as GitHub counts it: a space, a tab, a line ending, a form feed. */
const WHITESPACE = /\s/

/** A digit: the closing dollar may not be followed by one, or `$5 and $10` would be a formula. */
const DIGIT = /\d/

/**
 * Whether the dollars around this span are written like math. The span is the source between
 * its opening and closing dollar, so the characters that decide it are the one just inside the
 * opening dollar, the one just inside the closing one, and the one just after it.
 */
export function isMathSpan(source: string, start: number, end: number): boolean {
    const span = source.slice(start, end)
    // Two dollars: written on purpose, whatever sits between them.
    if (span[1] === '$') return true
    // The opening dollar must not be followed by whitespace, nor the closing one preceded by it.
    if (WHITESPACE.test(span[1] ?? '')) return false
    if (WHITESPACE.test(span[span.length - 2] ?? '')) return false
    // The closing dollar must not be followed by a digit.
    if (DIGIT.test(source[end] ?? '')) return false
    return true
}

/** Take back the spans whose dollars are prose, wherever they sit in the tree. */
function guard(node: Node, source: string): void {
    if (!node.children) return
    for (let i = 0; i < node.children.length; i++) {
        const child = node.children[i]
        if (child.type === 'inlineMath') {
            const start = child.position?.start?.offset
            const end = child.position?.end?.offset
            if (start !== undefined && end !== undefined && !isMathSpan(source, start, end)) {
                // The source as it was written, back in the tree as the text it is.
                node.children[i] = { type: 'text', value: source.slice(start, end) }
                continue
            }
        }
        guard(child, source)
    }
}

/**
 * A remark plugin: an inline math span whose dollars are prose is put back as the text it was.
 *
 * Only `inlineMath` is touched. A `math` span is display math on its own line, which no prose
 * can be mistaken for.
 */
export function remarkGuardedMath() {
    return (tree: Node, file: Source) => {
        const source = typeof file?.value === 'string' ? file.value : ''
        if (!source) return
        guard(tree, source)
    }
}
