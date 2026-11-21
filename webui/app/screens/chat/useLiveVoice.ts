/**
 * A conversation's live voice call (`specs/055-live-voice`): the client, its view, and the actions
 * the composer deck's `CallPill` and the composer's voice button need.
 *
 * The surface is design's: the call is a `CallPill` in the deck's lip and the composer's Send turns
 * into the voice icon while nothing is typed (drawn on "Composer · two panels"). This hook owns the
 * client and nothing else. There is no interrupt control: the way to stop the agent talking is to
 * speak (FR-020/FR-021 are deferred).
 */

import { useEffect, useRef, useState } from 'react'
import type { CallState } from '@vizier/design'
import { useConnection } from '../../stores/connection'
import { useTurns } from '../../stores/turns'
import { LiveVoiceClient, type LiveVoiceView, type UtteranceEvent } from '../../lib/liveVoice/session'
import { WebAudioPlayer } from '../../lib/liveVoice/player'
import { startCapture } from '../../lib/liveVoice/capture'
import { bindLiveVoice, endLiveVoice, followLiveVoiceConnection } from '../../lib/liveVoice/control'
import { callState } from '../../lib/liveVoice/words'

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
    onView: (view: LiveVoiceView) => void,
    onHeard: () => void,
    endOfUtteranceMs?: number
) =>
    new LiveVoiceClient({
        send: (frame) => useConnection.getState().send(agentId, topicId, frame),
        player: new WebAudioPlayer(),
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
    level: number
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

    // How many utterances this call sent: once the call listens again after one, the stored page
    // is read back so the drawn utterance gives way to what the server wrote.
    const heard = useRef(0)
    const onHeard = () => {
        heard.current += 1
    }

    // Built on first use, never during render: an `AudioContext` is a resource, and a render that
    // is thrown away (StrictMode) must not create one.
    useEffect(() => {
        if (!topicId) return
        const bound = (client.current ??= makeClient(agentId, topicId, setView, onHeard, endOfUtteranceMs))
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
        void (client.current ??= makeClient(agentId, topicId, setView, onHeard, endOfUtteranceMs)).start()
    }

    const state = callState(view.state)
    const over = state === 'ended' || state === 'failed'
    return {
        state: dismissed && over ? null : state,
        elapsedSeconds: Math.floor(view.elapsedMs / 1000),
        level: view.level,
        reason: over ? (view.notice ?? undefined) : undefined,
        start,
        end: () => client.current?.stop('pressed_again'),
        dismiss: () => setDismissed(true),
    }
}
