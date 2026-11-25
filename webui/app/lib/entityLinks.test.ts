import { expect, it } from 'vitest'
import {
    entityFromHref,
    entityHref,
    entityToken,
    insertEntityLink,
    parseEntityLink,
    rewriteEntityLinks,
} from './entityLinks'

it('builds the href for every kind from the kind→target table', () => {
    expect(entityHref('plug', 'artifact', 'report')).toBe('/plug/artifacts?artifact=report')
    expect(entityHref('plug', 'function', 'fetch_weather')).toBe('/plug/functions/fetch_weather')
    expect(entityHref('plug', 'skill', 'summarize')).toBe('/plug/skills/summarize')
    expect(entityHref('plug', 'task', 'weekly-report')).toBe('/plug/tasks/weekly-report')
    expect(entityHref('plug', 'artifact', 'q report')).toBe('/plug/artifacts?artifact=q%20report')
})

it('parses entity targets and rejects everything else', () => {
    expect(parseEntityLink('artifact:report')).toMatchObject({ kind: 'artifact', name: 'report', alias: undefined })
    expect(parseEntityLink(' skill : summarize ')).toMatchObject({ kind: 'skill', name: 'summarize' })
    expect(parseEntityLink('artifact:report|my report')).toMatchObject({ kind: 'artifact', name: 'report', alias: 'my report' })
    expect(parseEntityLink('bundle/slug')).toBeNull()
    expect(parseEntityLink('bundle')).toBeNull()
    expect(parseEntityLink('flavour:vanilla')).toBeNull()
    expect(parseEntityLink('artifact:')).toBeNull()
})

it('rewrites every kind, honoring aliases', () => {
    expect(
        rewriteEntityLinks(
            'plug',
            'See [[artifact:report]], [[function:fetch_weather]], [[skill:summarize|the skill]] and [[task:weekly-report]].'
        )
    ).toBe(
        'See [report](/plug/artifacts?artifact=report), [fetch_weather](/plug/functions/fetch_weather), [the skill](/plug/skills/summarize) and [weekly-report](/plug/tasks/weekly-report).'
    )
})

it('leaves code, memory forms, unknown kinds and unclosed brackets alone', () => {
    const md =
        'Use `[[artifact:x]]` or\n```\n[[task:y]]\n```\nbut [[bundle/slug]], [[bundle]], [[slug]], [[flavour:vanilla]] and [[artifact:stay open.'
    expect(rewriteEntityLinks('plug', md)).toBe(md)
})

it('round-trips an internal href back to its kind and name', () => {
    expect(entityFromHref('/plug/artifacts?artifact=report')).toMatchObject({ kind: 'artifact', name: 'report' })
    expect(entityFromHref('/plug/functions/fetch_weather')).toMatchObject({ kind: 'function', name: 'fetch_weather' })
    expect(entityFromHref('/plug/memory/research/notes')).toBeNull()
    expect(entityFromHref('https://example.com/x')).toBeNull()
})

it('scans a `[[` token at the end of the input, with or without the kind prefix', () => {
    expect(entityToken('see [[')).toMatchObject({ start: 4, kind: null, query: '' })
    expect(entityToken('see [[sum')).toMatchObject({ start: 4, kind: null, query: 'sum' })
    expect(entityToken('see [[skill:')).toMatchObject({ start: 4, kind: 'skill', query: '' })
    expect(entityToken('see [[skill:sum')).toMatchObject({ start: 4, kind: 'skill', query: 'sum' })
    // A completed link is no longer a token; neither is mid-word or code.
    expect(entityToken('see [[skill:summarize]] done')).toBeNull()
    expect(entityToken('x[[y')).toBeNull()
    expect(entityToken('`[[x')).toBeNull()
})

it('inserts the complete link in place of the partial token', () => {
    expect(insertEntityLink('see [[sk', 4, 'skill', 'summarize')).toBe('see [[skill:summarize]] ')
    expect(insertEntityLink('', 0, 'artifact', 'report')).toBe('[[artifact:report]] ')
})
