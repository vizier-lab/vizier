import { renderToString } from 'react-dom/server'
import { MemoryRouter } from 'react-router'
import { describe, expect, it, vi } from 'vitest'

const mode = vi.hoisted(() => ({ authMode: 'http' as 'http' | 'desktop', pickFolder: vi.fn() }))
vi.mock('../../api/transport', () => ({ transport: mode }))

import { OpenProjectDialog } from './OpenProjectDialog'

describe('opening a project from the browser', () => {
    it('offers no creation action and never posts', () => {
        mode.authMode = 'http'
        const html = renderToString(
            <MemoryRouter>
                <OpenProjectDialog agentId="dp-a" open onClose={() => {}} />
            </MemoryRouter>
        )
        expect(html).toContain('The browser cannot open a project')
        expect(html).not.toContain('Open project')
        expect(mode.pickFolder).not.toHaveBeenCalled()
    })
})
