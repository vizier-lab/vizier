// What a Workspace topic socket's frames do to the topic held on screen. Pure, so the same
// handling serves the browser, the desktop app and the tests (contracts/socket-frames.md).

import type { ConnectionState } from '../api/transport/common'
import type { WorkspaceEntry, WorkspaceFrame, WorkspaceStep } from '../api/endpoints/workspaces'
import { mergeEntries, mergeSteps } from './workspaces'

export type TopicView = {
    entries: WorkspaceEntry[]
    steps: WorkspaceStep[]
    /** Older entries exist above what is held: the `before` for the next page. */
    nextBefore: number | null
    /** Agents with a turn running, by the last `agent_state` frame. */
    working: Record<string, true>
    connection: ConnectionState
    loaded: boolean
    /** The newest `seq` the server said it has; the held entries are missing some if it is higher. */
    latestSeq: number
    /** The page must be read again: the socket lagged, or reopened after a drop. */
    stale: boolean
    /** Members changed: the member list must be read again. */
    membersStale: boolean
    /** The person was removed or the Workspace deleted. */
    closed: 'removed' | 'deleted' | 'token_expired' | null
    /** The last post that was refused, for an inline notice. */
    postError: string | null
}

export const emptyTopicView = (): TopicView => ({
    entries: [],
    steps: [],
    nextBefore: null,
    working: {},
    connection: 'connecting',
    loaded: false,
    latestSeq: 0,
    stale: false,
    membersStale: false,
    closed: null,
    postError: null,
})

const asRecord = (frame: unknown): Record<string, unknown> | null =>
    frame && typeof frame === 'object' ? (frame as Record<string, unknown>) : null

/** Applies one frame. An unknown frame changes nothing, so a newer server cannot break an older page. */
export function applyFrame(state: TopicView, raw: unknown): TopicView {
    const frame = asRecord(raw) as Partial<Record<string, never>> & Record<string, unknown> | null
    if (!frame) return state
    const f = frame as unknown as Record<string, any> // eslint-disable-line @typescript-eslint/no-explicit-any

    if (f.hello) {
        const working: Record<string, true> = {}
        for (const w of (f.hello as { working: { agent: string }[] }).working ?? []) working[w.agent] = true
        const latestSeq = (f.hello as { latest_seq: number }).latest_seq ?? 0
        return { ...state, working, latestSeq, stale: state.stale || latestSeq > (state.entries.at(-1)?.seq ?? 0) }
    }
    if (f.workspace_entry) {
        const entry = f.workspace_entry as WorkspaceEntry
        return {
            ...state,
            entries: mergeEntries(state.entries, [entry]),
            latestSeq: Math.max(state.latestSeq, entry.seq),
            postError: null,
        }
    }
    if (f.history_entry) {
        const { agent_id, entry } = f.history_entry as WorkspaceStep
        return { ...state, steps: mergeSteps(state.steps, [{ agent_id, entry }]) }
    }
    if (f.workspace_mentions) {
        const { seq, mentions } = f.workspace_mentions as { seq: number; mentions: WorkspaceEntry['mentions'] }
        return { ...state, entries: state.entries.map((e) => (e.seq === seq ? { ...e, mentions } : e)) }
    }
    if (f.reactions) {
        const { message_uid, reactions } = f.reactions as { message_uid: string; reactions: WorkspaceStep['entry']['reactions'] }
        return {
            ...state,
            steps: state.steps.map((s) => (s.entry.uid === message_uid ? { ...s, entry: { ...s.entry, reactions } } : s)),
        }
    }
    if (f.agent_state) {
        const { agent, phase } = f.agent_state as { agent: string; phase: 'thinking' | 'idle' }
        const working = { ...state.working }
        if (phase === 'thinking') working[agent] = true
        else delete working[agent]
        return { ...state, working }
    }
    if (f.member_changed) return { ...state, membersStale: true }
    if (f.closed) return { ...state, closed: (f.closed as { reason: TopicView['closed'] }).reason }
    if (f.resync) return { ...state, stale: true }
    if (f.post_error) return { ...state, postError: (f.post_error as { message: string }).message }
    return state
}

/** A page read from the server: entries and steps are folded in, and the cursor is kept when it is the older one. */
export function applyPage(
    state: TopicView,
    page: { entries: WorkspaceEntry[]; steps: WorkspaceStep[]; next_before: number | null },
    older: boolean
): TopicView {
    return {
        ...state,
        entries: mergeEntries(state.entries, page.entries),
        steps: mergeSteps(state.steps, page.steps),
        // Reading the newest page again must not move the cursor back to a newer position.
        nextBefore: older || !state.loaded ? page.next_before : state.nextBefore,
        loaded: true,
        stale: false,
    }
}

export type { WorkspaceFrame }
