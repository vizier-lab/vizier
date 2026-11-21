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
    it('draws the tool row left to right: attach, slash, mic, speaker, context, then mode, Send', () => {
        const html = draw()
        // Document order is the drawn order: the row is one flex line.
        const at = [
            'Attach files',
            'Skills',
            'Record a voice message',
            'Ask for a spoken reply',
            'Context used',
            'Plan or Build',
            'Send',
        ].map((label) => html.indexOf(`aria-label="${label}"`))
        expect(at.every((i) => i > -1)).toBe(true)
        expect(at).toEqual([...at].sort((a, b) => a - b))
    })

    it('draws one row only: no hint under the box', () => {
        expect(draw()).not.toContain('Applies to the next message')
    })

    it('offers no mode switch outside a project', () => {
        expect(draw(false)).not.toContain('aria-label="Plan or Build"')
    })
})
