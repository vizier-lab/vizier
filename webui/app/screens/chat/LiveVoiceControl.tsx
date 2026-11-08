/**
 * The live voice controls, mounted next to the composer's own (`specs/055-live-voice` FR-001,
 * FR-020, FR-042).
 *
 * It owns the client and nothing else: the button, the status bar and the interrupt all read one
 * view, so the state word can never disagree with the audio.
 */

import { useEffect, useRef, useState } from 'react'
import { useConnection } from '../../stores/connection'
import { LiveVoiceClient, type LiveVoiceView } from '../../lib/liveVoice/session'
import { WebAudioPlayer } from '../../lib/liveVoice/player'
import { startCapture } from '../../lib/liveVoice/capture'
import { bindLiveVoice } from '../../lib/liveVoice/control'
import { LiveVoiceBar } from './LiveVoiceBar'
import { LiveVoiceButton } from './LiveVoiceButton'

export interface LiveVoiceControlProps {
    agentId: string
    topicId: string
    /** The agent detail's `live_voice`; the control is offered only when this is true. */
    capability: boolean
    /** Which half is missing, when the capability is false (FR-004). */
    missing?: 'recognizer' | 'voice' | null
    onOpenSettings?(): void
}

const OFF: LiveVoiceView = {
    state: 'off',
    thinking: false,
    thinkingMs: 0,
    level: 0,
    transcript: '',
    notice: null,
    micOpen: false,
}

export function LiveVoiceControl({ agentId, topicId, capability, missing, onOpenSettings }: LiveVoiceControlProps) {
    const [view, setView] = useState<LiveVoiceView>(OFF)
    const client = useRef<LiveVoiceClient | null>(null)

    // The client is built once per open conversation; the session it drives lives on the server.
    if (!client.current) {
        client.current = new LiveVoiceClient({
            send: (frame) => useConnection.getState().send(agentId, topicId, frame),
            player: new WebAudioPlayer(),
            openCapture: startCapture,
            onView: setView,
        })
    }

    useEffect(() => {
        const bound = client.current
        if (!bound) return
        const unbind = bindLiveVoice(agentId, topicId, bound, setView)
        return () => {
            // Leaving the conversation ends the session and releases the microphone (FR-028).
            bound.stop('left_conversation')
            unbind()
        }
    }, [agentId, topicId])

    // A closed tab or a backgrounded desktop window ends the session with a reason.
    useEffect(() => {
        const onHide = () => client.current?.stop('left_conversation')
        window.addEventListener('pagehide', onHide)
        return () => window.removeEventListener('pagehide', onHide)
    }, [])

    return (
        <div className="lv-controls">
            <LiveVoiceButton
                capability={capability}
                state={view.state}
                missing={missing}
                onOpenSettings={onOpenSettings}
                onStart={() => void client.current?.start()}
                onStop={() => client.current?.stop('pressed_again')}
            />
            <LiveVoiceBar view={view} onInterrupt={() => client.current?.interrupt('button')} />
        </div>
    )
}
