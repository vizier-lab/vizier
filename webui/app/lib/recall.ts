import type { ChatMessage, RecalledPassage } from '../api/frames'

export type Recalled = RecalledPassage & { origin: 'auto' | 'search'; uid: string }

/**
 * Every passage recalled in `entries`: the newest recollection first, and within one
 * recollection the best match first. The server records a `Recall` entry's passages in score
 * order (`dedupe_passages`), but sorting here keeps that order independent of storage and of
 * entries written before it was guaranteed.
 */
export function recalledFrom(entries: ChatMessage[]): Recalled[] {
    const out: Recalled[] = []
    for (let i = entries.length - 1; i >= 0; i--) {
        const e = entries[i]
        const recall = e.content.Recall
        if (!recall) continue
        const passages = [...recall.passages].sort((a, b) => b.score - a.score)
        for (const p of passages) out.push({ ...p, origin: recall.origin, uid: e.uid })
    }
    return out
}
