// A logic runner for tests: the real worker script, booted in a Node `vm`, behind the host's
// `Transport`. Calls go through exactly the code a browser frame runs, minus the frame.
import vm from 'node:vm'
import { WORKER_SOURCE } from './runnerSource'
import type { FromRunner, Transport } from './host'

export function vmTransport(): Transport {
    let toHost: (m: FromRunner) => void = () => {}
    const listeners: ((e: { data: unknown }) => void)[] = []
    const self = { postMessage: (m: FromRunner) => queueMicrotask(() => toHost(m)), addEventListener: (_: string, l: (e: { data: unknown }) => void) => listeners.push(l) }
    vm.runInContext(WORKER_SOURCE, vm.createContext({ self, Proxy, JSON, Object, Array, Function, String, Error, Math }))
    return {
        send: (message) => queueMicrotask(() => listeners.forEach((l) => l({ data: JSON.parse(JSON.stringify(message)) }))),
        onMessage: (l) => {
            toHost = l
            // The runner said hello before anyone listened.
            queueMicrotask(() => l({ type: 'hello' }))
        },
        destroy: () => {},
    }
}
