import { create } from 'zustand'
import {
    topicDetail,
    topicFiles,
    topicHistory,
    topicJobs,
} from '../api/endpoints/chat'
import { classifyFrame, type ChatMessage, type VizierRequestContent } from '../api/frames'
import { emptyTopic, nextQueued, reduce, type Action, type TopicState } from '../lib/frames'
import { handleClientCall } from '../lib/clientTools'
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
    open: (agentId: string, topicId: string) => Promise<void>
    close: (agentId: string, topicId: string) => void
    send: (agentId: string, topicId: string, text: string, attachments?: object[], spoken?: boolean) => void
    sendAudio: (agentId: string, topicId: string, attachment: object) => void
    clientEvent: (agentId: string, topicId: string, text: string, data?: unknown, sourceCallId?: string) => void
    saveClientState: (agentId: string, topicId: string, callId: string, state: unknown) => void
    command: (agentId: string, topicId: string, command: 'abort' | 'checkpoint' | 'lobotomy') => void
    react: (agentId: string, topicId: string, uid: string, emoji: string, action: 'added' | 'removed') => void
    loadOlder: (agentId: string, topicId: string) => Promise<void>
    dispatch: (agentId: string, topicId: string, action: Action) => void
}

let nextId = 1

function request(content: VizierRequestContent, attachments?: object[], spoken?: boolean) {
    return {
        timestamp: new Date().toISOString(),
        user: useSession.getState().user?.username ?? '',
        content,
        metadata: null,
        ...(attachments?.length ? { attachments } : {}),
        ...(spoken ? { expect_audio_reply: true } : {}),
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
            const entries = await topicHistory(agentId, topicId, { limit: PAGE })
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
        const next = nextQueued(state)
        if (next) {
            const sent = useConnection.getState().send(agentId, topicId, request({ chat: next.text }, next.attachments, next.spoken))
            if (sent) update(key, (s) => reduce(s, { type: 'dequeue', now: new Date().toISOString() }))
        }
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

        open: async (agentId, topicId) => {
            const key = topicKey(agentId, topicId)
            if (get().meta[key]?.loaded) return
            update(key, (s) => (s.session.agent_id ? s : emptyTopic({ agent_id: agentId, channel: 'vizier-webui', topic: topicId })))

            const [history, detail, files, jobs] = await Promise.allSettled([
                topicHistory(agentId, topicId, { limit: PAGE }),
                topicDetail(agentId, topicId),
                topicFiles(agentId, topicId),
                topicJobs(agentId, topicId),
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
                    topicDetail(agentId, topicId)
                        .then((d) => update(key, (s) => reduce(s, { type: 'busy', thinking: !!d?.is_thinking })))
                        .catch(() => {})
                    topicJobs(agentId, topicId)
                        .then((jobs) => update(key, (s) => reduce(s, { type: 'jobs', jobs: jobs ?? [] })))
                        .catch(() => {})
                },
            })
        },

        close: (agentId, topicId) => {
            useConnection.getState().close(agentId, topicId)
            const key = topicKey(agentId, topicId)
            set((cur) => {
                const { [key]: _t, ...topics } = cur.topics
                const { [key]: _m, ...meta } = cur.meta
                return { topics, meta }
            })
        },

        send: (agentId, topicId, text, attachments, spoken) => {
            const key = topicKey(agentId, topicId)
            const state = get().topics[key] ?? emptyTopic()
            const id = String(nextId++)
            if (state.phase !== 'idle') {
                dispatch(agentId, topicId, { type: 'send', id, text, attachments: attachments as never, spoken })
                return
            }
            const sent = useConnection.getState().send(agentId, topicId, request({ chat: text }, attachments, spoken))
            if (sent) dispatch(agentId, topicId, { type: 'send', id, text, attachments: attachments as never, now: new Date().toISOString() })
        },

        sendAudio: (agentId, topicId, attachment) => {
            const key = topicKey(agentId, topicId)
            if ((get().topics[key]?.phase ?? 'idle') !== 'idle') return
            const frame = { ...request({ audio_chat: [attachment, null] } as never), expect_audio_reply: true }
            if (useConnection.getState().send(agentId, topicId, frame)) {
                dispatch(agentId, topicId, {
                    type: 'send',
                    id: String(nextId++),
                    text: '',
                    attachments: [attachment as never],
                    now: new Date().toISOString(),
                })
            }
        },

        react: (agentId, topicId, uid, emoji, action) => {
            useConnection.getState().send(agentId, topicId, { reaction: { message_uid: uid, emoji, action } })
        },

        clientEvent: (agentId, topicId, text, data, sourceCallId) => {
            const content = { client_event: { text, data, source_call_id: sourceCallId } } as VizierRequestContent
            const key = topicKey(agentId, topicId)
            const idle = (get().topics[key]?.phase ?? 'idle') === 'idle'
            if (useConnection.getState().send(agentId, topicId, request(content)) && idle) {
                dispatch(agentId, topicId, { type: 'send', id: String(nextId++), text, now: new Date().toISOString() })
            }
        },

        saveClientState: (agentId, topicId, callId, state) => {
            useConnection.getState().send(agentId, topicId, { client_call_state: { call_id: callId, state } })
        },

        command: (agentId, topicId, command) => {
            useConnection.getState().send(agentId, topicId, request({ command }))
            if (command === 'abort') dispatch(agentId, topicId, { type: 'abort' })
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
                    })) ?? []
                update(key, (s) => reduce(s, { type: 'history', entries: older, position: 'older' }))
                setMeta(key, { exhausted: older.length < PAGE, loadingOlder: false })
            } catch {
                setMeta(key, { loadingOlder: false })
            }
        },
    }
})
