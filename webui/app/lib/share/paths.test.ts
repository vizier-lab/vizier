import { describe, expect, it } from 'vitest'
import { newCounts } from './redact'
import { stripPaths } from './paths'

const run = (s: string) => {
    const counts = newCounts()
    return { text: stripPaths(s, counts), count: counts.paths }
}

describe('stripPaths: removes the directory part, keeps the base name', () => {
    it.each([
        ['open /home/alice/x now', 'open x now'],
        ['at /root/.vizier/config.toml.', 'at config.toml.'],
        ['(/Users/bob/Library/notes.txt)', '(notes.txt)'],
        ['C:\\Users\\alice\\Documents\\report.pdf', 'report.pdf'],
        ['C:/Users/alice/Documents/report.pdf', 'report.pdf'],
        ['\\\\fileserver\\share\\dept\\budget.xlsx', 'budget.xlsx'],
        ['file:///home/alice/pics/cat.png', 'cat.png'],
        ['file:///C:/Users/alice/pics/cat.png', 'cat.png'],
        ['cd ~/projects/vizier/src', 'cd src'],
        ['in /tmp/build-1/out.log, then', 'in out.log, then'],
    ])('%s', (input, expected) => {
        const { text, count } = run(input)
        expect(text).toBe(expected)
        expect(count).toBe(1)
    })

    it('handles a path with spaces in a directory and unicode in the name', () => {
        const { text, count } = run('see /home/alice/My Documents/résumé 2026/café.md ok')
        expect(text).toBe('see café.md ok')
        expect(count).toBe(1)
    })
    it('handles a quoted path with spaces in the name', () => {
        expect(run('open "/home/alice/My Documents/final report.pdf" please').text).toBe('open "final report.pdf" please')
        expect(run("open 'C:\\Users\\alice\\My Files\\final report.pdf'").text).toBe("open 'final report.pdf'")
    })
    it('handles JSON-escaped Windows paths', () => {
        const json = JSON.stringify({ p: 'C:\\Users\\alice\\.vizier\\db.sqlite', u: '\\\\srv\\share\\a.txt' })
        const { text, count } = run(json)
        expect(count).toBe(2)
        expect(JSON.parse(text)).toEqual({ p: 'db.sqlite', u: 'a.txt' })
    })
    it('removes a path inside a code block', () => {
        const { text } = run('```sh\ncat /home/alice/.vizier/secrets.env\n```')
        expect(text).toBe('```sh\ncat secrets.env\n```')
    })
    it('counts each path', () => {
        expect(run('/home/a/x /home/b/y C:\\Users\\c\\z').count).toBe(3)
    })
    it('is idempotent', () => {
        const once = run('/home/alice/x and C:\\Users\\a\\b').text
        expect(run(once)).toEqual({ text: once, count: 0 })
    })
})

describe('stripPaths: leaves alone', () => {
    const keep = [
        'edit src/main.rs and ./lib/util.ts and ../docs/readme.md',
        'workspace file notes/todo.md',
        'https://example.com/home/alice/page',
        'http://localhost:3000/Users/list?x=/root/a',
        'mailto:bob@example.com',
        'use and/or either/or 1/2 and 3/4',
        'the /api/v1/shares endpoint',
        'a date 2026/11/08 and ratio 16/9',
        'file name report.pdf only',
        'visit www.example.com/tmp/file',
    ]
    it.each(keep)('keeps %s', (s) => {
        expect(run(s)).toEqual({ text: s, count: 0 })
    })
    it('does not alter markdown links to URLs', () => {
        const s = '[doc](https://example.com/root/docs/x.md) and ![img](https://cdn.example.com/home/a.png)'
        expect(run(s)).toEqual({ text: s, count: 0 })
    })
})
