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

describe('surfaces', () => {
    const scene = (extra: Record<string, unknown>) => ({
        title: 'T',
        spec: { root: 's', elements: { s: { type: 'Scene3D', props: {}, children: ['x'] }, x: { type: 'Box', props: {} }, ...extra } },
    })

    it('accepts a 3D part inside a scene, directly or in a group', () => {
        expect(validateUi(scene({})).ok).toBe(true)
        expect(validateUi({ title: 'T', spec: { root: 's', elements: { s: { type: 'Scene3D', props: {}, children: ['g'] }, g: { type: 'Group3D', props: {}, children: ['x'] }, x: { type: 'Box', props: {} } } } }).ok).toBe(true)
    })

    it('refuses a 3D part outside a scene and a 2D child inside one', () => {
        const outside = messages({ title: 'T', spec: { root: 'a', elements: { a: { type: 'Stack', props: {}, children: ['x'] }, x: { type: 'Box', props: {} } } } })
        expect(outside[0]).toContain('Box is a 3D part: put it inside a Scene3D')
        const inside = messages({ title: 'T', spec: { root: 's', elements: { s: { type: 'Scene3D', props: {}, children: ['t'] }, t: { type: 'Text', props: { text: 'x' } } } } })
        expect(inside.join('\n')).toContain('Scene3D holds 3D parts only; "t" is a Text')
    })

    it('checks a heightfield has resolution² heights and a resolution within the limit', () => {
        const field = (props: Record<string, unknown>) => scene({ x: { type: 'Heightfield', props: { size: [10, 10], ...props } } })
        expect(messages(field({ resolution: 3, heights: [0, 0, 0] }))[0]).toContain('has 3 numbers; resolution 3 needs 9')
        expect(messages(field({ resolution: 500, heights: [] })).join('\n')).toMatch(/resolution/)
        expect(validateUi(field({ resolution: 2, heights: [0, 1, 1, 0] })).ok).toBe(true)
        expect(validateUi(field({ resolution: 3, heights: { $state: '/_h' } })).ok).toBe(true)
    })

    it('limits shapes and repeated items', () => {
        const shapes = Array.from({ length: 5001 }, () => ({ kind: 'rect' }))
        expect(messages({ title: 'T', spec: { root: 'c', elements: { c: { type: 'Canvas2D', props: { width: 10, height: 10, shapes } } } } }).join('\n')).toMatch(/shapes/)
        const items = Array.from({ length: 2049 }, (_, i) => i)
        const out = messages({ title: 'T', spec: { root: 'a', state: { items }, elements: { a: { type: 'Stack', props: {}, repeat: { statePath: '/items' }, children: ['b'] }, b: { type: 'Divider', props: {} } } } })
        expect(out.join('\n')).toContain('repeats 2049 items; the limit is 2048')
    })
})

describe('links and media', () => {
    const ui = (el: Record<string, unknown>) => ({ title: 'T', spec: { root: 'a', elements: { a: el } } })
    const files = { files: [{ name: 'forecast.png', mime: 'image/png' }, { name: 'report.pdf', mime: 'application/pdf' }] }
    const out = (args: unknown, ctx = files) => {
        const r = validateUi(args, ctx)
        return r.ok ? [] : r.issues.map((i) => `${i.path}: ${i.message}`)
    }

    it('accepts files of the conversation by name', () => {
        expect(validateUi(ui({ type: 'Image', props: { file: 'forecast.png', alt: 'x' } }), files).ok).toBe(true)
        expect(validateUi(ui({ type: 'FileCard', props: { file: 'report.pdf' } }), files).ok).toBe(true)
        expect(validateUi(ui({ type: 'Gallery', props: { files: ['forecast.png'] } }), files).ok).toBe(true)
    })

    it('refuses an unknown file and lists the available ones', () => {
        const [issue] = out(ui({ type: 'Image', props: { file: 'cat.png', alt: 'x' } }))
        expect(issue).toContain('elements.a.props.file: "cat.png" is not a file of this conversation.')
        expect(issue).toContain('Available: forecast.png (image/png), report.pdf (application/pdf).')
        expect(issue).toContain('list_session_files')
    })

    it('says so when the conversation has no files, and skips the check when it is not known', () => {
        expect(out(ui({ type: 'Audio', props: { file: 'a.mp3' } }), { files: [] })[0]).toContain('This conversation has no files yet.')
        expect(validateUi(ui({ type: 'Audio', props: { file: 'a.mp3' } })).ok).toBe(true)
    })

    it('refuses an external image address and an image with no source', () => {
        expect(out(ui({ type: 'Image', props: { src: 'https://example.com/x.png', alt: 'x' } }))[0]).toContain('must be a data:image/ URL')
        expect(out(ui({ type: 'Image', props: { alt: 'x' } }))[0]).toContain('Image needs file')
        expect(validateUi(ui({ type: 'Image', props: { src: 'data:image/png;base64,AAAA', alt: 'x' } }), files).ok).toBe(true)
    })

    it('refuses links that break the rules, naming the allowed schemes', () => {
        for (const href of ['javascript:alert(1)', 'data:text/html,x', '//host', 'file:///etc/passwd']) {
            expect(out(ui({ type: 'Link', props: { text: 'x', href } }))[0]).toContain('Links take https, http, mailto')
        }
        expect(validateUi(ui({ type: 'Link', props: { text: 'Docs', href: 'https://example.com' } }), files).ok).toBe(true)
        expect(validateUi(ui({ type: 'Link', props: { text: 'Get', file: 'report.pdf' } }), files).ok).toBe(true)
        expect(out(ui({ type: 'Link', props: { text: 'x' } }))[0]).toContain('Link needs href')
    })

    it('checks the links written in markdown text', () => {
        expect(out(ui({ type: 'Text', props: { text: 'see [a](javascript:alert(1))' } }))[0]).toContain('is not allowed')
        expect(validateUi(ui({ type: 'Text', props: { text: 'see [a](https://example.com)' } }), files).ok).toBe(true)
    })

    it('limits the number of different files', () => {
        const many = Array.from({ length: 101 }, (_, i) => `f${i}.png`)
        const known = { files: many.map((name) => ({ name, mime: 'image/png' })) }
        const issues = out({ title: 'T', spec: { root: 'a', elements: { a: { type: 'Stack', props: {}, children: ['b', 'c', 'd'] }, b: { type: 'Gallery', props: { files: many.slice(0, 40) } }, c: { type: 'Gallery', props: { files: many.slice(40, 80) } }, d: { type: 'Gallery', props: { files: many.slice(80) } } } } }, known)
        expect(issues.join('\n')).toContain('refers to 101 different files; the limit is 100')
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
