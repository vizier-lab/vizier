/**
 * The live voice wire types and their codec (`specs/055-live-voice/contracts/live-voice-frames.md`).
 *
 * Pure: no DOM, no socket, no timers. The session module owns all of that, so the codec and the
 * state machine can be unit-tested without a browser.
 */

/** The one sample rate this release sends and accepts. */
export const SAMPLE_RATE = 16_000
/** The one format this release sends and accepts. */
export const FORMAT = 'pcm16'
/** One client chunk: 100 ms of PCM16 mono. */
export const CHUNK_SAMPLES = SAMPLE_RATE / 10
/** The top-level key every server frame carries, so an older client ignores it. */
export const FRAME_KEY = 'live_voice'

/** FR-006, one for one: the client never invents a state. */
export type SessionState =
    | 'off'
    | 'connecting'
    | 'listening'
    | 'waiting'
    | 'speaking'
    | 'interrupted'
    | 'reconnecting'
    | 'ended'
    | 'failed'

export type EndReason =
    | 'pressed_again'
    | 'left_conversation'
    | 'page_closed'
    | 'permission_withdrawn'
    | 'conversation_deleted'
    | 'agent_changed'
    | 'idle'
    | 'connection_lost'
    | 'server_restart'
    | 'no_device'
    | 'no_recognizer'
    | 'no_voice'
    | 'provider_failed'
    | 'unintelligible'
    | 'unsupported_audio'
    | 'key_revoked'
    | 'key_expired'
    | 'moved'

/** A control frame: the only family that arrives on the session socket's broadcast. */
export type ControlFrame =
    | { type: 'state'; state: SessionState; generation: number; reason?: EndReason | null }
    | { type: 'transcript'; utterance_id: string; text: string; submitted: boolean }
    | { type: 'error'; kind: EndReason; message: string }
    | { type: 'stall'; dropped: number }

/** One segment of the reply, written per connection and never broadcast. */
export interface SpeechFrame {
    type: 'speech'
    generation: number
    seq: number
    sample_rate: number
    format: string
    base64: string
    final: boolean
}

/** What arrived under the `live_voice` key. */
export type ServerFrame = ControlFrame | SpeechFrame

/** Whether a parsed socket message is a live voice frame, and which. */
export function decodeServerFrame(raw: unknown): ServerFrame | null {
    if (!raw || typeof raw !== 'object') return null
    const body = (raw as Record<string, unknown>)[FRAME_KEY]
    if (!body || typeof body !== 'object') return null
    const frame = body as Record<string, unknown>
    switch (frame.type) {
        case 'state':
            return {
                type: 'state',
                state: frame.state as SessionState,
                generation: Number(frame.generation ?? 0),
                reason: (frame.reason as EndReason | null | undefined) ?? null,
            }
        case 'transcript':
            return {
                type: 'transcript',
                utterance_id: String(frame.utterance_id ?? ''),
                text: String(frame.text ?? ''),
                submitted: Boolean(frame.submitted),
            }
        case 'error':
            return { type: 'error', kind: frame.kind as EndReason, message: String(frame.message ?? '') }
        case 'stall':
            return { type: 'stall', dropped: Number(frame.dropped ?? 0) }
        case 'speech':
            return {
                type: 'speech',
                generation: Number(frame.generation ?? 0),
                seq: Number(frame.seq ?? 0),
                sample_rate: Number(frame.sample_rate ?? SAMPLE_RATE),
                format: String(frame.format ?? FORMAT),
                base64: String(frame.base64 ?? ''),
                final: Boolean(frame.final),
            }
        default:
            return null
    }
}

/** The six actions a client may send, exactly as the server recognises them. */
export function startFrame(language?: string): Record<string, unknown> {
    return { live_voice_start: language ? { language } : {} }
}

export function audioFrame(utteranceId: string, seq: number, base64: string): Record<string, unknown> {
    return {
        live_voice_audio: {
            utterance_id: utteranceId,
            seq,
            sample_rate: SAMPLE_RATE,
            format: FORMAT,
            base64,
        },
    }
}

export function utteranceEndFrame(utteranceId: string): Record<string, unknown> {
    return { live_voice_utterance_end: { utterance_id: utteranceId } }
}

/** The barge-in signal: sent the moment the VAD hears the person, before any audio (FR-019). */
export function speechStartFrame(utteranceId: string): Record<string, unknown> {
    return { live_voice_speech_start: { utterance_id: utteranceId } }
}

export function interruptFrame(reason: 'button' | 'typed' | 'barge_in'): Record<string, unknown> {
    return { live_voice_interrupt: { reason } }
}

export function stopFrame(): Record<string, unknown> {
    return { live_voice_stop: {} }
}

/** PCM16 little-endian samples to base64, chunk by chunk. */
export function encodePcm16Base64(samples: Int16Array): string {
    const bytes = new Uint8Array(samples.length * 2)
    for (let i = 0; i < samples.length; i++) {
        const s = samples[i]
        bytes[i * 2] = s & 0xff
        bytes[i * 2 + 1] = (s >> 8) & 0xff
    }
    let binary = ''
    // `String.fromCharCode(...bytes)` blows the argument limit on a 3.2 KB chunk in some engines.
    for (let i = 0; i < bytes.length; i += 0x8000) {
        binary += String.fromCharCode(...bytes.subarray(i, i + 0x8000))
    }
    return btoa(binary)
}

/** base64 back to float samples in [-1, 1], for playback. */
export function decodePcm16Base64(base64: string): Float32Array {
    const binary = atob(base64)
    const samples = new Float32Array(Math.floor(binary.length / 2))
    for (let i = 0; i < samples.length; i++) {
        const lo = binary.charCodeAt(i * 2)
        const hi = binary.charCodeAt(i * 2 + 1)
        const value = (hi << 8) | lo
        samples[i] = (value >= 0x8000 ? value - 0x10000 : value) / 0x8000
    }
    return samples
}

/** Downmix to mono and resample to 16 kHz, the only shape the server accepts. */
export function toMono16k(input: Float32Array, inputRate: number): Float32Array {
    if (inputRate === SAMPLE_RATE) return input
    const ratio = inputRate / SAMPLE_RATE
    const out = new Float32Array(Math.floor(input.length / ratio))
    for (let i = 0; i < out.length; i++) {
        out[i] = input[Math.floor(i * ratio)]
    }
    return out
}

export function floatToPcm16(samples: Float32Array): Int16Array {
    const out = new Int16Array(samples.length)
    for (let i = 0; i < samples.length; i++) {
        const s = Math.max(-1, Math.min(1, samples[i]))
        out[i] = s < 0 ? s * 0x8000 : s * 0x7fff
    }
    return out
}
