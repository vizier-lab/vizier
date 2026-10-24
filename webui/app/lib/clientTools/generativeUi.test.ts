import { describe, expect, it } from 'vitest'
import { COMPONENT_NAMES } from '../generativeUi/catalog/index'
import { UI_CATEGORY_DESCRIPTION, UI_COMPONENTS_DESCRIPTION, UI_RENDER_DESCRIPTION, uiComponentsSchema, uiRenderSchema } from './generativeUi'

const bytes = (text: string) => new TextEncoder().encode(text).length

// The server's own limits (client-tools protocol §1) plus room for the ends-turn notice it adds.
const NOTICE_BYTES = 200

describe('generative UI tool definitions', () => {
    it('keeps ui_render under the description limit with room for the turn-ending notice', () => {
        expect(bytes(UI_RENDER_DESCRIPTION)).toBeLessThanOrEqual(2048 - NOTICE_BYTES)
    })

    it('keeps the other descriptions short', () => {
        expect(bytes(UI_CATEGORY_DESCRIPTION)).toBeLessThanOrEqual(512)
        expect(bytes(UI_COMPONENTS_DESCRIPTION)).toBeLessThanOrEqual(2048)
    })

    it('keeps the schemas under 16 KB', () => {
        expect(bytes(JSON.stringify(uiRenderSchema()))).toBeLessThanOrEqual(16 * 1024)
        expect(bytes(JSON.stringify(uiComponentsSchema()))).toBeLessThanOrEqual(16 * 1024)
    })

    it('generates the type enum from the catalogue', () => {
        const schema = uiRenderSchema() as { properties: { spec: { properties: { elements: { additionalProperties: { properties: { type: { enum: string[] } } } } } } } }
        expect(schema.properties.spec.properties.elements.additionalProperties.properties.type.enum).toEqual(COMPONENT_NAMES)
    })

    it('names the discovery path in the descriptions', () => {
        expect(UI_RENDER_DESCRIPTION).toContain('ui_components')
        expect(UI_CATEGORY_DESCRIPTION).toContain('Start with ui_components')
    })
})
