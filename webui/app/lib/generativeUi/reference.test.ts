import { describe, expect, it } from 'vitest'
import { ACTIVE_GROUPS, catalog, COMPONENT_NAMES } from './catalog/index'
import { answerComponents, describeComponent, describeType, MAX_REFERENCE_BYTES } from './reference'

const bytes = (value: unknown) => new TextEncoder().encode(JSON.stringify(value)).length

describe('ui_components', () => {
    it('indexes every component, the actions, functions and recipes', () => {
        const index = answerComponents() as { components: { name: string }[]; actions: unknown[]; functions: string[]; recipes: string[] }
        expect(index.components.map((c) => c.name).sort()).toEqual([...COMPONENT_NAMES].sort())
        expect(index.actions.length).toBe(6)
        expect(index.functions).toContain('mod')
        expect(index.recipes).toContain('dashboard')
        expect(bytes(index)).toBeLessThan(8 * 1024)
    })

    it('narrows the index to a group, and names the groups when it is unknown', () => {
        const layout = answerComponents({ group: 'layout' }) as { components: { group: string }[] }
        expect(layout.components.every((c) => c.group === 'layout')).toBe(true)
        const bad = answerComponents({ group: 'layot' }) as { groups: string[]; did_you_mean: string[] }
        expect(bad.groups).toEqual(ACTIVE_GROUPS)
        expect(bad.did_you_mean).toContain('layout')
    })

    it('describes props from the schema, with types, requirement and notes', () => {
        const detail = describeComponent(catalog.Grid)
        expect(detail.props.cols).toBe('number (required) — columns, 1–12')
        expect(detail.props.gap).toContain('"none" | "sm" | "md" | "lg" (optional)')
        expect(detail.children).toBe('any')
        expect(describeComponent(catalog.Button).events).toEqual(['press'])
        expect(describeComponent(catalog.Table).props.columns).toContain('{key: string, label: string')
    })

    it('shows a valid-looking example element, with a child when it takes one', () => {
        const stack = describeComponent(catalog.Stack).example as Record<string, { children?: string[] }>
        expect(stack.stack.children).toEqual(['child'])
        expect(Object.keys(describeComponent(catalog.Divider).example)).toEqual(['divider'])
    })

    it('answers names in detail and flags unknown ones with suggestions', () => {
        const out = answerComponents({ names: ['Table', 'Chrt'] }) as { components: { name: string }[]; unknown: { name: string; did_you_mean: string[] }[] }
        expect(out.components.map((c) => c.name)).toEqual(['Table'])
        expect(out.unknown[0].name).toBe('Chrt'); expect(out.unknown[0].did_you_mean[0]).toBe('Chart')
    })

    it('returns a recipe, or lists the recipes for an unknown one', () => {
        const recipe = answerComponents({ recipe: 'dashboard' }) as { spec: { root: string }; intent: string }
        expect(recipe.spec.root).toBe('page')
        expect(recipe.intent).not.toBe('')
        expect((answerComponents({ recipe: 'nope' }) as { recipes: string[] }).recipes).toContain('dashboard')
    })

    it('stays under the size cap even for every name at once', () => {
        expect(bytes(answerComponents({ names: COMPONENT_NAMES }))).toBeLessThanOrEqual(MAX_REFERENCE_BYTES)
    })

    it('describes nested types compactly', () => {
        expect(describeType(catalog.Tabs.props.shape.tabs)).toContain('{id: string, label: string}[]')
    })
})
