import { renderToString } from 'react-dom/server'
import { describe, expect, it } from 'vitest'
import { Composer } from './Composer'

/** A project chat with an agent that can speak, drawn as the chat draws it. */
const draw = (project = true) =>
    renderToString(
        <Composer
            agentId="oz"
            agentName="Archivist"
            sttEnabled
            ttsEnabled
            busy={false}
            context={{ used: 1000, window: 200000 }}
            onSend={() => {}}
            onSendAudio={() => {}}
            onCommand={() => {}}
            onStop={() => {}}
            project={project ? { id: 'p1', mode: 'build', shell: true } : undefined}
        />
    )

describe("the composer's own controls", () => {
    it('puts the Plan/Build switch in the tool row, left of Send', () => {
        const html = draw()
        const mode = html.indexOf('aria-label="Plan or Build"')
        expect(mode).toBeGreaterThan(-1)
        expect(mode).toBeLessThan(html.indexOf('aria-label="Send"'))
    })

    it('puts the spoken-reply toggle after the tools, before the mode switch', () => {
        const html = draw()
        const spoken = html.indexOf('aria-label="Ask for a spoken reply"')
        expect(spoken).toBeGreaterThan(html.indexOf('aria-label="Skills"'))
        expect(spoken).toBeLessThan(html.indexOf('aria-label="Plan or Build"'))
    })

    it('draws one row only: no hint under the box', () => {
        expect(draw()).not.toContain('Applies to the next message')
    })

    it('offers no mode switch outside a project', () => {
        expect(draw(false)).not.toContain('aria-label="Plan or Build"')
    })
})
