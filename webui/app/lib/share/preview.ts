// What the person sees before anything is stored: counts, what was left out, the size against the
// limit, and a refusal with the ways out. Built from the same result the request is built from, so
// the numbers shown are the numbers stored.
import { SHARE_LIMITS } from './limits'
import { byteLength, type Removals, type Snapshot } from './schema'
import { tooManyTurns, type SerializeResult } from './serialize'

export type RemovalKind = keyof Removals

export const REMOVAL_LABELS: Record<RemovalKind, { one: string; many: string }> = {
    credentials: { one: 'credential or sensitive request', many: 'credentials or sensitive requests' },
    paths: { one: 'folder path shortened', many: 'folder paths shortened' },
    recall: { one: 'memory passage', many: 'memory passages' },
    reasoning: { one: 'reasoning step', many: 'reasoning steps' },
    media: { one: 'file left out', many: 'files left out' },
}

export interface Refusal {
    reason: 'empty' | 'too_large' | 'too_many_turns'
    message: string
    /** The ways to make it fit; empty when there is none. */
    options: string[]
}

export interface Preview {
    messages: number
    images: number
    pages: number
    questions: number
    /** Tool calls kept in the snapshot (trail on). */
    trailEntries: number
    sizeBytes: number
    limitBytes: number
    includeTrail: boolean
    /** Same object shape the request carries as `removals`. */
    removals: Removals
    removed: { kind: RemovalKind; count: number; label: string }[]
    placeholders: { name: string; reason: string; label: string }[]
    omittedTrail: number
    truncated: number
    turnInProgress: boolean
    notes: string[]
    refusal: Refusal | null
    canShare: boolean
}

/** The bytes the server will keep for this snapshot: its UTF-8 JSON text. */
export const snapshotBytes = (snapshot: Snapshot): number => byteLength(JSON.stringify(snapshot))

export const formatBytes = (n: number): string => (n >= 1024 * 1024 ? `${(n / 1024 / 1024).toFixed(1)} MiB` : n >= 1024 ? `${Math.round(n / 1024)} KiB` : `${n} B`)

export function buildPreview(result: SerializeResult): Preview {
    const { snapshot } = result
    let messages = 0
    let pages = 0
    let questions = 0
    let trailEntries = 0
    for (const turn of snapshot.turns) {
        trailEntries += turn.trail?.length ?? 0
        for (const p of turn.parts) {
            if (p.kind === 'text') messages++
            else if (p.kind === 'ui') pages++
            else if (p.kind === 'question') questions++
        }
    }
    const images = Object.keys(snapshot.media).length
    const sizeBytes = snapshotBytes(snapshot)
    const limitBytes = SHARE_LIMITS.bodyBytes - SHARE_LIMITS.envelopeBytes
    const removals = { ...result.removals }
    const removed = (Object.keys(removals) as RemovalKind[])
        .filter((k) => removals[k] > 0)
        .map((kind) => ({ kind, count: removals[kind], label: `${removals[kind]} ${removals[kind] === 1 ? REMOVAL_LABELS[kind].one : REMOVAL_LABELS[kind].many}` }))

    const notes: string[] = []
    if (result.turnInProgress) notes.push('The reply that is still being written is not included.')
    if (!snapshot.include_trail && result.omittedTrail > 0) notes.push('The agent’s activity (tool calls and results) is not included.')
    if (snapshot.include_trail) notes.push(`The agent’s activity is included (${trailEntries} ${trailEntries === 1 ? 'step' : 'steps'}), with credentials and folder paths removed.`)
    if (result.truncated > 0) notes.push(`${result.truncated} very long ${result.truncated === 1 ? 'message was' : 'messages were'} cut.`)

    let refusal: Refusal | null = null
    if (snapshot.turns.length > SHARE_LIMITS.turns) refusal = { reason: 'too_many_turns', message: tooManyTurns(), options: ['Start a new conversation with the part you want to share.'] }
    else if (!snapshot.turns.some((t) => t.parts.length > 0 || (t.trail?.length ?? 0) > 0)) {
        refusal = { reason: 'empty', message: 'There is nothing to share yet: this conversation has no messages.', options: [] }
    } else if (sizeBytes > limitBytes) {
        const mediaBytes = Object.values(snapshot.media).reduce((n, m) => n + m.data.length, 0)
        const trailBytes = snapshot.turns.reduce((n, t) => n + (t.trail ? byteLength(JSON.stringify(t.trail)) : 0), 0)
        const options: string[] = []
        if (snapshot.include_trail && trailBytes > 0) options.push(`Turn off the agent’s activity (about ${formatBytes(trailBytes)}).`)
        if (images > 0) options.push(`Share a conversation with fewer images (${images} images take about ${formatBytes(mediaBytes)}).`)
        options.push('Start a new conversation with the part you want to share.')
        refusal = { reason: 'too_large', message: `This conversation would be ${formatBytes(sizeBytes)}; a shared conversation can be at most ${formatBytes(limitBytes)}. Nothing was cut.`, options }
    }

    return {
        messages, images, pages, questions, trailEntries, sizeBytes, limitBytes,
        includeTrail: snapshot.include_trail,
        removals, removed,
        placeholders: result.placeholders.map((p) => ({ name: p.name, reason: p.reason, label: p.label })),
        omittedTrail: result.omittedTrail,
        truncated: result.truncated,
        turnInProgress: result.turnInProgress,
        notes, refusal, canShare: refusal === null,
    }
}
