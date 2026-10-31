import type { DiffLine } from '@vizier/design'

/**
 * A unified diff's body as DiffView lines (spec 041 US4). File headers, `@@` hunks and
 * `\ No newline` markers are dropped; everything else is an addition, a removal or context.
 */
export function parseUnifiedDiff(text: string): DiffLine[] {
    const lines: DiffLine[] = []
    for (const raw of text.split('\n')) {
        if (
            raw.startsWith('diff --git') ||
            raw.startsWith('index ') ||
            raw.startsWith('new file mode') ||
            raw.startsWith('deleted file mode') ||
            raw.startsWith('--- ') ||
            raw.startsWith('+++ ') ||
            raw.startsWith('@@') ||
            raw.startsWith('\\ No newline')
        )
            continue
        if (raw.startsWith('+')) lines.push({ kind: 'add', text: raw.slice(1) })
        else if (raw.startsWith('-')) lines.push({ kind: 'del', text: raw.slice(1) })
        else if (raw.length > 0) lines.push({ kind: 'ctx', text: raw.startsWith(' ') ? raw.slice(1) : raw })
    }
    return lines
}

export function isBinaryDiff(diff: { binary: boolean }): boolean {
    return diff.binary
}
