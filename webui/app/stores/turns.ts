import { create } from 'zustand'
import {
    CHANNEL,
    topicDetail,
    topicFiles,
    topicHistory,
    topicJobs,
} from '../api/endpoints/chat'
import { classifyFrame, type ChatMessage, type QuestionAnswerPayload, type VizierRequestContent } from '../api/frames'
import { emptyTopic, reduce, type Action, type TopicState } from '../lib/frames'
import { handleClientCall } from '../lib/clientTools'
import { playNotify } from '../lib/sound'
import { topicKey, useConnection } from './connection'
import { useSession } from './session'
import { useAgents } from './agents'

const PAGE = 50

interface Meta {
    /** No older entries to load above the oldest held. */
    exhausted: boolean
    loadingOlder: boolean
    loaded: boolean
}

type TurnStore = {
    topics: Record<string, TopicState>
    meta: Record<string, Meta>
    /** Opens a topic: history, files and jobs, then the socket. Safe to call again. */
    open: (agentId: string, topicId: string, channel?: string) => Promise<void>
    close: (agentId: string, topicId: string) => void
    send: (
        agentId: string,
        topicId: string,
        text: string,
        attachments?: object[],
        spoken?: boolean,
        mentions?: string[]
    ) => void
    sendAudio: (agentId: string, topicId: string, attachment: object) => void
    clientEvent: (agentId: string, topicId: string, text: string, data?: unknown, sourceCallId?: string) => void
    saveClientState: (agentId: string, topicId: string, callId: string, state: unknown) => void
    /** Submit (or dismiss) a question set the agent asked: one ordinary request, claimed once on the server. */
    answerQuestions: (agentId: string, topicId: string, answer: QuestionAnswerPayload) => void
    /** Hand a sensitive value to the script that asked, or decline. The value is sent once and kept nowhere. */
    answerSensitive: (agentId: string, topicId: string, askId: string, value: string | null) => void
    command: (agentId: string, topicId: string, command: 'abort' | 'checkpoint' | 'lobotomy') => void
    react: (agentId: string, topicId: string, uid: string, emoji: string, action: 'added' | 'removed') => void
    loadOlder: (agentId: string, topicId: string) => Promise<void>
    dispatch: (agentId: string, topicId: string, action: Action) => void
}

let nextId = 1

/** The channel each open topic lives on: the WebUI's own, or `project__<id>` (spec 040). */
const channels = new Map<string, string>()

const channelOf = (agentId: string, topicId: string) =>
    channels.get(topicKey(agentId, topicId)) ?? CHANNEL

function request(
    content: VizierRequestContent,
    attachments?: object[],
    spoken?: boolean,
    mentions?: string[]
) {
    return {
        timestamp: new Date().toISOString(),
        user: useSession.getState().user?.username ?? '',
        content,
        metadata: null,
        ...(attachments?.length ? { attachments } : {}),
        ...(spoken ? { expect_audio_reply: true } : {}),
        ...(mentions?.length ? { mentions } : {}),
    }
}

