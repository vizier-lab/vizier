import { renderToString } from 'react-dom/server'
import { beforeEach, describe, expect, it, vi } from 'vitest'
import type { SensitiveAsk } from '../../api/frames'

// The design `Dialog` is the one place Escape, an outside press and the close control all land: it
// calls `onCancel`. Capturing what it is handed proves each of them reaches the dialog's decline.
const captured: {
    props?: { onCancel: () => void; onConfirm?: () => void; confirmLabel?: string; title: string; children?: unknown }
} = {}
vi.mock('@vizier/design', async (importOriginal) => ({
    ...(await importOriginal<typeof import('@vizier/design')>()),
    // Draws its body so the dialog's own content can be read.
    Dialog: (props: { children?: unknown }) => {
        captured.props = props as never
        return props.children as never
    },
}))

import { SensitiveAskDialog } from './SensitiveAskDialog'

const ask: SensitiveAsk = {
    ask_id: 'a1',
    label: 'GitHub token',
    purpose: 'To file the issue you asked for.',
    protected: true,
    min_bytes: 8,
    max_bytes: 65536,
}

beforeEach(() => {
    captured.props = undefined
})

describe('the sensitive-input dialog', () => {
    it('declines on every way of closing it, so nothing is left waiting (FR-036)', () => {
        const answered: Array<[string, string | null]> = []
        renderToString(<SensitiveAskDialog ask={ask} onAnswer={(id, value) => answered.push([id, value])} />)
        // Escape, an outside press and the × all arrive as the dialog's cancel.
        captured.props!.onCancel()
        expect(answered).toEqual([['a1', null]])
    })

    it('has no confirm of its own in the header and names the ask', () => {
        renderToString(<SensitiveAskDialog ask={ask} onAnswer={() => {}} />)
        expect(captured.props?.title).toBe('GitHub token')
        expect(captured.props?.confirmLabel).toBe('')
    })

    it('sends nothing for an empty value: dismissal is how to decline', () => {
        const answered: unknown[] = []
        renderToString(<SensitiveAskDialog ask={ask} onAnswer={(...a) => answered.push(a)} />)
        captured.props!.onConfirm!()
        expect(answered).toEqual([])
    })

    it('is masked, never autocompleted and not offered to a password manager', () => {
        const html = renderToString(<SensitiveAskDialog ask={ask} onAnswer={() => {}} />)
        expect(html).toContain('type="password"')
        expect(html).toContain('autoComplete="off"')
        expect(html).toContain('data-1p-ignore')
        expect(html).toContain('data-lpignore="true"')
        // Reveal is an eye inside the field, named for a screen reader; it starts masked.
        expect(html).toContain('aria-label="Show value"')
        expect(html).not.toContain('aria-label="Hide value"')
        expect(html).not.toContain('>Reveal<')
        expect(html).toContain('The value goes to this run only')
        expect(html).toContain('To file the issue you asked for.')
        expect(html).toContain('never written to the conversation')
        // Nothing is prefilled.
        expect(html).not.toMatch(/value="[^"]+"/)
    })

    it('says before submission that a value this short cannot be scrubbed (FR-043)', () => {
        const html = renderToString(<SensitiveAskDialog ask={{ ...ask, protected: false }} onAnswer={() => {}} />)
        expect(html).toContain('shorter than 8 bytes cannot be scrubbed')
        expect(html).toContain('role="alert"')
        expect(renderToString(<SensitiveAskDialog ask={ask} onAnswer={() => {}} />)).not.toContain('cannot be scrubbed')
    })

    it('offers Send and Reject, and Reject declines like every other way out', () => {
        const html = renderToString(<SensitiveAskDialog ask={ask} onAnswer={() => {}} />)
        expect(html).toContain('Send')
        expect(html).toContain('Reject')
    })

    it('draws nothing when no ask is open', () => {
        expect(renderToString(<SensitiveAskDialog ask={null} onAnswer={() => {}} />)).toBe('')
        expect(captured.props).toBeUndefined()
    })
})
