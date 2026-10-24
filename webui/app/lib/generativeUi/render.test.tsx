import { createStateStore } from '@json-render/core'
import { JSONUIProvider, Renderer } from '@json-render/react'
import { renderToString } from 'react-dom/server'
import { describe, expect, it } from 'vitest'
import { COMPUTED_FUNCTIONS } from './functions'
import { RECIPES } from './recipes/index'
import { guarded } from './registry/guard'
import { registry } from './registry/index'

function draw(name: string) {
    const spec = RECIPES[name].spec
    const store = createStateStore((spec.state ?? {}) as Record<string, unknown>)
    return renderToString(
        <JSONUIProvider registry={guarded(registry)} store={store} functions={COMPUTED_FUNCTIONS}>
            <Renderer spec={spec} registry={guarded(registry)} />
        </JSONUIProvider>
    )
}

describe('drawing the recipes', () => {
    it('draws the dashboard', () => {
        const html = draw('dashboard')
        expect(html).toContain('Weekly report')
        expect(html).toContain('1,284')
        expect(html).toContain('Median latency')
        expect(html).toContain('Vizier')
        expect(html).not.toContain('could not be drawn')
    })

    it('draws the form with its inputs and only the check button', () => {
        const html = draw('form')
        expect(html).toContain('Request access')
        expect(html).toContain('Email')
        expect(html).toContain('Check')
        expect(html).not.toContain('>Send<')
        expect(html).not.toContain('could not be drawn')
    })

    it('draws the first tab\'s panel and hides the other', () => {
        const html = draw('tabs_and_filter')
        expect(html).toContain('Draft the report')
        expect(html).toContain('Water the plants')
        expect(html).not.toContain('Tabs and filters change what is shown')
        expect(html).not.toContain('could not be drawn')
    })

    it('draws the board as 64 pressable cells with their pieces', () => {
        const html = draw('board_game')
        expect(html.match(/<button/g)?.length).toBe(64 + 1)
        expect(html).toContain('♔')
        expect(html).toContain('White')
        expect(html).toContain('Ask the agent for a move')
        expect(html).not.toContain('could not be drawn')
    })

    it('draws the 2D canvas with its shapes', () => {
        const html = draw('canvas_2d')
        expect(html).toContain('<svg')
        expect(html).toContain('<circle')
        expect(html).toContain('viewBox="0 0 200 100"')
        expect(html).not.toContain('could not be drawn')
    })

    it('draws a link with the host beside a label that hides it, and refuses a bad scheme in text', () => {
        const spec = {
            root: 'a',
            elements: {
                a: { type: 'Stack', props: {}, children: ['l', 'm', 'i'] },
                l: { type: 'Link', props: { text: 'paypal.com', href: 'https://evil.test/x' } },
                m: { type: 'Text', props: { text: 'bad [click](javascript:alert(1)) good [ok](https://example.com)' } },
                i: { type: 'Image', props: { file: 'gone.png', alt: 'gone' } },
            },
        }
        const store = createStateStore({})
        const html = renderToString(
            <JSONUIProvider registry={guarded(registry)} store={store} functions={COMPUTED_FUNCTIONS}>
                <Renderer spec={spec as never} registry={guarded(registry)} />
            </JSONUIProvider>
        ).replace(/<!-- -->/g, '')
        expect(html).toContain('href="https://evil.test/x"')
        expect(html).toContain('rel="noopener noreferrer nofollow"')
        expect(html).toContain('(evil.test)')
        expect(html).not.toContain('javascript:')
        expect(html).toContain('href="https://example.com/"')
        expect(html).toContain('File not available: gone.png')
    })
})
