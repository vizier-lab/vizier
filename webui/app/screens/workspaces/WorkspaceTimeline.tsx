import { AgentHeader, AgentReply, MentionNote, OriginBadge, UserMessage } from '@vizier/design'
import { useMemo } from 'react'
import type { WorkspaceEntry } from '../../api/endpoints/workspaces'
import { blockDurationMs } from '../../lib/trail'
import { mentionsAsCode } from '../../lib/mentions'
import { replyStats, replyText, statsLine, time } from '../../lib/messages'
import { buildTimeline, entriesByUid, type Segment, type TimelineItem } from '../../lib/workspaces'
import { Markdown } from '../shared/Markdown'
import { Trail } from '../chat/Trail'

type Props = {
    entries: WorkspaceEntry[]
    steps: Parameters<typeof buildTimeline>[1]
    /** The signed-in person, to put their messages on the right and to mark mentions of them. */
    me: string
    /** Display names by agent id. */
    names: Record<string, string>
    /** Agents with a turn running. */
    working: Record<string, true>
}

/** Why an agent was not woken, in a sentence. */
function failure(handle: string, name: string | undefined): string {
    return `${name ?? handle} is unavailable, so it was not woken`
}

/** A line the system adds, centred between two rules: a command someone ran. */
function Note({ children }: { children: string }) {
    return (
        <div className="flex items-center gap-8 type-small text-muted before:h-px before:flex-1 before:bg-border after:h-px after:flex-1 after:bg-border">
            <span>{children}</span>
        </div>
    )
}

/** An agent's mentions that did not wake anyone. A successful mention draws nothing beyond its text. */
function Failed({ entry, names }: { entry: WorkspaceEntry | undefined; names: Props['names'] }) {
    const failed = entry?.mentions.filter((m) => m.kind === 'agent' && m.state === 'unavailable') ?? []
    if (failed.length === 0) return null
    return (
        <div className="flex flex-col gap-4">
            {failed.map((m) => (
                <MentionNote key={m.handle}>{failure(m.handle, names[m.handle])}</MentionNote>
            ))}
        </div>
    )
}

function Run({
    segment,
    byUid,
    names,
    running,
}: {
    segment: Segment
    byUid: Map<string, WorkspaceEntry>
    names: Props['names']
    running: boolean
}) {
    const name = names[segment.agentId] ?? segment.agentId
    const outcome = segment.outcome
    const content = outcome?.content.Response?.content
    const text = content ? replyText(content) : null
    const stats = content ? replyStats(content) : undefined
    const live = running && segment.last && !outcome
    const final = outcome ? byUid.get(outcome.uid) : undefined
    return (
        <div className="flex flex-col gap-10">
            <AgentHeader agentId={segment.agentId} name={name} time={time(outcome?.timestamp)} />
            {segment.blocks.map((block, i) => {
                if (block.kind === 'message') {
                    const entry = byUid.get(block.id)
                    const handles = entry?.mentions.map((m) => m.handle)
                    return (
                        <div key={block.id} className="flex flex-col gap-6">
                            <Markdown>{mentionsAsCode(block.text, handles)}</Markdown>
                            <Failed entry={entry} names={names} />
                        </div>
                    )
                }
                const open = live && i === segment.blocks.length - 1
                return (
                    <Trail
                        key={`${block.events[0]?.id ?? i}-${open ? 'open' : 'done'}`}
                        trail={block.events}
                        live={open}
                        turnRunning={live}
                        durationMs={blockDurationMs(block)}
                        failed={segment.last && !!outcome && !text}
                        aborted={segment.last && segment.aborted}
                        agentName={name}
                    />
                )
            })}
            {live && segment.blocks[segment.blocks.length - 1]?.kind !== 'trail' && <Trail trail={[]} live agentName={name} />}
            {outcome && text != null && (
                <AgentReply
                    stats={stats ? statsLine(stats) : undefined}
                    onCopy={() => void navigator.clipboard?.writeText(text)}
                >
                    <Markdown>{mentionsAsCode(text, final?.mentions.map((m) => m.handle))}</Markdown>
                </AgentReply>
            )}
            {outcome && <Failed entry={final} names={names} />}
        </div>
    )
}

function Item({ item, me, byUid, names, working }: { item: TimelineItem; me: string; byUid: Map<string, WorkspaceEntry>; names: Props['names']; working: Props['working'] }) {
    if (item.kind === 'note') return <Note>{item.entry.body}</Note>
    if (item.kind === 'run') {
        return <Run segment={item.segment} byUid={byUid} names={names} running={!!working[item.segment.agentId]} />
    }
    const entry = item.entry
    const mine = entry.author.id === me
    const handles = entry.mentions.map((m) => m.handle)
    return (
        <UserMessage
            side={mine ? 'right' : 'left'}
            name={mine ? 'You' : entry.author.name}
            seed={entry.author.id}
            time={time(new Date(entry.at).toISOString())}
            mentioned={!mine && handles.includes(me)}
        >
            {entry.origin && <OriginBadge kind={entry.origin.kind} label={entry.origin.label} />}
            <Markdown size="document">{mentionsAsCode(entry.body, handles)}</Markdown>
            <Failed entry={entry} names={names} />
        </UserMessage>
    )
}

/** A topic's conversation: people's messages, notes, and each agent's turn as trail, message, trail, reply. */
export function WorkspaceTimeline({ entries, steps, me, names, working }: Props) {
    const items = useMemo(() => buildTimeline(entries, steps), [entries, steps])
    const byUid = useMemo(() => entriesByUid(entries), [entries])
    return (
        <div className="flex flex-col gap-20">
            {items.map((item) => (
                <Item key={item.key} item={item} me={me} byUid={byUid} names={names} working={working} />
            ))}
        </div>
    )
}
