// A logic runner for tests: the real runner script, booted in a Node `vm`, behind the host's
// `Transport`. Calls go through exactly the code a browser frame runs, minus the frame.
import vm from 'node:vm'
import { RUNNER_SOURCE } from './runnerSource'
import type { FromRunner, Transport } from './host'

export function vmTransport(): Transport {
    let toHost: (m: FromRunner) => void = () => {}
    const listeners: ((e: { source: unknown; data: unknown }) => void)[] = []
    const parent = { postMessage: (m: FromRunner) => queueMicrotask(() => toHost(m)) }
    const window = { parent, addEventListener: (_: string, l: (e: { source: unknown; data: unknown }) => void) => listeners.push(l) }
    vm.runInContext(RUNNER_SOURCE, vm.createContext({ window, Proxy, JSON, Object, Array, Function, String, Error, Math }))
    return {
        send: (message) => queueMicrotask(() => listeners.forEach((l) => l({ source: parent, data: JSON.parse(JSON.stringify(message)) }))),
        onMessage: (l) => {
            toHost = l
            // The runner said hello before anyone listened.
            queueMicrotask(() => l({ type: 'hello' }))
        },
        destroy: () => {},
    }
}
