import type { DiffLine } from '@vizier/design'

const HUNK = /^@@ -(\d+)(?:,\d+)? \+(\d+)(?:,\d+)? @@/

/**
 * A unified diff's body as DiffView lines (spec 041 US4). File headers and `\ No newline`
 * markers are dropped; each `@@` hunk becomes a `hunk` line and the lines under it carry their
 * old/new line numbers; everything else is an addition, a removal or context.
 */
export function parseUnifiedDiff(text: string): DiffLine[] {
    const lines: DiffLine[] = []
    let oldNo = 0
    let newNo = 0
    for (const raw of text.split('\n')) {
        if (
            raw.startsWith('diff --git') ||
            raw.startsWith('index ') ||
            raw.startsWith('new file mode') ||
            raw.startsWith('deleted file mode') ||
            raw.startsWith('--- ') ||
            raw.startsWith('+++ ') ||
            raw.startsWith('\ No newline')
        )
            continue
        const hunk = HUNK.exec(raw)
        if (hunk) {
            oldNo = Number(hunk[1])
            newNo = Number(hunk[2])
            lines.push({ kind: 'hunk', text: raw })
        } else if (raw.startsWith('+')) lines.push({ kind: 'add', text: raw.slice(1), newNo: newNo++ })
        else if (raw.startsWith('-')) lines.push({ kind: 'del', text: raw.slice(1), oldNo: oldNo++ })
        else if (raw.length > 0)
            lines.push({
                kind: 'ctx',
                text: raw.startsWith(' ') ? raw.slice(1) : raw,
                oldNo: oldNo++,
                newNo: newNo++,
            })
    }
    return lines
}

export function isBinaryDiff(diff: { binary: boolean }): boolean {
    return diff.binary
}
