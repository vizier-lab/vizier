/** Edit distance between two short names. */
export function distance(a: string, b: string): number {
    const row = Array.from({ length: b.length + 1 }, (_, i) => i)
    for (let i = 1; i <= a.length; i++) {
        let prev = row[0]
        row[0] = i
        for (let j = 1; j <= b.length; j++) {
            const next = row[j]
            row[j] = Math.min(row[j] + 1, row[j - 1] + 1, prev + (a[i - 1].toLowerCase() === b[j - 1].toLowerCase() ? 0 : 1))
            prev = next
        }
    }
    return row[b.length]
}

/** Up to `limit` of `candidates` that look like `name` (a typo, a different case, a prefix). */
export function nearest(name: string, candidates: readonly string[], limit = 3): string[] {
    const lower = name.toLowerCase()
    return candidates
        .map((candidate) => {
            const c = candidate.toLowerCase()
            const close = c.startsWith(lower) || lower.startsWith(c) || c.includes(lower) || lower.includes(c)
            return { candidate, score: distance(name, candidate) - (close ? 2 : 0) }
        })
        .filter(({ candidate, score }) => score <= Math.max(2, Math.floor(candidate.length / 3)))
        .sort((a, b) => a.score - b.score)
        .slice(0, limit)
        .map(({ candidate }) => candidate)
}
