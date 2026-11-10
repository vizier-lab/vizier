/**
 * The live session's control, as one row inside the composer's meta line (B1-1, FR-043).
 *
 * This component owns the client and nothing else; the row it returns is the whole surface. The
 * two pieces it used to render — `LiveVoiceButton` and `LiveVoiceBar` — are archived in place:
 * they are the record of the previous surface, and the handoff geometry still refers to them.
 * There is no interrupt control anywhere, because FR-020/FR-021 are deferred (C7): the only way
 * to stop the agent talking is to speak.
 */

import { useEffect, useRef, useState } from 'react'
import { useConnection } from '../../stores/connection'
import { useTurns } from '../../stores/turns'
import { LiveVoiceClient, type LiveVoiceView } from '../../lib/liveVoice/session'
import { WebAudioPlayer } from '../../lib/liveVoice/player'
import { startCapture } from '../../lib/liveVoice/capture'
import { bindLiveVoice, endLiveVoice, followLiveVoiceConnection } from '../../lib/liveVoice/control'
import { LiveVoiceRow } from './LiveVoiceRow'

export interface LiveVoiceControlProps {
    agentId: string
    topicId: string
    /** The agent's display name, for the row (FR-044). The id is not a name. */
    agentName?: string
    /** The agent detail's `live_voice`; the row is offered only when this is true. */
    capability: boolean
    /** Which half is missing, when the capability is false (FR-004). */
    missing?: 'recognizer' | 'voice' | null
    /** The agent's own hangover, in ms (G1). Absent leaves the spec's default. */
    endOfUtteranceMs?: number
    onOpenSettings?(): void
}

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

/** One client per open conversation; the session it drives lives on the server. */
const makeClient = (
    agentId: string,
    topicId: string,
    onView: (view: LiveVoiceView) => void,
    endOfUtteranceMs?: number
) =>
    new LiveVoiceClient({
        send: (frame) => useConnection.getState().send(agentId, topicId, frame),
        player: new WebAudioPlayer(),
        openCapture: startCapture,
        onView,
        endOfUtteranceMs,
    })

export function LiveVoiceControl({
    agentId,
    topicId,
    agentName,
    capability,
    missing,
    endOfUtteranceMs,
    onOpenSettings,
}: LiveVoiceControlProps) {
    const [view, setView] = useState<LiveVoiceView>(OFF)
    const client = useRef<LiveVoiceClient | null>(null)

    // Built on first use, never during render: an `AudioContext` is a resource, and a render that
    // is thrown away (StrictMode, a server render) must not create one. The row draws from `view`
    // alone, so an idle surface needs no client at all.
    useEffect(() => {
        const bound = (client.current ??= makeClient(agentId, topicId, setView, endOfUtteranceMs))
        bindLiveVoice(agentId, topicId, bound, setView)
        // Leaving the conversation ends the session and releases the microphone (FR-028). The
        // unmount and the page close below both land in `endLiveVoice`, so there is one leaving
        // path and not two (T090).
        return () => endLiveVoice(agentId, topicId, bound, 'left_conversation')
    }, [agentId, topicId])

    // FR-033: the socket's own state reaches the call. Nothing did this before — the transport
    // publishes `reconnecting` and `offline` into the connection store, and the call never
    // listened, so a drop left the bar saying `listening` for good while the server dropped every
    // frame it sent. The subscription lives as long as the surface does, and the current value is
    // forwarded on the way in for a call that starts on a socket that is already open.
    useEffect(() => followLiveVoiceConnection(agentId, topicId), [agentId, topicId])

    // A live voice turn is submitted by the server, so this tab never sends it and the thread used
    // to stay empty until a refresh read it back. The words are drawn as they are heard: the
    // transcript frame carries what was recognised, and the stored request replaces the stand-in
    // when it arrives (`coveredByStored`), so a later read leaves one entry and not two.
    const heard = useRef({ text: '', turns: 0 })
    useEffect(() => {
        const text = view.transcript.trim()
        if (!text || text === heard.current.text) return
        heard.current = { text, turns: heard.current.turns + 1 }
        useTurns.getState().adoptTurn(agentId, topicId, `${topicId}-${heard.current.turns}`, text)
    }, [view.transcript, agentId, topicId])

    // V9: once the words above have reached the thread, give the row's middle slot back to the
    // waveform. It is cleared here and not on the state change, because the transcript is still the
    // only thing on screen until `adoptTurn` has it — clearing first would erase the sentence a
    // person is reading, which is the bug this fixes, in the other direction.
    useEffect(() => {
        const text = view.transcript.trim()
        if (!text || text !== heard.current.text) return
        // Only once the call is listening again: while the reply is still being spoken the words
        // stay on screen, and the meter returns in the window a person actually asks about it.
        if (view.state !== 'listening') return
        client.current?.transcriptAdopted()
    }, [view.transcript, view.state])

    // The reply has been spoken and the session is listening again: read the stored page back, so
    // the stand-ins give way to what the server actually wrote (and nothing is drawn twice).
    useEffect(() => {
        if (view.state !== 'listening' || !heard.current.turns) return
        void useTurns.getState().refreshHistory(agentId, topicId)
    }, [view.state, agentId, topicId])

    // A closed tab or a backgrounded desktop window ends the session with a reason.
    useEffect(() => {
        const onHide = () => endLiveVoice(agentId, topicId, client.current, 'left_conversation')
        window.addEventListener('pagehide', onHide)
        return () => window.removeEventListener('pagehide', onHide)
    }, [agentId, topicId])

    // FR-004: a control that cannot start says which half is missing, where the settings are.
    const start = () => {
        if (!capability) {
            onOpenSettings?.()
            return
        }
        void (client.current ??= makeClient(agentId, topicId, setView, endOfUtteranceMs)).start()
    }

    return (
        <LiveVoiceRow
            view={view}
            agentName={agentName}
            capability={capability}
            missing={missing}
            elapsedSeconds={Math.floor(view.elapsedMs / 1000)}
            onStart={start}
            onEnd={() => client.current?.stop('pressed_again')}
            onRestart={start}
        />
    )
}
