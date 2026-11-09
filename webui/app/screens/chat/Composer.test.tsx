import { renderToString } from 'react-dom/server'
import { describe, expect, it } from 'vitest'
import { Composer } from './Composer'

/** A project chat with an agent that can speak, drawn as the chat draws it. */
const draw = (project = true) =>
    renderToString(
        <Composer
            agentId="oz"
            agentName="Archivist"
            topicId="t1"
            liveVoiceEnabled
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
    it('draws the tool row left to right: attach, slash, live voice, context, then mode, Send', () => {
        const html = draw()
        // Document order is the drawn order: the row is one flex line. Live voice sits where the
        // mic and the spoken-reply toggle used to be (spec 055 B1-1): those two are the manual
        // speech surfaces and are not drawn any more (FR-053/T107).
        const at = [
            'Attach files',
            'Skills',
            'Start live voice',
            'Context used',
            'Plan or Build',
            'Send',
        ].map((label) => html.indexOf(`aria-label="${label}"`))
        expect(at.every((i) => i > -1)).toBe(true)
        expect(at).toEqual([...at].sort((a, b) => a - b))
    })

    it('does not draw the manual speech surfaces, though the agent can record and speak (FR-053/T107)', () => {
        const html = draw()
        expect(html).not.toContain('Record a voice message')
        expect(html).not.toContain('Ask for a spoken reply')
    })

    it('offers no interrupt control: the only way to stop the agent talking is to speak (C7)', () => {
        expect(draw().toLowerCase()).not.toContain('interrupt')
    })

    it('draws one row only: no hint under the box', () => {
        expect(draw()).not.toContain('Applies to the next message')
    })

    it('offers no mode switch outside a project', () => {
        expect(draw(false)).not.toContain('aria-label="Plan or Build"')
    })
})
