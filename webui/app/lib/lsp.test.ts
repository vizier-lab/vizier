import { describe, expect, it } from 'vitest'
import type { LspServer } from '../api/endpoints/projects'
import {
    buildOverride,
    formatArgs,
    groupByLanguage,
    metaLine,
    parseArgs,
    parseEnv,
    readyCount,
    rootLabel,
    worstDot,
} from './lsp'

const server = (over: Partial<LspServer>): LspServer => ({
    language: 'rust',
    name: 'Rust',
    root: '.',
    state: 'ready',
    detail: null,
    reason: null,
    install_hint: null,
    overridden: false,
    ...over,
})

describe('the strip item', () => {
    it('counts ready servers and shows the worst state as the dot', () => {
        const ready = { enabled: true, servers: [server({}), server({ root: 'app' })] }
        expect(readyCount(ready)).toBe(2)
        expect(worstDot(ready)).toBeUndefined()

        const starting = { enabled: true, servers: [...ready.servers, server({ state: 'starting' })] }
        expect(worstDot(starting)).toBe('pulse')
        const missing = { enabled: true, servers: [...starting.servers, server({ language: 'go', state: 'missing' })] }
        expect(worstDot(missing)).toBe('warn')
        const failed = { enabled: true, servers: [...missing.servers, server({ state: 'failed' })] }
        expect(worstDot(failed)).toBe('fail')
    })

    it('has no dot when it is off', () => {
        expect(worstDot({ enabled: false, servers: [server({ state: 'failed' })] })).toBeUndefined()
    })
})

describe('the panel meta line', () => {
    it('lists what is going on and says none when nothing is', () => {
        expect(metaLine({ enabled: false, servers: [] })).toBe('off')
        expect(metaLine({ enabled: true, servers: [] })).toBe('none running')
        expect(
            metaLine({
                enabled: true,
                servers: [
                    server({}),
                    server({ root: 'app' }),
                    server({ language: 'typescript', state: 'starting' }),
                    server({ language: 'go', state: 'missing' }),
                    server({ language: 'go', root: 'x', state: 'missing' }),
                ],
            })
        ).toBe('2 ready · 1 starting · 1 missing')
    })
})

describe('grouping', () => {
    it('groups roots under their language in the order sent', () => {
        const groups = groupByLanguage({
            servers: [
                server({}),
                server({ language: 'typescript', name: 'TypeScript', root: 'webui' }),
                server({ root: 'app' }),
            ],
        })
        expect(groups.map((g) => g.id)).toEqual(['rust', 'typescript'])
        expect(groups[0].roots.map((r) => r.root)).toEqual(['.', 'app'])
    })

    it('shows one note for a language that is missing everywhere', () => {
        const [go] = groupByLanguage({
            servers: [
                server({ language: 'go', name: 'Go', state: 'missing', install_hint: 'go install gopls', reason: 'gopls was not found' }),
                server({ language: 'go', name: 'Go', root: 'x', state: 'missing', install_hint: 'go install gopls' }),
            ],
        })
        expect(go.missing?.install_hint).toBe('go install gopls')
        expect(go.roots).toEqual([])
    })

    it('keeps the roots when only some are missing, and reads the override command', () => {
        const [rust] = groupByLanguage({
            servers: [server({ overridden: true }), server({ root: 'x', state: 'missing' })],
            overrides: [{ language: 'rust', command: '/opt/ra', args: [], env: {} }],
            languages: [{ id: 'rust', name: 'Rust', default_command: 'rust-analyzer' }],
        })
        expect(rust.missing).toBeNull()
        expect(rust.roots).toHaveLength(2)
        expect(rust.overridden).toBe(true)
        expect(rust.command).toBe('/opt/ra')
    })

    it('falls back to the built-in command', () => {
        const [rust] = groupByLanguage({
            servers: [server({})],
            languages: [{ id: 'rust', name: 'Rust', default_command: 'rust-analyzer' }],
        })
        expect(rust.command).toBe('rust-analyzer')
        expect(rust.overridden).toBe(false)
    })
})

describe('roots', () => {
    it('reads the folder itself as ./', () => {
        expect(rootLabel('.')).toBe('./')
        expect(rootLabel('app')).toBe('app/')
        expect(rootLabel('.vizier/agent/fix')).toBe('.vizier/agent/fix/')
    })
})

describe('the override form', () => {
    it('parses NAME=value entries separated by lines or commas', () => {
        expect(parseEnv('RA_LOG=warn\nFOO=a=b,BAR=')).toEqual({ env: { RA_LOG: 'warn', FOO: 'a=b', BAR: '' } })
        expect(parseEnv('')).toEqual({ env: {} })
        expect(parseEnv('1BAD=x')).toHaveProperty('error')
        expect(parseEnv('novalue')).toHaveProperty('error')
    })

    it('splits arguments with quotes', () => {
        expect(parseArgs('--stdio --log "a b" \'c d\'')).toEqual(['--stdio', '--log', 'a b', 'c d'])
        expect(parseArgs('')).toEqual([])
        expect(parseArgs('""')).toEqual([''])
        expect(parseArgs(formatArgs(['--x', 'a b']))).toEqual(['--x', 'a b'])
    })

    it('builds an override or says what is wrong', () => {
        expect(buildOverride(' /opt/ra ', '-v', 'A=1')).toEqual({
            override: { command: '/opt/ra', args: ['-v'], env: { A: '1' } },
        })
        expect(buildOverride('', '', '')).toEqual({ error: 'Enter a command' })
        expect(buildOverride('x', '', 'bad')).toHaveProperty('error')
    })
})
