// The text the agent learns the components from: generated from the catalogue that validates and
// draws them, so it cannot drift (spec FR-003a). Returned by `ui_components`.
import type { z } from 'zod'
import { ACTIONS, ACTIVE_GROUPS, catalog, COMPONENT_NAMES, GROUPS, inGroup, type ComponentDef, type Group } from './catalog/index'
import { FUNCTION_NAMES } from './functions'
import { nearest } from './suggest'
import { RECIPES, RECIPE_NAMES } from './recipes/index'

export const MAX_REFERENCE_BYTES = 24 * 1024

type Def = { type: string; [key: string]: unknown }
const defOf = (schema: unknown): Def => (schema as { _zod: { def: Def } })._zod.def

/** A compact type for a prop: `"a" | "b"`, `string`, `number[]`, `{label: string, value: number}`. */
export function describeType(schema: unknown, depth = 0): string {
    const def = defOf(schema)
    switch (def.type) {
        case 'optional':
        case 'default':
        case 'nullable':
            return describeType(def.innerType, depth)
        case 'string':
        case 'number':
        case 'boolean':
            return def.type
        case 'enum':
            return Object.values(def.entries as Record<string, string>)
                .map((v) => JSON.stringify(v))
                .join(' | ')
        case 'literal':
            return (def.values as unknown[]).map((v) => JSON.stringify(v)).join(' | ')
        case 'array':
            return `${wrapUnion(describeType(def.element, depth + 1))}[]`
        case 'tuple':
            return `[${(def.items as unknown[]).map((item) => describeType(item, depth + 1)).join(', ')}]`
        case 'union':
            return (def.options as unknown[]).map((option) => describeType(option, depth + 1)).join(' | ')
        case 'record':
            return `{[key: string]: ${describeType(def.valueType, depth + 1)}}`
        case 'object': {
            if (depth > 2) return 'object'
            const shape = def.shape as Record<string, unknown>
            return `{${Object.entries(shape)
                .map(([key, value]) => `${key}${isOptional(value) ? '?' : ''}: ${describeType(value, depth + 1)}`)
                .join(', ')}}`
        }
        default:
            return 'any'
    }
}

const wrapUnion = (type: string) => (type.includes(' | ') ? `(${type})` : type)

function isOptional(schema: unknown): boolean {
    return (schema as z.ZodType).safeParse(undefined).success
}

function note(schema: unknown): string {
    const text = (schema as { description?: string }).description
    return text ? ` — ${text}` : ''
}

const childrenText = { none: 'none', any: 'any', three_d: '3D parts only' } as const

/** The element map for a component's example: the element itself, with a child when it takes one. */
function exampleElements(def: ComponentDef): Record<string, unknown> {
    const id = def.name.toLowerCase()
    const element: Record<string, unknown> = { type: def.name, props: def.example.props }
    if (def.example.children) {
        element.children = ['child']
        return {
            [id]: element,
            child: def.children === 'three_d' ? { type: 'Box', props: { size: [1, 1, 1] } } : { type: 'Text', props: { text: 'Hello' } },
        }
    }
    return { [id]: element }
}

export function summaryOf(def: ComponentDef): string {
    const first = def.description.split(/(?<=\.)\s/)[0]
    return first
}

export function describeComponent(def: ComponentDef) {
    const props = Object.fromEntries(
        Object.entries(def.props.shape).map(([key, schema]) => [key, `${describeType(schema)} (${isOptional(schema) ? 'optional' : 'required'})${note(schema)}`])
    )
    return {
        name: def.name,
        group: def.group,
        summary: summaryOf(def),
        description: def.description,
        props,
        children: childrenText[def.children],
        events: def.events,
        example: exampleElements(def),
    }
}

export type ComponentsQuery = { names?: unknown; group?: unknown; recipe?: unknown }

const indexOf = (group?: Group) => ({
    components: (group ? inGroup(group) : ACTIVE_GROUPS.flatMap((g) => inGroup(g))).map((def) => ({ name: def.name, group: def.group, summary: summaryOf(def) })),
    ...(group ? {} : { actions: ACTIONS, functions: FUNCTION_NAMES, recipes: RECIPE_NAMES }),
    next: 'ui_components(names=[…]) for props; ui_components(recipe=…) for full examples',
})

/** What `ui_components` answers. Never throws: an unknown name or group is data for the agent. */
export function answerComponents(query: ComponentsQuery = {}): Record<string, unknown> {
    const result = build(query)
    const bytes = new TextEncoder().encode(JSON.stringify(result)).length
    if (bytes <= MAX_REFERENCE_BYTES) return result
    // Only a list of names can be this large; answer with the first that fit.
    const names = (Array.isArray(query.names) ? query.names : []) as string[]
    return build({ names: names.slice(0, Math.max(1, Math.floor(names.length / 2))) })
}

function build(query: ComponentsQuery): Record<string, unknown> {
    if (typeof query.recipe === 'string') {
        const recipe = RECIPES[query.recipe]
        if (!recipe) {
            const near = nearest(query.recipe, RECIPE_NAMES)
            return { error: `no recipe "${query.recipe}"`, recipes: RECIPE_NAMES, did_you_mean: near }
        }
        return { recipe: query.recipe, intent: recipe.intent, notes: recipe.notes, spec: recipe.spec }
    }
    if (Array.isArray(query.names)) {
        const names = query.names.filter((n): n is string => typeof n === 'string')
        const found = names.filter((name) => catalog[name])
        return {
            components: found.map((name) => describeComponent(catalog[name])),
            unknown: names.filter((name) => !catalog[name]).map((name) => ({ name, did_you_mean: nearest(name, COMPONENT_NAMES) })),
        }
    }
    if (typeof query.group === 'string') {
        if (!(GROUPS as readonly string[]).includes(query.group) || !ACTIVE_GROUPS.includes(query.group as Group)) {
            return { error: `no group "${query.group}"`, groups: ACTIVE_GROUPS, did_you_mean: nearest(query.group, ACTIVE_GROUPS) }
        }
        return indexOf(query.group as Group)
    }
    return indexOf()
}
