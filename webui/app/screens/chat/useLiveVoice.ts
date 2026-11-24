/**
 * A conversation's live voice call (`specs/055-live-voice`): the client, its view, and the actions
 * the composer's `CallRow` and its voice button need.
 *
 * The surface is design's: the call is a `CallRow` in the composer above the text, drawn like the
 * voice recorder, and the composer's Send turns into the voice icon while nothing is typed. This hook owns the
 * client and nothing else. There is no interrupt control: the way to stop the agent talking is to
 * speak (FR-020/FR-021 are deferred).
 */

import { useEffect, useRef, useState } from 'react'
import { create } from 'zustand'
import type { CallState } from '@vizier/design'
import { useConnection } from '../../stores/connection'
import { useTurns } from '../../stores/turns'
import { LiveVoiceClient, type LiveVoiceView, type UtteranceEvent } from '../../lib/liveVoice/session'
import { WebAudioPlayer } from '../../lib/liveVoice/player'
import { startCapture } from '../../lib/liveVoice/capture'
import { bindLiveVoice, endLiveVoice, followLiveVoiceConnection } from '../../lib/liveVoice/control'
import { callState } from '../../lib/liveVoice/words'

/** How often the call's level is sampled into its waveform: a bar every 60 ms, as the recorder. */
const LEVEL_EVERY_MS = 60
/** How many samples are kept: more bars than the widest composer holds. */
const MAX_LEVELS = 400
/** A reply's samples: ten minutes of speech, which is also how its time is counted. */
const MAX_REPLY_LEVELS = 10_000

/** How long the reply has been heard, from its samples. */
export const replySeconds = (reply: number[]) => (reply.length * LEVEL_EVERY_MS) / 1000

/**
 * The call's waveforms, apart from the call's view: they change every 60 ms, and only the call row
 * and the spoken reply's card draw them, so they are read there and nothing else re-renders. One
 * call per person at a time (FR-005), so one store per tab.
 */
export const useCallLevels = create<{
    /** The sound as it is heard, newest last: the microphone, or the reply while it is spoken. */
    levels: number[]
    /** The reply being spoken now: its own sound, newest last. Empty until it speaks. */
    reply: number[]
}>(() => ({ levels: [], reply: [] }))

const resetLevels = () => useCallLevels.setState({ levels: [], reply: [] })

const OFF: LiveVoiceView = {
    state: 'off',
    thinking: false,
    thinkingMs: 0,
    elapsedMs: 0,
    level: 0,
    transcript: '',
    notice: null,
    micOpen: false,
}

/**
 * Draw one utterance in the thread (spec 055): a voice message the moment its recording is kept,
 * its transcript when recognition has it, and gone again if it was never sent. The stored request
 * replaces the drawn one (`coveredByStored`), so a later read leaves one entry.
 */
const drawUtterance = (agentId: string, topicId: string, event: UtteranceEvent) => {
    const turns = useTurns.getState()
    switch (event.kind) {
        case 'heard':
            turns.adoptTurn(agentId, topicId, event.id, '', { filename: event.filename, content: { local: event.url } })
            break
        case 'transcribed':
            turns.adoptTurn(agentId, topicId, event.id, event.text)
            break
        case 'dropped':
            turns.dropAdopted(agentId, topicId, event.id)
            break
    }
}

/** One client per open conversation; the session it drives lives on the server. */
const makeClient = (
    agentId: string,
    topicId: string,
    player: WebAudioPlayer,
    onView: (view: LiveVoiceView) => void,
    onHeard: () => void,
    endOfUtteranceMs?: number
) =>
    new LiveVoiceClient({
        send: (frame) => useConnection.getState().send(agentId, topicId, frame),
        player,
        openCapture: startCapture,
        onView,
        onUtterance: (event) => {
            if (event.kind === 'transcribed') onHeard()
            drawUtterance(agentId, topicId, event)
        },
        endOfUtteranceMs,
    })

export type LiveVoiceCall = {
    /** What the pill draws; `null` while there is no call and no ending to show. */
    state: CallState | null
    elapsedSeconds: number
    /** The agent is speaking a reply now (its card plays along, from `useCallLevels`). */
    speaking: boolean
    /** Why it ended or failed, in a few words. */
    reason?: string
    start: () => void
    end: () => void
    dismiss: () => void
}

