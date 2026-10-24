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
})
