import type { DiffLine } from '@vizier/design'

// How revision history is shown: who made a save, and a diff as the library draws it.

export type RevisionActor = { type: 'agent' } | { type: 'user'; user_id: string; username: string } | { type: 'system' }

export type RevisionTrigger =
    | { type: 'conversation' | 'dream' | 'webui' | 'api' | 'import' | 'baseline' }
    | { type: 'rollback'; restored_from: number }

export interface DiffHunk {
    old_start: number
    old_lines: number
    new_start: number
    new_lines: number
    lines: { op: 'equal' | 'insert' | 'delete' | string; old_line: number | null; new_line: number | null; text: string }[]
}

export interface RevisionDiff {
    from_seq: number
    to_seq: number
    additions: number
    deletions: number
    hunks: DiffHunk[]
}

const VIA: Record<string, string> = {
    conversation: 'in a conversation',
    dream: 'dream cycle',
    webui: 'WebUI',
    api: 'API',
    import: 'import',
    baseline: 'first version',
}

/** "ganesha · WebUI", "dream cycle", "restored from rev 3". */
export function authorLabel(actor: RevisionActor, trigger: RevisionTrigger, agentName?: string): string {
    if (trigger.type === 'dream') return 'dream cycle'
    if (trigger.type === 'rollback') return `restored from rev ${trigger.restored_from}`
    if (trigger.type === 'baseline') return 'first version'
    const who = actor.type === 'user' ? actor.username : actor.type === 'agent' ? (agentName ?? 'the agent') : 'system'
    return `${who} · ${VIA[trigger.type] ?? trigger.type}`
}

/** A diff's lines in the library's shape: each hunk opens with its range, and lines keep their numbers. */
export function diffLines(diff: RevisionDiff): DiffLine[] {
    const out: DiffLine[] = []
    for (const h of diff.hunks) {
        out.push({ kind: 'hunk', text: `@@ -${h.old_start},${h.old_lines} +${h.new_start},${h.new_lines} @@` })
        for (const l of h.lines) {
            out.push({
                kind: l.op === 'insert' ? 'add' : l.op === 'delete' ? 'del' : 'ctx',
                text: l.text,
                oldNo: l.old_line ?? undefined,
                newNo: l.new_line ?? undefined,
            })
        }
    }
    return out
}
