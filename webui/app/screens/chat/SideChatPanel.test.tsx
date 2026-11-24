import { renderToString } from 'react-dom/server'
import { describe, expect, it } from 'vitest'
import { SideChatPanel } from './SideChatPanel'

/** The panel as the chat draws it, with one question in flight. */
const draw = () =>
    renderToString(
        <SideChatPanel
            open
            onClose={() => {}}
            agentId="oz"
            agentName="Archivist"
            channelId="c1"
            topicId="t1"
            question="What is the parser doing?"
        />
    )

describe('the side chat while it waits for an answer', () => {
    it('says what the agent is doing, as the activity trail does', () => {
        const html = draw()
        expect(html).toContain('Archivist is ')
        expect(html).not.toContain('Thinking…')
    })

    it('draws the same three dots as the trail, after the line', () => {
        const html = draw()
        expect(html.match(/rounded-pill animate-dot-wave/g) ?? []).toHaveLength(3)
        // The dots follow the line, as they do on a trail head.
        expect(html.indexOf('Archivist is ')).toBeLessThan(html.indexOf('animate-dot-wave'))
    })

    it('is a status, not just a line of text', () => {
        expect(draw()).toContain('role="status"')
    })
})