export function useLiveVoice({
    agentId,
    topicId,
    endOfUtteranceMs,
}: {
    agentId: string
    /** No topic yet (a conversation not opened): no call. */
    topicId: string | undefined
    /** The agent's own hangover, in ms (G1). Absent leaves the spec's default. */
    endOfUtteranceMs?: number
}): LiveVoiceCall {
    const [view, setView] = useState<LiveVoiceView>(OFF)
    const [dismissed, setDismissed] = useState(false)
    const client = useRef<LiveVoiceClient | null>(null)
    // Kept so the waveforms can read what the reply sounds like as it plays.
    const player = useRef<WebAudioPlayer | null>(null)
    const newClient = (topic: string) =>
        makeClient(agentId, topic, (player.current ??= new WebAudioPlayer()), setView, onHeard, endOfUtteranceMs)

    // How many utterances this call sent: once the call listens again after one, the stored page
    // is read back so the drawn utterance gives way to what the server wrote.
    const heard = useRef(0)
    const onHeard = () => {
        heard.current += 1
        // A new turn: the next reply's waveform starts empty.
        useCallLevels.setState({ reply: [] })
    }

    // Built on first use, never during render: an `AudioContext` is a resource, and a render that
    // is thrown away (StrictMode) must not create one.
    useEffect(() => {
        if (!topicId) return
        const bound = (client.current ??= newClient(topicId))
        bindLiveVoice(agentId, topicId, bound, setView)
        // Leaving the conversation ends the call and releases the microphone (FR-028). The unmount
        // and the page close below both land in `endLiveVoice`: one leaving path (T090).
        return () => {
            endLiveVoice(agentId, topicId, bound, 'left_conversation')
            client.current = null
            setView(OFF)
        }
    }, [agentId, topicId])

    // FR-033: the socket's own state reaches the call, so a dropped socket reads as reconnecting.
    useEffect(() => (topicId ? followLiveVoiceConnection(agentId, topicId) : undefined), [agentId, topicId])

    // The reply has been spoken and the call listens again: read the stored page back, so the
    // stand-ins give way to what the server wrote and nothing is drawn twice.
    useEffect(() => {
        if (!topicId || view.state !== 'listening' || !heard.current) return
        void useTurns.getState().refreshHistory(agentId, topicId)
    }, [view.state, agentId, topicId])

    // Closing the tab ends the call with a reason. Only `pagehide`: a hidden window keeps the
    // microphone (FR-038).
    useEffect(() => {
        if (!topicId) return
        const onHide = () => endLiveVoice(agentId, topicId, client.current, 'left_conversation')
        window.addEventListener('pagehide', onHide)
        return () => window.removeEventListener('pagehide', onHide)
    }, [agentId, topicId])

    const start = () => {
        if (!topicId) return
        setDismissed(false)
        resetLevels()
        void (client.current ??= newClient(topicId)).start()
    }

    // The waveform: the view's level, sampled at a steady pace while there is sound to draw. A
    // view changes on every analysis window and on everything else too, so it is sampled rather
    // than appended on change.
    // While the agent speaks the level is the reply's, read from the player; otherwise it is the
    // microphone's. The reply's own samples are kept apart too, for its card in the thread.
    const level = useRef(0)
    level.current = view.level
    const speaking = view.state === 'speaking'
    const sounding = view.state === 'listening' || view.state === 'waiting' || speaking
    useEffect(() => {
        if (!sounding) return
        const id = setInterval(() => {
            const now = speaking ? (player.current?.level() ?? 0) : level.current
            useCallLevels.setState(({ levels, reply }) => ({
                levels: [...levels.slice(-(MAX_LEVELS - 1)), now],
                reply: speaking ? [...reply.slice(-(MAX_REPLY_LEVELS - 1)), now] : reply,
            }))
        }, LEVEL_EVERY_MS)
        return () => clearInterval(id)
    }, [sounding, speaking])

    const state = callState(view.state)
    const over = state === 'ended' || state === 'failed'
    return {
        state: dismissed && over ? null : state,
        elapsedSeconds: Math.floor(view.elapsedMs / 1000),
        speaking,
        reason: over ? (view.notice ?? undefined) : undefined,
        start,
        end: () => client.current?.stop('pressed_again'),
        dismiss: () => setDismissed(true),
    }
}
