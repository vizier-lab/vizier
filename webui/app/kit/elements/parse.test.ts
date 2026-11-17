import { describe, expect, it } from 'vitest'
import { ELEMENTS, LIBRARIES, manifestData } from '../manifest'
import { parseAttributes } from './parse'

const spec = (name: string) => ELEMENTS.find((e) => e.name === name)!
const attrs = (record: Record<string, string>) => (name: string) => record[name] ?? null

describe('reading an element’s attributes', () => {
    it('turns valid attributes into values', () => {
        const parsed = parseAttributes(spec('vz-stat'), attrs({ label: 'Activated', value: '0.61', format: 'percent' }))
        expect(parsed).toEqual({ values: { label: 'Activated', value: 0.61, format: 'percent' }, problems: [] })
    })

    it('reports every problem at once, naming the attribute', () => {
        const parsed = parseAttributes(spec('vz-stat'), attrs({ value: 'lots', format: 'percentage' }))
        expect(parsed.problems).toEqual([
            'label is required',
            'value: expected a number, got "lots"',
            'format: "percentage" is not one of number, percent, currency, bytes, duration',
        ])
    })

    it('checks JSON attributes against their shape, with the path of the first issue', () => {
        const bad = parseAttributes(spec('vz-table'), attrs({ columns: '[{"key":"a","label":"A","align":"middle"}]' }))
        expect(bad.problems).toEqual([expect.stringMatching(/^columns\[0\.align\]: /)])
        const notJson = parseAttributes(spec('vz-chart'), attrs({ kind: 'bar', data: '[{label: x}]' }))
        expect(notJson.problems).toEqual(['data: not valid JSON ([{label: x}])'])
    })

    it('takes a JSON value set from script over the attribute', () => {
        const parsed = parseAttributes(spec('vz-chart'), attrs({ kind: 'line', data: 'broken' }), { data: [{ label: 'Mon', value: 3 }] })
        expect(parsed.problems).toEqual([])
        expect(parsed.values.data).toEqual([{ label: 'Mon', value: 3 }])
    })

    it('reads a boolean the HTML way', () => {
        const on = parseAttributes(spec('vz-table'), attrs({ columns: '[{"key":"a","label":"A"}]', selectable: '' }))
        const off = parseAttributes(spec('vz-table'), attrs({ columns: '[{"key":"a","label":"A"}]', selectable: 'false' }))
        expect(on.values.selectable).toBe(true)
        expect(off.values.selectable).toBe(false)
    })
})

describe('the manifest', () => {
    it('names every element once, with a description and an example using its own tag', () => {
        const names = ELEMENTS.map((e) => e.name)
        expect(new Set(names).size).toBe(names.length)
        for (const element of ELEMENTS) {
            expect(element.name).toMatch(/^vz-[a-z]+$/)
            expect(element.description.length).toBeGreaterThan(10)
            expect(element.example).toContain(`<${element.name}`)
            for (const attr of Object.values(element.attributes)) expect(attr.description.length).toBeGreaterThan(3)
        }
    })

    it('parses every example without a problem', () => {
        for (const element of ELEMENTS) {
            const tag = element.example.match(new RegExp(`<${element.name}([^>]*)>`))![1]
            const found: Record<string, string> = {}
            for (const m of tag.matchAll(/([a-z-]+)(?:=(?:'([^']*)'|"([^"]*)"))?/g)) found[m[1]] = m[2] ?? m[3] ?? ''
            expect(parseAttributes(element, attrs(found)).problems, element.name).toEqual([])
        }
    })

    it('is plain JSON for the build, without the zod schemas', () => {
        const data = manifestData()
        expect(JSON.parse(JSON.stringify(data))).toEqual(data)
        expect(data.libraries).toEqual(LIBRARIES.map((l) => l.name))
        expect(data.elements.find((e) => e.name === 'vz-table')!.attributes.columns).not.toHaveProperty('schema')
    })
})
