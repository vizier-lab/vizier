import { AgentHeader, AgentReply, MentionNote, OriginBadge, StoppedNote, UserMessage } from '@vizier/design'
import { useMemo } from 'react'
import type { VizierAttachment } from '../../api/frames'
import { API_BASE } from '../../api/transport'
import type { WorkspaceEntry } from '../../api/endpoints/workspaces'
import { Enter, stagger } from '../../lib/motion'
import { usePrices } from '../../lib/prices'
import { blockDurationMs, outcomeDurationMs } from '../../lib/trail'
import { mentionsAsCode } from '../../lib/mentions'
import { isAudioReply, replyAttachments, replyError, replyStats, replyText, statsLine, time } from '../../lib/messages'
import { buildTimeline, entriesByUid, waitingAgents, type Segment, type TimelineItem } from '../../lib/workspaces'
import { Markdown, MentionHandles } from '../shared/Markdown'
import { Reactions } from '../chat/Reactions'
import { ReplyMedia } from '../chat/ReplyMedia'
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
    onReact: (uid: string, emoji: string, action: 'added' | 'removed') => void
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
    onReact,
}: {
    onReact: Props['onReact']
    segment: Segment
    byUid: Map<string, WorkspaceEntry>
    names: Props['names']
    running: boolean
}) {
    const name = names[segment.agentId] ?? segment.agentId
    const prices = usePrices()
    const outcome = segment.outcome
    const content = outcome?.content.Response?.content
    const text = content ? replyText(content) : null
    const attachments = replyAttachments(content, outcome?.content.Response?.attachments)
    const spoken = isAudioReply(content)
    const stats = content ? replyStats(content) : undefined
    if (stats?.provider && stats.model) prices.load(stats.provider, stats.model)
    const price = stats?.provider && stats.model ? prices.prices[`${stats.provider}/${stats.model}`] : undefined
    const live = running && segment.last && !outcome
    const final = outcome ? byUid.get(outcome.uid) : undefined
    const failed = !!(content && replyError(content))
    // As in a direct chat: one stretch of work shows the turn's time, several show their own.
    const inParts = segment.blocks.length > 1
    const lastTrail = segment.blocks.map((b) => b.kind).lastIndexOf('trail')
    return (
        <div className="flex flex-col gap-10">
            <AgentHeader agentId={segment.agentId} name={name} time={time(outcome?.timestamp)} live={live} />
            {segment.blocks.map((block, i) => {
                if (block.kind === 'message') {
                    const entry = byUid.get(block.id)
                    const handles = entry?.mentions.map((m) => m.handle)
                    return (
                        <div key={block.id} className="flex flex-col gap-6">
                            {entry?.origin && <OriginBadge kind={entry.origin.kind} label={entry.origin.label} />}
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
                        durationMs={inParts ? blockDurationMs(block) : (outcomeDurationMs(outcome) ?? blockDurationMs(block))}
                        failed={i === lastTrail && failed}
                        aborted={i === lastTrail && segment.aborted}
                        agentName={name}
                    />
                )
            })}
            {live && segment.blocks[segment.blocks.length - 1]?.kind !== 'trail' && <Trail trail={[]} live agentName={name} />}
            {outcome && (text != null || attachments.length > 0) && (
                <AgentReply
                    stats={stats ? statsLine(stats, price, inParts ? outcomeDurationMs(outcome) : undefined) : undefined}
                    onCopy={text != null ? () => void navigator.clipboard?.writeText(text) : undefined}
                    reactions={<Reactions reactions={outcome.reactions ?? []} disabled={!final?.history_uid} onToggle={(emoji, action) => onReact(outcome.uid, emoji, action)} />}
                >
                    <div className="flex flex-col gap-16">
                        {/* A spoken reply's transcript sits behind "Show transcript" in its player. */}
                        {text != null && text !== '' && !spoken && <Markdown>{mentionsAsCode(text, final?.mentions.map((m) => m.handle))}</Markdown>}
                        <ReplyMedia attachments={attachments} transcript={text} />
                    </div>
                </AgentReply>
            )}
            {outcome?.content.Response?.resumed && <StoppedNote>Picked up where it left off after vizier restarted.</StoppedNote>}
            {outcome && <Failed entry={final} names={names} />}
        </div>
    )
}

/** A message's files, in the shape a direct chat's attachments are drawn from. */
const entryAttachments = (entry: WorkspaceEntry): VizierAttachment[] =>
    entry.files.map((f) => ({ filename: f.name, content: { local: `${API_BASE}/files/${f.file_id}` } }))

function Item({ item, me, byUid, names, working, onReact }: { item: TimelineItem; me: string; byUid: Map<string, WorkspaceEntry>; names: Props['names']; working: Props['working']; onReact: Props['onReact'] }) {
    if (item.kind === 'note') return <Note>{item.entry.body}</Note>
    if (item.kind === 'run') {
        return <Run segment={item.segment} byUid={byUid} names={names} running={!!working[item.segment.agentId]} onReact={onReact} />
    }
    const entry = item.entry
    const mine = entry.author.id === me
    const handles = entry.mentions.map((m) => m.handle)
    const files = entryAttachments(entry)
    return (
        <UserMessage
            side={mine ? 'right' : 'left'}
            name={mine ? 'You' : entry.author.name}
            seed={entry.author.id}
            time={time(new Date(entry.at).toISOString())}
            mentioned={!mine && handles.includes(me)}
            attachments={files.length ? <ReplyMedia attachments={files} /> : undefined}
        >
            {entry.origin && <OriginBadge kind={entry.origin.kind} label={entry.origin.label} />}
            {entry.body !== '' && <Markdown size="document">{mentionsAsCode(entry.body, handles)}</Markdown>}
            <Failed entry={entry} names={names} />
        </UserMessage>
    )
}

/** A topic's conversation: people's messages, notes, and each agent's turn as trail, message, trail, reply. */
export function WorkspaceTimeline({ entries, steps, me, names, working, onReact }: Props) {
    const mentionable = useMemo(() => ({ handles: new Set(Object.keys(names)), me }), [names, me])
    const items = useMemo(() => buildTimeline(entries, steps), [entries, steps])
    const byUid = useMemo(() => entriesByUid(entries), [entries])
    return (
        <MentionHandles.Provider value={mentionable}>
            <div className="flex flex-col gap-20">
                {/* Arrivals wait a beat on one another, as in a direct chat. */}
                {items.map((item, i) => (
                    <Enter key={item.key} delay={stagger(i)}>
                        <Item item={item} me={me} byUid={byUid} names={names} working={working} onReact={onReact} />
                    </Enter>
                ))}
                {/* Waiting on the model, with no step saved yet: the live line a direct chat shows. */}
                {waitingAgents(items, working).map((agent) => (
                    <Enter key={`waiting-${agent}`}>
                        <Run
                            segment={{ agentId: agent, blocks: [], continued: false, last: true }}
                            byUid={byUid}
                            names={names}
                            running
                            onReact={onReact}
                        />
                    </Enter>
                ))}
            </div>
        </MentionHandles.Provider>
    )
}
