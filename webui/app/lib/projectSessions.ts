import type { ProjectSessionRow } from '../api/endpoints/projects'

/** How many of a project's sessions the sidebar shows before Show all. */
export const PROJECT_SESSION_PREVIEW = 5

/**
 * A project's sessions as its sidebar rows show them: the worktree chats first (they are never
 * hidden), then the rest in the order the server gave — creation, newest first. At most `limit`
 * of the non-worktree ones are kept unless `all`; `hidden` says how many were left out, and
 * `total` how many there were, which is what decides whether Show all / Show less is offered at
 * all (it must outlive the click that hides nothing).
 */
export function projectSessionRows(
    sessions: ProjectSessionRow[],
    all: boolean,
    limit: number = PROJECT_SESSION_PREVIEW
): { rows: ProjectSessionRow[]; hidden: number; total: number } {
    const worktrees = sessions.filter((s) => !!s.worktree)
    const plain = sessions.filter((s) => !s.worktree)
    const shown = all ? plain : plain.slice(0, Math.max(0, limit))
    return { rows: [...worktrees, ...shown], hidden: plain.length - shown.length, total: plain.length }
}

/**
 * The session a bare project URL opens: the one used most recently. The list is in creation
 * order, so the first row is the newest chat, not the last one talked to; a session that has
 * never run a turn loses to one that has.
 */
export function mostRecentSession(sessions: ProjectSessionRow[]): ProjectSessionRow | undefined {
    let best: ProjectSessionRow | undefined
    for (const session of sessions) {
        const at = session.last_activity ?? Number.NEGATIVE_INFINITY
        if (!best || at > (best.last_activity ?? Number.NEGATIVE_INFINITY)) best = session
    }
    return best
}
