/** `@` mention parsing for project chats (spec 041 US5). */

/** The `@` token being typed at the end of the input, or null. */
export function mentionQuery(value: string): { start: number; query: string } | null {
    const match = /(^|\s)@([^\s@]*)$/.exec(value)
    if (!match) return null
    const start = match.index + match[1].length
    return { start, query: match[2] }
}

/** Picked mentions that are still present in the text, as the frame sends them. */
export function pickedMentions(text: string, picked: readonly string[]): string[] {
    const present = new Set<string>()
    for (const match of text.matchAll(/(^|\s)@([^\s]+)/g)) present.add(match[2])
    return picked.filter((path) => present.has(path))
}

/** Replace the token being typed with the picked path, leaving a space to keep typing. */
export function insertMention(value: string, start: number, path: string): string {
    return `${value.slice(0, start)}@${path} `
}

/**
 * Drop a response that arrived for an older query: the caller keeps a request id and only
 * applies the result whose id is still the newest one.
 */
export function isCurrent(requestId: number, latest: number): boolean {
    return requestId === latest
}
