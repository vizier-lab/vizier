// The runtime every agent page starts with (specs/063-html-generative-ui contracts/bridge.md).
//
// Inlined by the server into the frame document before the page's own scripts
// (vizier-http/src/ui_frame.rs), so `window.vizier` exists when they run. It talks to the host
// (screens/chat/client/SandboxedPage.tsx) only through `postMessage`: the frame has an opaque
// origin and no other way out. The host enforces every rule that matters (who may send, how
// often, how big the state is); nothing here is trusted.
//
// Messages are `{ vz: 1, type, ... }` both ways; see PROTOCOL below.

export const PROTOCOL = 1

type Problem = {
    kind: 'error' | 'rejection' | 'console' | 'blocked' | 'kit' | 'unresponsive'
    message: string
    source: 'page' | 'kit'
    line?: number
    column?: number
    detail?: string
}

type Boot = { pageId: string; version: number; state: unknown }

declare global {
    interface Window {
        __vizierBoot?: Boot
        vizier?: unknown
    }
}

/** How often the frame tells the host it is alive (the host reports `unresponsive` without it). */
export const HEARTBEAT_MS = 250
const MAX_TEXT = 500
const WORKER_TIMEOUT_MS = 10_000
const WORKER_TIMEOUT_MAX_MS = 60_000

const clip = (text: unknown) => String(text ?? '').slice(0, MAX_TEXT)

function post(type: string, body: Record<string, unknown> = {}) {
    try {
        parent.postMessage({ vz: PROTOCOL, type, ...body }, '*')
    } catch {
        // A value that cannot be cloned; the page gets nothing back for it.
    }
}

/** Report a problem to the host (verification and the error indicator). */
export function report(problem: Problem) {
    post('problem', { problem: { ...problem, message: clip(problem.message), detail: problem.detail && clip(problem.detail) } })
}

function installHooks() {
    addEventListener('error', (event) => {
        report({ kind: 'error', message: event.message || String(event.error), source: 'page', line: event.lineno, column: event.colno })
    })
    addEventListener('unhandledrejection', (event) => {
        const reason = event.reason as { message?: string } | undefined
        report({ kind: 'rejection', message: reason?.message ?? String(event.reason), source: 'page' })
    })
    addEventListener('securitypolicyviolation', (event) => {
        report({
            kind: 'blocked',
            message: `Blocked by the page policy: ${event.violatedDirective}`,
            source: 'page',
            detail: `${event.effectiveDirective} ${event.blockedURI}`,
        })
    })
    const original = console.error.bind(console)
    console.error = (...args: unknown[]) => {
        report({ kind: 'console', message: args.map((a) => (a instanceof Error ? a.message : typeof a === 'string' ? a : safeJson(a))).join(' '), source: 'page' })
        original(...args)
    }
}

function safeJson(value: unknown): string {
    try {
        return JSON.stringify(value)
    } catch {
        return String(value)
    }
}

/** Turn `<script type="text/plain" data-vz-lib>` blocks into blob URLs behind an import map, so
 * `import * as THREE from "three"` works with no network. Runs before the page's modules parse. */
function loadLibraries() {
    const blocks = document.querySelectorAll<HTMLScriptElement>('script[type="text/plain"][data-vz-lib]')
    if (!blocks.length) return
    const imports: Record<string, string> = {}
    for (const block of blocks) {
        const name = block.dataset.vzLib
        if (!name) continue
        imports[name] = URL.createObjectURL(new Blob([block.textContent ?? ''], { type: 'text/javascript' }))
        block.remove()
    }
    const map = document.createElement('script')
    map.type = 'importmap'
    map.textContent = JSON.stringify({ imports })
    document.head.appendChild(map)
}

/** The page's own `<html>` follows the WebUI's theme: the host posts the resolved tokens. */
function applyTheme(tokens: Record<string, string>, dark: boolean) {
    const root = document.documentElement
    for (const [name, value] of Object.entries(tokens)) {
        if (name.startsWith('--')) root.style.setProperty(name, value)
    }
    root.dataset.theme = dark ? 'dark' : 'light'
    root.style.colorScheme = dark ? 'dark' : 'light'
    dispatchEvent(new CustomEvent('vizier:theme', { detail: { dark } }))
}

type WorkerSource = ((input: unknown) => unknown) | string

