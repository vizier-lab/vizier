// One WebSocket per open topic, with backoff and a connection state the UI can show.

import type { ConnectionState } from './common'

const FIRST_DELAY_MS = 1000
const MAX_DELAY_MS = 30_000
export const OFFLINE_AFTER = 3

/** 1 s, 2 s, 4 s … capped at 30 s. */
export function backoffDelay(failures: number): number {
    return Math.min(MAX_DELAY_MS, FIRST_DELAY_MS * 2 ** Math.max(0, failures - 1))
}

/** `reconnecting` for the first few failed attempts, `offline` after that (it keeps trying). */
export function stateAfterFailures(failures: number): ConnectionState {
    return failures >= OFFLINE_AFTER ? 'offline' : 'reconnecting'
}

export interface TopicSocketOptions {
    url: () => string | null
    onFrame: (data: unknown) => void
    onState: (state: ConnectionState) => void
    /** Fired on every open; `reopened` is true after a drop, when history must be re-read. */
    onOpen: (reopened: boolean) => void
}

export class TopicSocket {
    private ws: WebSocket | null = null
    private failures = 0
    private timer: ReturnType<typeof setTimeout> | null = null
    private closed = false
    private everOpened = false

    constructor(private opts: TopicSocketOptions) {
        this.connect()
    }

    private connect() {
        if (this.closed) return
        const url = this.opts.url()
        if (!url) return
        this.opts.onState(this.failures === 0 && !this.everOpened ? 'connecting' : stateAfterFailures(Math.max(1, this.failures)))
        const ws = new WebSocket(url)
        this.ws = ws

        ws.onopen = () => {
            if (this.ws !== ws) return
            const reopened = this.everOpened
            this.everOpened = true
            this.failures = 0
            this.opts.onState('open')
            this.opts.onOpen(reopened)
        }
        ws.onmessage = (event) => {
            if (this.ws !== ws) return
            try {
                this.opts.onFrame(JSON.parse(event.data as string))
            } catch {
                /* a frame that isn't JSON is ignored */
            }
        }
        ws.onclose = () => {
            if (this.ws !== ws || this.closed) return
            this.ws = null
            this.failures += 1
            this.opts.onState(stateAfterFailures(this.failures))
            this.timer = setTimeout(() => this.connect(), backoffDelay(this.failures))
        }
        ws.onerror = () => {
            /* onclose follows */
        }
    }

    get isOpen(): boolean {
        return this.ws?.readyState === WebSocket.OPEN
    }

    send(frame: object): boolean {
        if (!this.isOpen) return false
        this.ws!.send(JSON.stringify(frame))
        return true
    }

    /** Skips the wait and tries again now (the "Retry" in the Offline banner). */
    retryNow() {
        if (this.timer) clearTimeout(this.timer)
        this.timer = null
        if (!this.ws) this.connect()
    }

    close() {
        this.closed = true
        if (this.timer) clearTimeout(this.timer)
        const ws = this.ws
        this.ws = null
        ws?.close()
    }
}
