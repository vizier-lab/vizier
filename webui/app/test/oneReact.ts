// The design library has its own node_modules, so CommonJS code under it (lucide-react) would
// `require('react')` a second copy and fail on hooks when a test draws design components. This
// points those requires at the app's React, as `resolve.dedupe` does for the bundler.
import { createRequire } from 'node:module'

const require = createRequire(import.meta.url)
// eslint-disable-next-line @typescript-eslint/no-explicit-any
const Module = require('node:module') as any

const original = Module._resolveFilename
const mine = ['react', 'react-dom']

Module._resolveFilename = function (request: string, ...rest: unknown[]) {
    const root = request.split('/')[0]
    if (mine.includes(root)) return original.call(this, request, { id: import.meta.url, filename: import.meta.url.replace('file://', ''), paths: Module._nodeModulePaths(process.cwd()) }, ...rest.slice(1))
    return original.call(this, request, ...rest)
}
