import type { DreamChange, DreamCycle, DreamNight } from '../api/endpoints/dream'
import { absoluteTime } from './format'
import { docHref } from './memoryTree'

/** "Oct 20 · 3 memories · CORE", the line a night says on hover. */
export function nightHint(n: DreamNight): string {
    const day = new Date(`${n.date}T00:00:00`).toLocaleDateString('en-US', { month: 'short', day: 'numeric' })
    if (!n.cycle_id) return `${day} · no dream`
    const parts = [day, `${n.memories} memor${n.memories === 1 ? 'y' : 'ies'}`]
    if (n.core) parts.push('CORE')
    if (n.state === 'running') parts.push('running')
    if (n.state === 'failed') parts.push('failed')
    return parts.join(' · ')
}

/** Labels under some of the strip's nights: the first, every fifth, and today. */
export function nightLabel(n: DreamNight, index: number, count: number): string | undefined {
    if (index === count - 1) return 'today'
    if (index % 5 !== 0) return undefined
    return new Date(`${n.date}T00:00:00`).toLocaleDateString('en-US', { month: 'short', day: 'numeric' })
}

/** "Oct 20, 03:00 – 03:12", or "Oct 20, 03:00 · running". */
export function cycleWhen(c: Pick<DreamCycle, 'started_at' | 'ended_at' | 'state'>): string {
    const start = absoluteTime(c.started_at)
    if (c.state === 'running' || !c.ended_at) return `${start} · running`
    const end = new Date(c.ended_at).toLocaleTimeString('en-GB', { hour: '2-digit', minute: '2-digit' })
    return `${start} – ${end}`
}

/** Where a change leads: the document in Memory, or the CORE revision. */
export function changeHref(agentId: string, c: DreamChange): string {
    if (c.kind === 'core') return `/${agentId}/core`
    return docHref(agentId, c.bundle ?? '', c.path ?? '')
}

export function changeText(c: DreamChange): string {
    if (c.kind === 'core') return `rev ${c.seq}`
    return `${c.bundle}/${c.path}`
}

/** "2 written · 1 revised · CORE" or "no changes". */
export function cycleSummary(c: DreamCycle): string {
    const parts: string[] = []
    if (c.written) parts.push(`${c.written} written`)
    if (c.revised) parts.push(`${c.revised} revised`)
    if (c.removed) parts.push(`${c.removed} removed`)
    if (c.core) parts.push('CORE')
    return parts.join(' · ') || 'no changes'
}