export const useTurns = create<TurnStore>()((set, get) => {
    const update = (key: string, fn: (s: TopicState) => TopicState) =>
        set((cur) => ({ topics: { ...cur.topics, [key]: fn(cur.topics[key] ?? emptyTopic()) } }))
    const setMeta = (key: string, patch: Partial<Meta>) =>
        set((cur) => ({
            meta: { ...cur.meta, [key]: { ...{ exhausted: false, loadingOlder: false, loaded: false }, ...cur.meta[key], ...patch } },
        }))

    /** Re-reads the newest page, so stand-ins give way to what the server stored. */
    const refreshHistory = async (agentId: string, topicId: string) => {
        const key = topicKey(agentId, topicId)
        try {
            const entries = await topicHistory(agentId, topicId, {
                limit: PAGE,
                channel: channelOf(agentId, topicId),
            })
            update(key, (s) => reduce(s, { type: 'history', entries: entries ?? [] }))
        } catch {
            /* the stand-ins stay until the next read */
        }
    }

    /** After every change: a turn that just ended is read back, and the next queued message goes. */
    const settle = (agentId: string, topicId: string, before: TopicState['phase']) => {
        const key = topicKey(agentId, topicId)
        const state = get().topics[key]
        if (!state) return
        // The agent's live dot follows its health, which is polled: read it at once when a turn
        // starts or ends, and again once the server has settled its mark.
        if ((before === 'idle') !== (state.phase === 'idle')) {
            const { refreshHealth } = useAgents.getState()
            void refreshHealth()
            setTimeout(() => void refreshHealth(), 1000)
        }
        if (before !== 'idle' && state.phase === 'idle') void refreshHistory(agentId, topicId)
    }

    const dispatch: TurnStore['dispatch'] = (agentId, topicId, action) => {
        const key = topicKey(agentId, topicId)
        const before = get().topics[key]?.phase ?? 'idle'
        update(key, (s) => reduce(s, action))
        settle(agentId, topicId, before)
    }

    return {
        topics: {},
        meta: {},

        dispatch,

        open: async (agentId, topicId, channel = CHANNEL) => {
            const key = topicKey(agentId, topicId)
            if (get().meta[key]?.loaded) return
            channels.set(key, channel)
            update(key, (s) => (s.session.agent_id ? s : emptyTopic({ agent_id: agentId, channel, topic: topicId })))

            const [history, detail, files, jobs] = await Promise.allSettled([
                topicHistory(agentId, topicId, { limit: PAGE, channel }),
                topicDetail(agentId, topicId, channel),
                topicFiles(agentId, topicId, 0, 100, channel),
                topicJobs(agentId, topicId, channel),
            ])
            if (history.status === 'fulfilled') {
                const entries = history.value ?? []
                update(key, (s) => reduce(s, { type: 'history', entries }))
                setMeta(key, { exhausted: entries.length < PAGE })
            }
            if (detail.status === 'fulfilled' && detail.value?.is_thinking) {
                update(key, (s) => reduce(s, { type: 'busy', thinking: true }))
            }
            if (files.status === 'fulfilled') {
                update(key, (s) => reduce(s, { type: 'files', files: files.value.items }))
            }
            if (jobs.status === 'fulfilled') {
                update(key, (s) => reduce(s, { type: 'jobs', jobs: jobs.value ?? [] }))
            }
            setMeta(key, { loaded: true })

            useConnection.getState().open(agentId, topicId, {
                onFrame: (raw) => {
                    const frame = classifyFrame(raw)
                    if (!frame) return
                    // An agent's reply arrived (a streamed step or an error is not one).
                    if (frame.kind === 'response') {
                        const content = frame.response.content
                        if (typeof content === 'object' && ('message' in content || 'audio_reply' in content)) playNotify()
                    }
                    // The agent stored a message this page was drawing as waiting: the wait is over.
                    if (frame.kind === 'history_entry' && frame.entry.content.Request) {
                        dispatch(agentId, topicId, {
                            type: 'sent',
                            request: frame.entry.content.Request.content,
                            timestamp: frame.entry.content.Request.timestamp,
                        })
                    }
                    // A call for a tool this client declared: run it here and answer on this socket.
                    if (frame.kind === 'client' && 'client_tool_call' in frame.frame && frame.frame.client_tool_call.answer) {
                        const call = frame.frame.client_tool_call
                        const send = (payload: object) => useConnection.getState().send(agentId, topicId, payload)
                        void handleClientCall(call, {
                            callId: call.call_id,
                            agentId,
                            topicId,
                            sendEvent: (text, data) => get().clientEvent(agentId, topicId, text, data, call.call_id),
                            saveState: (state) => get().saveClientState(agentId, topicId, call.call_id, state),
                        }).then((answer) => send({ client_tool_result: { call_id: call.call_id, ...answer } }))
                    }
                    dispatch(agentId, topicId, { type: 'frame', frame, now: new Date().toISOString() })
                },
                // After a drop, what happened meanwhile is read back, not replayed.
                onOpen: (reopened) => {
                    if (!reopened) return
                    void refreshHistory(agentId, topicId)
                    topicDetail(agentId, topicId, channelOf(agentId, topicId))
                        .then((d) => update(key, (s) => reduce(s, { type: 'busy', thinking: !!d?.is_thinking })))
                        .catch(() => {})
                    topicJobs(agentId, topicId, channelOf(agentId, topicId))
                        .then((jobs) => update(key, (s) => reduce(s, { type: 'jobs', jobs: jobs ?? [] })))
                        .catch(() => {})
                },
            }, channelOf(agentId, topicId))
        },

        close: (agentId, topicId) => {
            useConnection.getState().close(agentId, topicId)
            const key = topicKey(agentId, topicId)
            channels.delete(key)
            set((cur) => {
                const { [key]: _t, ...topics } = cur.topics
                const { [key]: _m, ...meta } = cur.meta
                return { topics, meta }
            })
        },

        send: (agentId, topicId, text, attachments, spoken, mentions) => {
            const id = String(nextId++)
            const now = new Date().toISOString()
            // Sent whether or not a turn is running: the agent queues it the same way, and a
            // message held in this tab is gone the moment the page is left. The wait is drawn
            // until the server stores it (`sent`, or the next history read).
            const sent = useConnection.getState().send(agentId, topicId, request({ chat: text }, attachments, spoken, mentions))
            if (sent) dispatch(agentId, topicId, { type: 'send', id, text, attachments: attachments as never, spoken, now })
        },

        sendAudio: (agentId, topicId, attachment) => {
            // A recording is a message like any other: it goes now, and waits its turn at the agent.
            const now = new Date().toISOString()
            const frame = { ...request({ audio_chat: [attachment, null] } as never), expect_audio_reply: true }
            if (useConnection.getState().send(agentId, topicId, frame)) {
                dispatch(agentId, topicId, {
                    type: 'send',
                    id: String(nextId++),
                    text: '',
                    attachments: [attachment as never],
                    now,
                })
            }
        },

        react: (agentId, topicId, uid, emoji, action) => {
            useConnection.getState().send(agentId, topicId, { reaction: { message_uid: uid, emoji, action } })
        },

        clientEvent: (agentId, topicId, text, data, sourceCallId) => {
            const content = { client_event: { text, data, source_call_id: sourceCallId } } as VizierRequestContent
            const now = new Date().toISOString()
            // A click inside a page the agent drew is a request like any other: queued at the
            // agent if it is busy, not dropped here.
            if (useConnection.getState().send(agentId, topicId, request(content))) {
                dispatch(agentId, topicId, { type: 'send', id: String(nextId++), text, now })
            }
        },

        answerQuestions: (agentId, topicId, answer) => {
            const content = { question_answer: answer } as VizierRequestContent
            if (useConnection.getState().send(agentId, topicId, request(content))) {
                // Drawn at once as the person's message; the stored request, with the server's own
                // summary, replaces it when it arrives.
                dispatch(agentId, topicId, {
                    type: 'send',
                    id: String(nextId++),
                    text: answer.declined ? 'Dismissed the questions.' : 'Answered the questions.',
                    now: new Date().toISOString(),
                })
            }
        },

        answerSensitive: (agentId, topicId, askId, value) => {
            useConnection
                .getState()
                .send(agentId, topicId, { sensitive_input_answer: value === null ? { ask_id: askId, decline: true } : { ask_id: askId, value } })
        },

        saveClientState: (agentId, topicId, callId, state) => {
            useConnection.getState().send(agentId, topicId, { client_call_state: { call_id: callId, state } })
        },

        command: (agentId, topicId, command) => {
            const now = new Date().toISOString()
            if (!useConnection.getState().send(agentId, topicId, request({ command }))) return
            // Drawn where it was run, so it shows at once instead of only after a reload: the
            // abort where it happened (a message sent next lands below the trail it ended), and
            // checkpoint/lobotomy as the line they are.
            if (command === 'abort') dispatch(agentId, topicId, { type: 'abort', now })
            else dispatch(agentId, topicId, { type: 'command', command, now })
        },

        loadOlder: async (agentId, topicId) => {
            const key = topicKey(agentId, topicId)
            const meta = get().meta[key]
            const oldest = get().topics[key]?.entries.find((e) => !e.uid.startsWith('live-'))
            if (!oldest || meta?.exhausted || meta?.loadingOlder) return
            setMeta(key, { loadingOlder: true })
            try {
                const older: ChatMessage[] =
                    (await topicHistory(agentId, topicId, {
                        before: oldest.timestamp,
                        before_seq: oldest.seq,
                        limit: PAGE,
                        channel: channelOf(agentId, topicId),
                    })) ?? []
                update(key, (s) => reduce(s, { type: 'history', entries: older, position: 'older' }))
                setMeta(key, { exhausted: older.length < PAGE, loadingOlder: false })
            } catch {
                setMeta(key, { loadingOlder: false })
            }
        },
    }
})
