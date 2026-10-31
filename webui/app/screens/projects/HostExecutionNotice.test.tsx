import { renderToString } from 'react-dom/server'
import { describe, expect, it, vi } from 'vitest'

const mode = vi.hoisted(() => ({ authMode: 'http' as 'http' | 'desktop' }))
vi.mock('../../api/transport', () => ({ transport: mode }))

import { HostExecutionNotice } from './HostExecutionNotice'

describe('the host execution notice', () => {
    it('is shown to a browser session (FR-023)', () => {
        mode.authMode = 'http'
        const html = renderToString(<HostExecutionNotice />)
        expect(html).toContain('Commands run on the host machine')
    })

    it('is absent in the desktop app, which is the host', () => {
        mode.authMode = 'desktop'
        expect(renderToString(<HostExecutionNotice />)).toBe('')
    })
})
