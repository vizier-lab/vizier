/**
 * Records the microphone as a 16 kHz mono 16-bit WAV.
 *
 * `MediaRecorder` can only produce what the browser picks (Opus in WebM or Ogg, AAC in MP4),
 * and the local Whisper model reads PCM. Capturing PCM ourselves gives every browser the same
 * file, and WAV is the one format the server decodes without a codec.
 */

export const WAV_SAMPLE_RATE = 16_000

/** Collects the input's first channel and posts each 128-frame block to the page. */
const WORKLET_SOURCE = `
class Capture extends AudioWorkletProcessor {
    process(inputs) {
        const channel = inputs[0] && inputs[0][0]
        if (channel) this.port.postMessage(channel.slice())
        return true
    }
}
registerProcessor('vizier-capture', Capture)
`

export function encodeWav(samples: Float32Array, sampleRate: number): Blob {
    const buffer = new ArrayBuffer(44 + samples.length * 2)
    const view = new DataView(buffer)
    const text = (offset: number, value: string) => {
        for (let i = 0; i < value.length; i++) {
            view.setUint8(offset + i, value.charCodeAt(i))
        }
    }

    text(0, 'RIFF')
    view.setUint32(4, 36 + samples.length * 2, true)
    text(8, 'WAVE')
    text(12, 'fmt ')
    view.setUint32(16, 16, true)
    view.setUint16(20, 1, true) // PCM
    view.setUint16(22, 1, true) // mono
    view.setUint32(24, sampleRate, true)
    view.setUint32(28, sampleRate * 2, true)
    view.setUint16(32, 2, true)
    view.setUint16(34, 16, true)
    text(36, 'data')
    view.setUint32(40, samples.length * 2, true)

    for (let i = 0; i < samples.length; i++) {
        const s = Math.max(-1, Math.min(1, samples[i]))
        view.setInt16(44 + i * 2, s < 0 ? s * 0x8000 : s * 0x7fff, true)
    }
    return new Blob([buffer], { type: 'audio/wav' })
}

export class WavRecorder {
    state: 'recording' | 'inactive' = 'recording'
    onstop: ((blob: Blob) => void) | null = null

    private constructor(
        private readonly stream: MediaStream,
        private readonly context: AudioContext,
        private readonly node: AudioWorkletNode,
        private readonly blocks: Float32Array[]
    ) {}

    static async start(stream: MediaStream): Promise<WavRecorder> {
        // The context resamples the microphone to 16 kHz for us.
        const context = new AudioContext({ sampleRate: WAV_SAMPLE_RATE })
        const url = URL.createObjectURL(
            new Blob([WORKLET_SOURCE], { type: 'text/javascript' })
        )
        try {
            await context.audioWorklet.addModule(url)
        } finally {
            URL.revokeObjectURL(url)
        }

        const blocks: Float32Array[] = []
        const node = new AudioWorkletNode(context, 'vizier-capture')
        node.port.onmessage = (e: MessageEvent<Float32Array>) =>
            blocks.push(e.data)

        context.createMediaStreamSource(stream).connect(node)
        // A worklet only runs while it reaches the destination; it outputs silence.
        node.connect(context.destination)
        return new WavRecorder(stream, context, node, blocks)
    }

    /** Stops without producing a recording: the microphone is released and `onstop` never fires. */
    cancel() {
        if (this.state !== 'recording') return
        this.state = 'inactive'
        this.node.disconnect()
        this.stream.getTracks().forEach((t) => t.stop())
        void this.context.close()
    }

    /** Stops and returns the recording (also handed to `onstop`). */
    stop(): Blob | null {
        if (this.state !== 'recording') return null
        this.state = 'inactive'
        this.node.disconnect()
        this.stream.getTracks().forEach((t) => t.stop())

        const length = this.blocks.reduce((n, b) => n + b.length, 0)
        const samples = new Float32Array(length)
        let at = 0
        for (const block of this.blocks) {
            samples.set(block, at)
            at += block.length
        }

        const rate = this.context.sampleRate
        void this.context.close()
        const blob = encodeWav(samples, rate)
        this.onstop?.(blob)
        return blob
    }
}
