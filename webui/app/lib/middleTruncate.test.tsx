// The component lives in design/, which has no unit-test runner: only Storybook stories render there
// (no Playwright browser is installed on this machine), so behaviour is pinned from the app side.
import { MiddleTruncate, middleTruncate } from '@vizier/design'
import { renderToString } from 'react-dom/server'
import { describe, expect, it } from 'vitest'

/** The classes both chat diff lists hand a changed path (ChangesPanel, ChangesDialog). */
const PATH_CLASS = 'min-w-0 flex-1 type-small text-soft'

/** A path as the diff list draws it. */
const draw = (text: string) => renderToString(<MiddleTruncate text={text} className={PATH_CLASS} />)

// The bug in the report: only the cut branch applied `className`, so a path of 40 characters or
// fewer drew at the body size, bright and wrapping, beside small grey one-line neighbours.
describe('a path drawn in the file list', () => {
    it('keeps the classes it was given when it fits', () => {
        const html = draw('crates/vizier-core/src/agents/project.rs')
        expect(html).toBe(`<span class="${PATH_CLASS}">crates/vizier-core/src/agents/project.rs</span>`)
    })

    it('is cut in the middle past the limit, still with those classes and the full path on hover', () => {
        const html = draw('crates/vizier-core/src/agents/agent/mod.rs')
        expect(html).toContain('crates/vizier-c…/src/agents/agent/mod.rs')
        expect(html).toContain('title="crates/vizier-core/src/agents/agent/mod.rs"')
        expect(html).toContain(`class="relative ${PATH_CLASS}"`)
    })

    it('carries no title when there is nothing to reveal', () => {
        expect(draw('CLAUDE.md')).not.toContain('title=')
    })
})

describe('middleTruncate', () => {
    it('leaves a text that fits whole, the limit included', () => {
        expect(middleTruncate('CLAUDE.md', 40)).toBe('CLAUDE.md')
        expect(middleTruncate(`${'a'.repeat(37)}.md`, 40)).toBe(`${'a'.repeat(37)}.md`)
    })

    it('cuts past the limit, keeping the start and the file name', () => {
        expect(middleTruncate('projects/2025-archive/q4-board-notes.md', 30)).toBe('projects/20…/q4-board-notes.md')
    })
})
