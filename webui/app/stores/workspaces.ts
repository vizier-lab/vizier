import { create } from 'zustand'
import { transport, type TopicHandle } from '../api/transport'
import {
    getTimeline,
    listWorkspaces,
    postEntry,
    type PostResult,
    type WorkspaceSummary,
} from '../api/endpoints/workspaces'
import { applyFrame, applyPage, emptyTopicView, type TopicView } from '../lib/workspaceFrames'

/** The key of one open topic. */
export const topicKey = (workspaceId: string, topicId: string) => `${workspaceId}/${topicId}`

type WorkspaceStore = {
    /** The Workspaces the person is in, for the rail. */
    list: WorkspaceSummary[]
    loaded: boolean
    /** Each open topic: what it holds, whether its socket is up, who is working. */
    topics: Record<string, TopicView>
    load: () => Promise<void>
    /** Opens a topic's socket and reads its newest page; the returned function closes it. */
    open: (workspaceId: string, topicId: string) => () => void
    loadOlder: (workspaceId: string, topicId: string) => Promise<void>
    /** Posts a message or runs a command. The result lists what was written. */
    post: (workspaceId: string, topicId: string, text: string) => Promise<PostResult>
    retry: (workspaceId: string, topicId: string) => void
    reset: () => void
}

const handles = new Map<string, TopicHandle>()

export const useWorkspaces = create<WorkspaceStore>()((set, get) => {
    const patch = (key: string, change: (view: TopicView) => TopicView) =>
        set((s) => ({ topics: { ...s.topics, [key]: change(s.topics[key] ?? emptyTopicView()) } }))

    /** Reads the newest page; every open does it, so a reopened socket fills the gap it missed. */
    const readNewest = async (workspaceId: string, topicId: string) => {
        const key = topicKey(workspaceId, topicId)
        try {
            const page = await getTimeline(workspaceId, topicId)
            patch(key, (v) => applyPage(v, page, false))
        } catch {
            patch(key, (v) => ({ ...v, loaded: true }))
        }
    }

    return {
        list: [],
        loaded: false,
        topics: {},

        load: async () => {
            try {
                set({ list: (await listWorkspaces()) ?? [], loaded: true })
            } catch {
                set({ loaded: true })
            }
        },

        open: (workspaceId, topicId) => {
            const key = topicKey(workspaceId, topicId)
            handles.get(key)?.close()
            patch(key, (v) => ({ ...v, connection: 'connecting', closed: null }))
            const handle = transport.openWorkspace(
                { workspaceId, topicId },
                {
                    onFrame: (frame) => {
                        patch(key, (v) => applyFrame(v, frame))
                        const view = get().topics[key]
                        if (view?.stale) void readNewest(workspaceId, topicId)
                        if (view?.membersStale) {
                            patch(key, (v) => ({ ...v, membersStale: false }))
                            void get().load()
                        }
                    },
                    onState: (connection) => patch(key, (v) => ({ ...v, connection })),
                    onOpen: () => void readNewest(workspaceId, topicId),
                }
            )
            handles.set(key, handle)
            return () => {
                handle.close()
                if (handles.get(key) === handle) handles.delete(key)
            }
        },

        loadOlder: async (workspaceId, topicId) => {
            const key = topicKey(workspaceId, topicId)
            const before = get().topics[key]?.nextBefore
            if (before == null) return
            try {
                const page = await getTimeline(workspaceId, topicId, before)
                patch(key, (v) => applyPage(v, page, true))
            } catch {
                /* the button stays; pressing it tries again */
            }
        },

        post: async (workspaceId, topicId, text) => {
            const key = topicKey(workspaceId, topicId)
            const result = await postEntry(workspaceId, topicId, text)
            // The socket delivers it to everyone, this tab included; folding it in here as well is
            // what keeps it on screen when the socket is down.
            for (const entry of [result.entry, result.note]) {
                if (entry) patch(key, (v) => applyFrame(v, { workspace_entry: entry }))
            }
            return result
        },

        retry: (workspaceId, topicId) => handles.get(topicKey(workspaceId, topicId))?.retryNow(),

        reset: () => {
            handles.forEach((h) => h.close())
            handles.clear()
            set({ list: [], loaded: false, topics: {} })
        },
    }
})
