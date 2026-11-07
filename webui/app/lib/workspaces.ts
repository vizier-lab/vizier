// A Workspace topic as a list of things to draw (specs/053-workspace-channel/contracts/turn-rendering.md).
//
// The server sends two lists, because a trail stays in its agent's own record: the shown messages
// (`entries`, in one order for everyone) and what the agents did (`steps`, their history rows).
// This module merges them by time into people's messages, notes, and each agent's turn as runs.
// A run ends where someone else's message lands inside the turn, and the same turn goes on under a
// repeated agent head. Pure and free of JSX so it can be tested without a browser.

import type { ChatMessage, TurnBlock } from '../api/frames'
import type { WorkspaceEntry, WorkspaceStep } from '../api/endpoints/workspaces'
import { groupHistory } from './trail'

/** One stretch of an agent's turn, drawn under one head. */
export type Segment = {
    agentId: string
    blocks: TurnBlock[]
    /** The turn's final reply, in the last segment. */
    outcome?: ChatMessage
    /** A later part of a turn that someone else spoke into: it draws a repeated head. */
    continued: boolean
    /** The last part of its turn. */
    last: boolean
    /** The person stopped the turn. */
    aborted?: boolean
}

export type TimelineItem =
    | { kind: 'message'; key: string; at: number; entry: WorkspaceEntry }
    | { kind: 'note'; key: string; at: number; entry: WorkspaceEntry }
    | { kind: 'run'; key: string; at: number; segment: Segment }

/** The rows of an agent's record that make up a turn on screen. Recall and the rest never do. */
const SHOWN = ['ToolCall', 'ToolResult', 'AssistantMessage', 'Response', 'QuestionSet', 'Command', 'Checkpoint'] as const

const keptStep = (message: ChatMessage) => SHOWN.some((kind) => (message.content as Record<string, unknown>)[kind] !== undefined)

const ms = (iso: string | undefined) => {
    const t = iso ? Date.parse(iso) : NaN
    return Number.isFinite(t) ? t : NaN
}

/**
 * A shown message with no step of its own: one `mention_agent` or `send_message_to_workspace`
 * wrote, or one whose step is outside the page. It joins its author's turn at its time.
 */
function stepFor(entry: WorkspaceEntry): ChatMessage {
    const timestamp = new Date(entry.at).toISOString()
    return {
        uid: entry.history_uid ?? `entry-${entry.seq}`,
        timestamp,
        vizier_session: { agent_id: entry.author.id, channel: '', topic: '' },
        content: entry.is_final
            ? { Response: { timestamp, content: { message: { content: entry.body } } } }
            : { AssistantMessage: entry.body },
    }
}

/** The entries by the uid of the history row they were written from. */
export function entriesByUid(entries: readonly WorkspaceEntry[]): Map<string, WorkspaceEntry> {
    const byUid = new Map<string, WorkspaceEntry>()
    for (const entry of entries) {
        if (entry.history_uid) byUid.set(entry.history_uid, entry)
        else byUid.set(`entry-${entry.seq}`, entry)
    }
    return byUid
}

/** Folds a page or a live entry into the ones held, by `seq`, oldest first. */
export function mergeEntries(held: readonly WorkspaceEntry[], incoming: readonly WorkspaceEntry[]): WorkspaceEntry[] {
    const bySeq = new Map(held.map((e) => [e.seq, e]))
    for (const entry of incoming) bySeq.set(entry.seq, entry)
    return [...bySeq.values()].sort((a, b) => a.seq - b.seq)
}

/** Folds steps into the ones held, by uid: a known one is replaced, a new one added. */
export function mergeSteps(held: readonly WorkspaceStep[], incoming: readonly WorkspaceStep[]): WorkspaceStep[] {
    const byUid = new Map(held.map((s) => [s.entry.uid, s]))
    for (const step of incoming) byUid.set(step.entry.uid, step)
    return [...byUid.values()]
}

const blockTime = (block: TurnBlock) => ms(block.kind === 'trail' ? block.startedAt : block.at)

