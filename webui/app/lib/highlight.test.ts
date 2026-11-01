import { describe, expect, it } from 'vitest'
import { highlightCode, resolveLanguage } from './highlight'

/** Every tone used in a block, so a test can assert what a language produced. */
const tones = (code: string, language: string) => (highlightCode(code, language) ?? []).flat().map((token) => `${token.text}:${token.tone ?? 'plain'}`)

/** What a line of tokens says, whatever it is coloured as. */
const text = (code: string, language: string) =>
    (highlightCode(code, language) ?? []).map((line) => line.map((token) => token.text).join(''))

describe('highlighting a fence', () => {
    it('colours python keywords, strings and numbers', () => {
        const out = tones('for x in hits:\n    print("db", 8)', 'python')
        expect(out).toContain('for:keyword')
        expect(out).toContain('in:keyword')
        expect(out).toContain('"db":string')
        expect(out).toContain('8:number')
    })

    it('colours a json value as a string and its key as plain text', () => {
        const out = tones('{ "name": "oz" }', 'json')
        expect(out).toContain('"oz":string')
        expect(out).toContain('"name":plain')
    })

    it('colours yaml values and comments', () => {
        const out = tones('name: oz # the agent\n', 'yaml')
        expect(out).toContain('oz:string')
        expect(out).toContain('# the agent:comment')
    })

    // The block draws these lines instead of the source, so the two must say the same thing.
    it('keeps every character, in order', () => {
        const source = 'def total(items):\n    return sum(items)  # noqa\n'
        expect(text(source, 'python').join('\n')).toBe(source.replace(/\n$/, ''))
    })

    it('draws a blank line as one empty line, not as none', () => {
        expect(text('a = 1\n\nb = 2', 'python')).toHaveLength(3)
    })

    it('highlights nothing without a language it knows', () => {
        expect(highlightCode('a = 1', undefined)).toBeUndefined()
        expect(highlightCode('a = 1', '')).toBeUndefined()
        expect(highlightCode('a = 1', 'brainfuck')).toBeUndefined()
        expect(highlightCode('a = 1', 'text')).toBeUndefined()
    })

    it('falls back to plain text rather than throwing on odd input', () => {
        expect(() => highlightCode('for x in :\n\u0000', 'python')).not.toThrow()
    })
})

describe('fence names', () => {
    it('maps the aliases the agent writes', () => {
        expect(resolveLanguage('py')).toBe('python')
        expect(resolveLanguage('TSX')).toBe('typescript')
        expect(resolveLanguage('yml')).toBe('yaml')
        expect(resolveLanguage('sh')).toBe('bash')
        expect(resolveLanguage('rs')).toBe('rust')
        expect(resolveLanguage('html')).toBe('xml')
    })

    it('takes the first word of a fence that carries more than its language', () => {
        expect(resolveLanguage('python title="run.py"')).toBe('python')
        expect(resolveLanguage('  json  ')).toBe('json')
    })

    // The agent fences its scripts as `execute_python` by habit; that is python.
    it('knows the tool names a fence is written with', () => {
        expect(resolveLanguage('execute_python')).toBe('python')
    })

    it('leaves a language it has no grammar for alone', () => {
        expect(resolveLanguage('graphql')).toBeUndefined()
        expect(resolveLanguage(undefined)).toBeUndefined()
    })
})