/** `vizier.worker`: heavy work on its own thread, with a time limit (research R7). */
export function makeWorker(source: WorkerSource) {
    let body: string
    if (typeof source === 'function') {
        body = `const __run = (${source.toString()});`
    } else {
        const block = document.querySelector<HTMLScriptElement>(source)
        if (!block) throw new Error(`vizier.worker: no script matches ${source}`)
        body = `${block.textContent ?? ''}\nconst __run = (input) => self.run(input);`
    }
    const code = `${body}
self.onmessage = async (event) => {
  try { self.postMessage({ ok: true, value: await __run(event.data) }) }
  catch (error) { self.postMessage({ ok: false, error: String((error && error.message) || error) }) }
};`
    const url = URL.createObjectURL(new Blob([code], { type: 'text/javascript' }))
    let worker: Worker | null = null
    let queue: Promise<unknown> = Promise.resolve()

    const start = () => (worker ??= new Worker(url))
    const runOnce = (input: unknown, timeoutMs: number) =>
        new Promise((resolve, reject) => {
            const w = start()
            const timer = setTimeout(() => {
                w.terminate()
                worker = null
                reject(new Error('timeout'))
            }, timeoutMs)
            w.onmessage = (event: MessageEvent<{ ok: boolean; value?: unknown; error?: string }>) => {
                clearTimeout(timer)
                if (event.data.ok) resolve(event.data.value)
                else reject(new Error(event.data.error))
            }
            w.onerror = (event) => {
                clearTimeout(timer)
                event.preventDefault()
                reject(new Error(event.message || 'worker failed'))
            }
            w.postMessage(input)
        })

    return {
        run(input: unknown, options: { timeoutMs?: number } = {}) {
            const limit = Math.min(Math.max(1, options.timeoutMs ?? WORKER_TIMEOUT_MS), WORKER_TIMEOUT_MAX_MS)
            const next = queue.then(() => runOnce(input, limit))
            queue = next.catch(() => undefined)
            return next
        },
        terminate() {
            worker?.terminate()
            worker = null
        },
    }
}

function install() {
    const boot: Boot = window.__vizierBoot ?? { pageId: '', version: 0, state: null }
    let state: unknown = boot.state ?? null
    const listeners = new Set<(value: unknown) => void>()
    const pending = new Map<number, { resolve: (v: unknown) => void; reject: (e: Error) => void }>()
    let nextId = 1

    const ask = (type: string, body: Record<string, unknown>) =>
        new Promise((resolve, reject) => {
            const id = nextId++
            pending.set(id, { resolve, reject })
            post(type, { id, ...body })
        })

    addEventListener('message', (event: MessageEvent) => {
        if (event.source !== parent) return
        const data = event.data as { vz?: number; type?: string; [key: string]: unknown }
        if (!data || data.vz !== PROTOCOL) return
        switch (data.type) {
            case 'theme':
                applyTheme((data.tokens as Record<string, string>) ?? {}, Boolean(data.dark))
                break
            case 'state':
                state = data.value ?? null
                for (const fn of listeners) fn(state)
                break
            case 'reply': {
                const waiting = pending.get(data.id as number)
                if (!waiting) break
                pending.delete(data.id as number)
                if (data.ok) waiting.resolve(data.value)
                else waiting.reject(new Error(String(data.error ?? 'refused')))
                break
            }
        }
    })

    const abilities = ['state', 'send', 'files', 'worker'] as const
    const bridge = Object.freeze({
        version: 1,
        abilities,
        has: (ability: string) => (abilities as readonly string[]).includes(ability),
        page: Object.freeze({ id: boot.pageId, version: boot.version }),
        state: Object.freeze({
            get: () => state,
            set(value: unknown) {
                state = value
                post('state.set', { value })
            },
            subscribe(fn: (value: unknown) => void) {
                listeners.add(fn)
                return () => listeners.delete(fn)
            },
        }),
        send: (text: string, data?: unknown) => ask('send', { text: String(text), data }).then(() => undefined),
        files: Object.freeze({ url: (name: string) => ask('files.url', { name: String(name) }) as Promise<string> }),
        worker: makeWorker,
    })
    Object.defineProperty(window, 'vizier', { value: bridge, configurable: false, writable: false })

    installHooks()
    loadLibraries()

    // The host sizes the frame to its content and treats silence as a hang.
    const sendHeight = () => post('height', { px: Math.ceil(document.documentElement.scrollHeight) })
    new ResizeObserver(sendHeight).observe(document.documentElement)
    addEventListener('load', sendHeight)
    setInterval(() => post('heartbeat'), HEARTBEAT_MS)
    post('ready', { pageId: boot.pageId, version: boot.version })
}

install()
