import { renderToString } from 'react-dom/server'
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest'
import type { ModelSummary, ProviderSummary } from '../../api/endpoints/catalogue'

const catalogue = vi.hoisted(() => ({
    providers: vi.fn(),
    models: vi.fn(),
}))
vi.mock('../../lib/catalogue', () => ({
    loadProviders: catalogue.providers,
    loadModels: catalogue.models,
    logoOf: () => ({}),
}))

import { ModelOverridePicker } from './ModelOverridePicker'

const provider = (id: string, patch: Partial<ProviderSummary> = {}): ProviderSummary => ({
    id,
    name: id,
    origin: 'catalogue',
    kind: 'cloud',
    dialect: 'openai_chat',
    logo: null,
    logo_full_color: false,
    model_count: 1,
    connection: { status: 'connected', source: 'saved' },
    popular: false,
    fields: [],
    ...patch,
})
const model = (id: string): ModelSummary => ({
    id,
    name: id,
    default: false,
    limit: {},
    caps: {},
    status: 'active',
})

beforeEach(() => {
    catalogue.providers.mockResolvedValue([provider('opencode')])
    catalogue.models.mockResolvedValue([model('deepseek/deepseek-flash')])
})
afterEach(() => vi.clearAllMocks())

describe('ModelOverridePicker', () => {
    it('renders exactly one control, labelled Agent default before a choice is verified', () => {
        const html = renderToString(<ModelOverridePicker value={{ provider: 'opencode', model: 'deepseek/deepseek-flash' }} onChange={() => {}} />)
        expect(html.match(/aria-haspopup=\"dialog\"/g)).toHaveLength(1)
        expect(html).toContain('Chat model: Agent default')
    })

    it('shows the default until the catalogue has settled the saved choice', () => {
        // The catalogue loads in an effect, which a server render never runs: on the first paint
        // the selector must not name a model it has not verified yet.
        const html = renderToString(<ModelOverridePicker value={{ provider: 'opencode', model: 'stale/model' }} onChange={() => {}} />)
        expect(html).toContain('Chat model: Agent default')
    })

    it('shows Agent default when nothing is selected', () => {
        const html = renderToString(<ModelOverridePicker value={null} onChange={() => {}} />)
        expect(html).toContain('Chat model: Agent default')
        expect(html.match(/aria-haspopup=\"dialog\"/g)).toHaveLength(1)
    })
})
