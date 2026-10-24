import { describe, expect, it } from 'vitest'
import { formatIssues, validateUi } from './validate'

const card = (extra: Record<string, unknown> = {}) => ({
    title: 'T',
    spec: { root: 'a', elements: { a: { type: 'Card', props: { title: 'Hi' }, children: [] }, ...extra } },
})
const messages = (args: unknown) => {
    const result = validateUi(args)
    if (result.ok) return []
    return result.issues.map((i) => `${i.path}: ${i.message}`)
}

describe('validateUi', () => {
    it('accepts a valid interface', () => {
        expect(validateUi(card()).ok).toBe(true)
    })

    it('names an unknown component with the valid ones and a suggestion', () => {
        const out = messages({ title: 'T', spec: { root: 'a', elements: { a: { type: 'Chrt', props: {} } } } })
        expect(out[0]).toContain('elements.a.type: "Chrt" is not a component. Valid: Stack')
        expect(out[0]).toContain('Did you mean "Chart"?')
    })

    it('rejects a wrong prop type', () => {
        const out = messages({ title: 'T', spec: { root: 'a', elements: { a: { type: 'Grid', props: { cols: 'three' } } } } })
        expect(out.join('\n')).toMatch(/elements\.a\.props\.cols: must be number/)
    })

    it('lists the valid options for an enum miss', () => {
        const out = messages({ title: 'T', spec: { root: 'a', elements: { a: { type: 'Button', props: { label: 'x', variant: 'big' } } } } })
        expect(out[0]).toBe('elements.a.props.variant: "big" is not allowed. Valid: primary, secondary, ghost, danger.')
    })

    it('reports a missing required prop and an unknown prop with a suggestion', () => {
        const out = messages({ title: 'T', spec: { root: 'a', elements: { a: { type: 'Heading', props: { txt: 'x' } } } } })
        expect(out).toContain('elements.a.props.text: is required')
        expect(out.some((m) => m.includes('no prop "txt"') && m.includes('Did you mean "text"?'))).toBe(true)
    })

    it('reports a dangling child', () => {
        const out = messages(card({ b: { type: 'Stack', props: {}, children: ['ghost'] } }) && { title: 'T', spec: { root: 'b', elements: { b: { type: 'Stack', props: {}, children: ['ghost'] } } } })
        expect(out.join('\n')).toMatch(/ghost/)
    })

    it('moves a misplaced visible out of props and says so', () => {
        const result = validateUi({ title: 'T', spec: { root: 'a', elements: { a: { type: 'Stack', props: { visible: { $state: '/x' } }, children: [] } } } })
        expect(result.ok).toBe(true)
        if (result.ok) {
            expect(result.fixed.length).toBe(1)
            expect(result.spec.elements.a.visible).toEqual({ $state: '/x' })
        }
    })

    it('lets expressions stand for values but checks $computed names and paths', () => {
        expect(validateUi({ title: 'T', spec: { root: 'a', elements: { a: { type: 'Grid', props: { cols: { $state: '/n' } } } } } }).ok).toBe(true)
        const out = messages({ title: 'T', spec: { root: 'a', elements: { a: { type: 'Text', props: { text: { $computed: 'ad', args: {} } } } } } })
        expect(out[0]).toContain('"ad" is not a function')
        expect(out[0]).toContain('Did you mean "add"?')
        expect(messages({ title: 'T', spec: { root: 'a', elements: { a: { type: 'Text', props: { text: { $state: 'x' } } } } } })[0]).toContain('state path starting with "/"')
    })

    it('checks events and actions, including run against logic', () => {
        const bad = messages({
            title: 'T',
            spec: { root: 'a', elements: { a: { type: 'Button', props: { label: 'x' }, on: { click: { action: 'run', params: { fn: 'go' } }, press: { action: 'setStat', params: {} } } } } },
        })
        expect(bad.some((m) => m.includes('does not emit "click"') && m.includes('Events: press.'))).toBe(true)
        expect(bad.some((m) => m.includes('"setStat" is not an action'))).toBe(true)
        const noFn = messages({ title: 'T', spec: { root: 'a', elements: { a: { type: 'Button', props: { label: 'x' }, on: { press: { action: 'run', params: { fn: 'go' } } } } } } })
        expect(noFn[0]).toContain('"go" is not in "logic"')
        expect(validateUi({ title: 'T', spec: { root: 'a', logic: { go: { params: [], body: 'return 1' } }, elements: { a: { type: 'Button', props: { label: 'x' }, on: { press: { action: 'run', params: { fn: 'go' } } } } } } }).ok).toBe(true)
    })

    it('refuses children on a leaf and too much logic', () => {
        expect(messages({ title: 'T', spec: { root: 'a', elements: { a: { type: 'Text', props: { text: 'x' }, children: ['b'] }, b: { type: 'Divider', props: {} } } } })[0]).toContain('cannot have children')
        const logic = Object.fromEntries(Array.from({ length: 40 }, (_, i) => [`f${i}`, { params: [], body: 'return 1' }]))
        expect(messages({ title: 'T', spec: { root: 'a', logic, elements: { a: { type: 'Divider', props: {} } } } }).join('\n')).toContain('40 functions')
    })

    it('refuses an oversize, too-many-element or too-deep interface', () => {
        const big = { title: 'T', spec: { root: 'a', elements: { a: { type: 'Text', props: { text: 'x'.repeat(300 * 1024) } } } } }
        expect(messages(big)[0]).toMatch(/spec: is 3\d\d KB; the limit is 256 KB/)
        const many = Object.fromEntries(Array.from({ length: 1501 }, (_, i) => [`e${i}`, { type: 'Divider', props: {} }]))
        expect(messages({ title: 'T', spec: { root: 'e0', elements: many } }).join('\n')).toContain('1501 elements')
        const chain = Object.fromEntries(Array.from({ length: 20 }, (_, i) => [`n${i}`, { type: 'Stack', props: {}, children: i < 19 ? [`n${i + 1}`] : [] }]))
        expect(messages({ title: 'T', spec: { root: 'n0', elements: chain } }).join('\n')).toContain('deeper than 16')
    })

    it('needs a title and a spec', () => {
        expect(messages({ spec: card().spec })[0]).toContain('title: is required')
        expect(messages({ title: 'T' })[0]).toContain('spec: is required')
        expect(messages('nope')[0]).toContain('arguments must be an object')
    })
})

describe('formatIssues', () => {
    it('lists every problem, caps at 20 and never echoes the spec', () => {
        const issues = Array.from({ length: 25 }, (_, i) => ({ path: `elements.e${i}.type`, message: 'bad' }))
        const text = formatIssues(issues)
        expect(text).toContain('refused the interface (25 problems)')
        expect(text).toContain('20. elements.e19.type: bad')
        expect(text).not.toContain('21. ')
        expect(text).toContain('…and 5 more.')
        expect(text).toContain('ui_components(names=[…])')
    })

    it('says "problem" for one', () => {
        expect(formatIssues([{ path: 'title', message: 'is required' }])).toContain('(1 problem):')
    })
})
