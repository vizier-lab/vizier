// `vizier.worker` (specs/063-html-generative-ui contracts/bridge.md, FR-016a): a page's heavy
// work on its own thread, with a time limit, so a long computation never stalls the page (or,
// where the frame shares it, the WebUI's thread). Part of the frame runtime (`runtime.ts`).

const WORKER_TIMEOUT_MS = 10_000
const WORKER_TIMEOUT_MAX_MS = 60_000

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
