import { renderToString } from 'react-dom/server'
import { beforeEach, describe, expect, it, vi } from 'vitest'
import type { LspServer, LspView } from '../../api/endpoints/projects'

// renderToString reads a zustand store's initial state, so the panel is drawn against a plain stand-in.
const fake = vi.hoisted(() => ({
    state: { lsp: {} as Record<string, unknown>, watchLsp: () => () => {}, setLsp: () => {}, loadLsp: async () => {}, refresh: async () => {} },
}))
vi.mock('../../stores/projects', () => ({
    useProjects: Object.assign((select: (s: typeof fake.state) => unknown) => select(fake.state), {
        getState: () => fake.state,
    }),
}))

import { LspPanel } from './LspPanel'

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

const view = (over: Partial<LspView> = {}): LspView => ({
    project_id: 'p1',
    enabled: true,
    capacity: { in_use: 1, max: 6 },
    can_manage: true,
    servers: [],
    languages: [
        { id: 'rust', name: 'Rust', default_command: 'rust-analyzer' },
        { id: 'go', name: 'Go', default_command: 'gopls' },
    ],
    overrides: [],
    ...over,
})

const draw = (v?: LspView) => {
    fake.state.lsp = v ? { p1: v } : {}
    return renderToString(<LspPanel agentId="oz" projectId="p1" onClose={() => {}} />)
}

beforeEach(() => {
    fake.state.lsp = {}
})

describe('the language-server panel', () => {
    it('shows what is running, grouped by language, with the roots used', () => {
        const html = draw(
            view({
                servers: [
                    server({}),
                    server({ root: 'app' }),
                    server({ language: 'typescript', name: 'TypeScript', root: 'webui', state: 'starting', detail: 'indexing 41%' }),
                ],
            })
        )
        expect(html).toContain('Language servers')
        expect(html).toContain('2 ready · 1 starting')
        expect(html).toContain('./')
        expect(html).toContain('app/')
        expect(html).toContain('Starting · indexing 41%')
        expect(html).toContain('rust-analyzer')
    })

    it('explains a missing server once, with the install command and Check again', () => {
        const html = draw(
            view({
                servers: [
                    server({ language: 'go', name: 'Go', state: 'missing', install_hint: 'go install golang.org/x/tools/gopls@latest', reason: 'gopls was not found' }),
                ],
            })
        )
        expect(html).toContain("gopls isn&#x27;t installed")
        expect(html).toContain('go install golang.org/x/tools/gopls@latest')
        expect(html).toContain('Copy')
        expect(html).toContain('Check again')
    })

    it('offers a restart for a failed server and shows why', () => {
        const html = draw(view({ servers: [server({ state: 'failed', reason: 'Crashed 3 times: exit code 2' })] }))
        expect(html).toContain('Crashed 3 times: exit code 2')
        expect(html).toContain('Restart')
    })

    it('says so when nothing has started yet', () => {
        const html = draw(view())
        expect(html).toContain('No servers running')
        expect(html).toContain('Add override')
    })

    it('shows Off with no list when the switch is off', () => {
        const html = draw(view({ enabled: false }))
        expect(html).toContain('Off for this project')
        expect(html).toContain('role="switch"')
        expect(html).toContain('aria-checked="false"')
        expect(html).not.toContain('Add override')
    })

    it('gives someone who cannot manage no switch and no controls', () => {
        const html = draw(
            view({
                can_manage: false,
                languages: undefined,
                overrides: undefined,
                servers: [
                    server({}),
                    server({ language: 'go', name: 'Go', state: 'missing', install_hint: 'go install gopls' }),
                ],
            })
        )
        expect(html).toContain('Someone who manages this project controls these settings.')
        expect(html).not.toContain('role="switch"')
        expect(html).not.toContain('Configure')
        expect(html).not.toContain('Add override')
        expect(html).not.toContain('Check again')
        expect(html).not.toContain('Restart')
    })

    it('marks a language that runs an override', () => {
        const html = draw(
            view({
                servers: [server({ overridden: true })],
                overrides: [{ language: 'rust', command: '/opt/ra/rust-analyzer', args: [], env: {} }],
            })
        )
        expect(html).toContain('override')
        expect(html).toContain('/opt/ra/rust-analyzer')
    })

    it('carries no warning about running code', () => {
        const html = draw(view({ servers: [server({})] })).toLowerCase()
        expect(html).not.toContain('execute')
        expect(html).not.toContain('build script')
        expect(html).not.toContain('runs code')
    })
})
