/**
 * The microphone, cut into the chunks the contract sends (`specs/055-live-voice` C5, FR-038).
 *
 * The same worklet the recorder already uses (`/vizier-capture-worklet.js`), so nothing new is
 * loaded and the desktop app's CSP keeps working: a `blob:` worklet would be refused there. Every
 * chunk is 100 ms of PCM16 mono at 16 kHz.
 */

import { CHUNK_SAMPLES, SAMPLE_RATE, floatToPcm16, toMono16k } from './frames'
import { WORKLET_URL, WAV_SAMPLE_RATE } from '../wavRecorder'

export interface Capture {
    /** Stop the microphone and release it: the OS indicator goes out here. */
    stop(): void
    /** Whether the microphone is still open. */
    get open(): boolean
}

export interface CaptureOptions {
    /** Called with one 100 ms chunk of PCM16 samples, in order. */
    onChunk(samples: Int16Array): void
    /** Called for every analysis window, so the VAD and the level meter see the same audio. */
    onWindow(samples: Float32Array): void
    /** The device was taken away, or permission was withdrawn (FR-030). */
    onLost(reason: string): void
}

/**
 * Echo cancellation, noise suppression and gain control are all on: the agent's own voice arrives
 * through the speakers, and the first defence against hearing itself is the browser's canceller
 * (`plan.md` risk R4). The VAD's reply guard is the second.
 */
const DEVICE_CONSTRAINTS: MediaStreamConstraints = {
    audio: { echoCancellation: true, noiseSuppression: true, autoGainControl: true, channelCount: 1 },
    video: false,
}

/** Open the microphone and start emitting chunks. Throws with a reason a person can act on. */
export async function startCapture(options: CaptureOptions): Promise<Capture> {
    let stream: MediaStream
    try {
        stream = await navigator.mediaDevices.getUserMedia(DEVICE_CONSTRAINTS)
    } catch (e) {
        throw new Error(deviceReason(e))
    }

    // The context resamples to 16 kHz for us; the worklet hands over float blocks.
    const context = new AudioContext({ sampleRate: WAV_SAMPLE_RATE })
    await context.audioWorklet.addModule(WORKLET_URL)
    const node = new AudioWorkletNode(context, 'vizier-capture')
    const source = context.createMediaStreamSource(stream)

    let tail = new Float32Array(0)
    node.port.onmessage = (event: MessageEvent<Float32Array>) => {
        const block = toMono16k(event.data, context.sampleRate)
        options.onWindow(block)
        // Re-chunk into exactly 100 ms, because a worklet block is 128 frames at whatever the
        // device runs at, and a chunk has to be one chunk of the contract.
        const joined = new Float32Array(tail.length + block.length)
        joined.set(tail)
        joined.set(block, tail.length)
        let offset = 0
        while (offset + CHUNK_SAMPLES <= joined.length) {
            options.onChunk(floatToPcm16(joined.subarray(offset, offset + CHUNK_SAMPLES)))
            offset += CHUNK_SAMPLES
        }
        tail = joined.slice(offset)
    }

    source.connect(node)
    // A worklet only runs while it reaches the destination; it outputs silence.
    node.connect(context.destination)

    let open = true
    const release = () => {
        if (!open) return
        open = false
        node.port.onmessage = null
        node.disconnect()
        source.disconnect()
        for (const track of stream.getTracks()) track.stop()
        void context.close()
    }

    for (const track of stream.getAudioTracks()) {
        track.onended = () => {
            release()
            options.onLost('the microphone was taken away')
        }
    }
    context.onstatechange = () => {
        if (context.state === 'suspended' && open) {
            // A locked screen or an unfocused desktop window suspends the context; the session
            // ends with a reason rather than looking live (T061).
            options.onLost('the audio context was suspended')
        }
    }

    return {
        stop: release,
        get open() {
            return open
        },
    }
}

/** FR-030: a reason the person can act on, not a raw DOMException name. */
export function deviceReason(error: unknown): string {
    const name = (error as { name?: string } | null)?.name ?? ''
    switch (name) {
        case 'NotAllowedError':
        case 'SecurityError':
            return 'the microphone was refused; allow it in the browser and try again'
        case 'NotFoundError':
        case 'OverconstrainedError':
            return 'no microphone was found'
        case 'NotReadableError':
            return 'the microphone is in use by another application'
        default:
            return 'the microphone could not be opened'
    }
}

export const CAPTURE_SAMPLE_RATE = SAMPLE_RATE
