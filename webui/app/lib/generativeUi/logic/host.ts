// The host side of the logic sandbox: one runner per interface, calls one at a time in order,
// each held to 250 ms (contracts/logic-sandbox.md §3). The frame itself is behind `Transport`, so
// the queue and the limits are tested without a browser.
import { RUNNER_DOCUMENT } from './runnerSource'

export const CALL_TIMEOUT_MS = 250
export const MAX_QUEUE = 16
export const MAX_BYTES = 256 * 1024

export type LogicDef = { params: string[]; body: string }
export type Defs = Record<string, LogicDef>

/** A message from the runner. */
export type FromRunner = { type: 'hello' } | { type: 'result'; id: number; ok: true; json: string } | { type: 'result'; id: number; ok: false; error: string }

/** The connection to one runner: send to it, hear from it, destroy it. */
export interface Transport {
    send(message: unknown): void
    onMessage(listener: (message: FromRunner) => void): void
    destroy(): void
}

export class LogicError extends Error {
    constructor(
        readonly code: 'logic_timeout' | 'logic_busy' | 'logic_error' | 'logic_result_too_large' | 'logic_args_too_large' | 'logic_disposed',
        message: string
    ) {
        super(message)
    }
}

type Call = { id: number; fn: string; args: unknown[]; resolve: (value: unknown) => void; reject: (error: Error) => void }

const byteLength = (text: string) => new TextEncoder().encode(text).length

export class LogicHost {
    private transport: Transport | null = null
    private ready = false
    private queue: Call[] = []
    private current: (Call & { timer: ReturnType<typeof setTimeout> }) | null = null
    private nextId = 1
    private disposed = false

    constructor(
        private readonly defs: Defs,
        private readonly connect: () => Transport,
        private readonly timeoutMs = CALL_TIMEOUT_MS
    ) {}

    /** Call `fn` with JSON `args`; resolves with the JSON result. */
    call(fn: string, args: unknown[]): Promise<unknown> {
        if (this.disposed) return Promise.reject(new LogicError('logic_disposed', 'the interface is closed'))
        let size = 0
        try {
            size = byteLength(JSON.stringify(args) ?? 'null')
        } catch {
            return Promise.reject(new LogicError('logic_error', 'the arguments are not JSON'))
        }
        if (size > MAX_BYTES) return Promise.reject(new LogicError('logic_args_too_large', `the arguments are over ${MAX_BYTES / 1024} KB`))
        if (this.queue.length >= MAX_QUEUE) return Promise.reject(new LogicError('logic_busy', 'too many calls waiting'))
        return new Promise((resolve, reject) => {
            this.queue.push({ id: this.nextId++, fn, args, resolve, reject })
            this.pump()
        })
    }

    /** Stop the runner; the next call starts a new one. Pending calls are rejected. */
    dispose() {
        this.disposed = true
        this.stop()
        const error = new LogicError('logic_disposed', 'the interface is closed')
        this.current && (clearTimeout(this.current.timer), this.current.reject(error))
        this.current = null
        for (const call of this.queue.splice(0)) call.reject(error)
    }

    /** Drop an idle runner to free its frame; the next call recreates it. */
    release() {
        if (this.current || this.queue.length) return
        this.stop()
    }

    private stop() {
        this.transport?.destroy()
        this.transport = null
        this.ready = false
    }

    private start() {
        const transport = this.connect()
        this.transport = transport
        transport.onMessage((message) => {
            if (this.transport !== transport) return
            if (message.type === 'hello') {
                transport.send({ type: 'init', defs: this.defs })
                this.ready = true
                this.pump()
            } else if (message.type === 'result') this.finish(message)
        })
    }

    private pump() {
        if (this.disposed || this.current || this.queue.length === 0) return
        if (!this.transport) this.start()
        if (!this.ready) return
        const call = this.queue.shift()!
        const timer = setTimeout(() => this.timedOut(call.id), this.timeoutMs)
        this.current = { ...call, timer }
        this.transport!.send({ type: 'call', id: call.id, fn: call.fn, args: call.args })
    }

    private timedOut(id: number) {
        const current = this.current
        if (!current || current.id !== id) return
        this.current = null
        // The only way to stop a loop in a frame is to destroy it.
        this.stop()
        current.reject(new LogicError('logic_timeout', `${current.fn} took longer than ${this.timeoutMs} ms`))
        this.pump()
    }

    private finish(message: Extract<FromRunner, { type: 'result' }>) {
        const current = this.current
        if (!current || current.id !== message.id) return
        clearTimeout(current.timer)
        this.current = null
        if (!message.ok) current.reject(new LogicError('logic_error', message.error))
        else if (byteLength(message.json) > MAX_BYTES) current.reject(new LogicError('logic_result_too_large', `the result is over ${MAX_BYTES / 1024} KB`))
        else {
            try {
                current.resolve(JSON.parse(message.json))
            } catch {
                current.reject(new LogicError('logic_error', 'the result is not JSON'))
            }
        }
        this.pump()
    }
}

/** A runner in a sandboxed frame: an opaque origin, no network (the document's CSP). */
export function iframeTransport(): Transport {
    const frame = document.createElement('iframe')
    frame.setAttribute('sandbox', 'allow-scripts')
    frame.setAttribute('referrerpolicy', 'no-referrer')
    frame.setAttribute('aria-hidden', 'true')
    frame.tabIndex = -1
    frame.title = 'Interface logic'
    frame.style.cssText = 'position:fixed;width:0;height:0;border:0;visibility:hidden'
    frame.srcdoc = RUNNER_DOCUMENT
    let listener: ((message: FromRunner) => void) | null = null
    const onWindowMessage = (event: MessageEvent) => {
        // Only this frame is believed.
        if (event.source !== frame.contentWindow) return
        const data = event.data
        if (data && typeof data === 'object' && (data.type === 'hello' || data.type === 'result')) listener?.(data as FromRunner)
    }
    window.addEventListener('message', onWindowMessage)
    document.body.appendChild(frame)
    return {
        send: (message) => frame.contentWindow?.postMessage(message, '*'),
        onMessage: (l) => {
            listener = l
        },
        destroy: () => {
            window.removeEventListener('message', onWindowMessage)
            listener = null
            frame.remove()
        },
    }
}