/** Splits one agent turn where `interruptions` (ms) land between its parts. */
function segmentsOf(
    agentId: string,
    turnKey: string,
    blocks: TurnBlock[],
    outcome: ChatMessage | undefined,
    aborted: boolean | undefined,
    interruptions: number[]
): { key: string; at: number; segment: Segment }[] {
    // The outcome counts as a part of its own, so a final reply that lands after someone spoke
    // draws under its own head.
    type Part = { block?: TurnBlock; time: number }
    const parts: Part[] = blocks.map((block) => ({ block, time: blockTime(block) }))
    if (outcome) parts.push({ time: ms(outcome.timestamp) })
    // A part without a time stays with the one before it.
    let previous = -Infinity
    for (const part of parts) {
        if (Number.isNaN(part.time)) part.time = previous
        previous = part.time
    }
    const groups: Part[][] = []
    let lastCount = -1
    for (const part of parts) {
        const count = interruptions.filter((t) => t < part.time).length
        if (count !== lastCount || groups.length === 0) groups.push([])
        groups[groups.length - 1].push(part)
        lastCount = count
    }
    return groups.map((group, i) => {
        const hasOutcome = group.some((p) => !p.block)
        return {
            key: `${agentId}:${turnKey}:${i}`,
            at: group[0].time,
            segment: {
                agentId,
                blocks: group.flatMap((p) => (p.block ? [p.block] : [])),
                outcome: hasOutcome ? outcome : undefined,
                continued: i > 0,
                last: i === groups.length - 1,
                aborted: i === groups.length - 1 ? aborted : undefined,
            },
        }
    })
}

/** Everything to draw for a page, in order. */
export function buildTimeline(entries: readonly WorkspaceEntry[], steps: readonly WorkspaceStep[]): TimelineItem[] {
    const items: TimelineItem[] = []
    const stepUids = new Set(steps.map((s) => s.entry.uid))

    // Each agent's record: its steps, plus the shown messages that have no step.
    const byAgent = new Map<string, { message: ChatMessage; order: number }[]>()
    const add = (agentId: string, message: ChatMessage) => {
        const list = byAgent.get(agentId) ?? []
        list.push({ message, order: list.length })
        byAgent.set(agentId, list)
    }
    for (const step of steps) if (keptStep(step.entry)) add(step.agent_id, step.entry)
    for (const entry of entries) {
        if (entry.kind !== 'agent') continue
        if (entry.history_uid && stepUids.has(entry.history_uid)) continue
        add(entry.author.id, stepFor(entry))
    }

    for (const entry of entries) {
        if (entry.kind === 'person') items.push({ kind: 'message', key: `m${entry.seq}`, at: entry.at, entry })
        else if (entry.kind === 'note') items.push({ kind: 'note', key: `n${entry.seq}`, at: entry.at, entry })
    }

    for (const [agentId, records] of byAgent) {
        const ordered = records
            .slice()
            .sort((a, b) => {
                const d = ms(a.message.timestamp) - ms(b.message.timestamp)
                return Number.isFinite(d) && d !== 0 ? d : a.order - b.order
            })
            .map((r) => r.message)
        // What lands in this agent's turns from others: people, notes, and other agents' messages.
        const others = entries.filter((e) => e.author.id !== agentId || e.kind !== 'agent').map((e) => e.at)
        for (const turn of groupHistory(ordered)) {
            if (turn.blocks.length === 0 && !turn.outcome) continue
            for (const { key, at, segment } of segmentsOf(agentId, turn.key, turn.blocks, turn.outcome, turn.aborted, others)) {
                items.push({ kind: 'run', key, at, segment })
            }
        }
    }

    return items.sort((a, b) => a.at - b.at || a.key.localeCompare(b.key))
}

/** A topic name as the server will take it: lower-case letters, digits and dashes. */
export const normalizeTopicName = (typed: string) => typed.trim().toLowerCase()

/** Why a typed topic name can't be used, or undefined. `except` is the topic being renamed. */
export function topicNameProblem(typed: string, topics: readonly { id: string; name: string }[], except?: string): string | undefined {
    const name = normalizeTopicName(typed)
    if (!name) return undefined
    if (topics.some((t) => t.name === name && t.id !== except)) return 'A topic with that name already exists.'
    if (!/^[a-z0-9][a-z0-9-]*$/.test(name) || name.length > 64) return 'Use lower-case letters, digits and dashes, up to 64.'
    return undefined
}
