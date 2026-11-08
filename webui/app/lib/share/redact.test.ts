import { describe, expect, it } from 'vitest'
import { redactText, redactValue, newCounts } from './redact'

// Test secrets are assembled so no literal looks like a live credential to a scanner.
const j = (...p: string[]) => p.join('')
const OPENAI = j('sk-', 'proj-', 'Ab3dEf7hIj1kLm5nOp9qRs2tUv4wXy6z')
const ANTHROPIC = j('sk-ant-', 'api03-', 'Zy9xWv8uTs7rQp6oNm5lKj4iHg3fEd2c')
const GOOGLE = j('AIza', 'SyD3mo9KfakeFakeFakeFake1234567890ab')
const AWS = j('AKIA', 'IOSFODNN7EXAMPLE')
const GH = j('gh', 'p_', 'a1B2c3D4e5F6g7H8i9J0k1L2m3N4o5P6q7R8')
const GHPAT = j('github', '_pat_', '11ABCDEFG0abcdefghij_klmnopqrstuvwxyz0123456789ABCDEFGHIJKLMNOPQRSTUV')
const SLACK = j('xo', 'xb-', '123456789012-abcdefghijklmnop')
const STRIPE = j('sk_', 'live_', '4eC39HqLyjWDarjtT1zdp7dc')
const JWT = j('eyJhbGciOiJIUzI1NiJ9', '.eyJzdWIiOiJhbGljZSIsImV4cCI6OTk5OTk5OTl9', '.dBjftJeZ4CVPmB92K27uhbUJU1p1r_wW1gFWFOEjXk')
const BEARER = 'q8Zr7Yx6Wv5Ut4Ss3Rr2Qq1Pp0OoNnMmLlKkJj'
const PEM = `-----BEGIN ${'RSA '}PRIVATE KEY-----\nMIIEowIBAAKCAQEAfakefakefake0123456789\nabcdefghijklmnopqrstuvwxyz0123456789ABCD\n-----END ${'RSA '}PRIVATE KEY-----`

const run = (s: string) => {
    const counts = newCounts()
    const text = redactText(s, counts)
    return { text, count: counts.credentials }
}

describe('redactText: one case per shape', () => {
    const cases: [string, string, string][] = [
        ['OpenAI', `use ${OPENAI} now`, OPENAI],
        ['Anthropic', `key=${ANTHROPIC}`, ANTHROPIC],
        ['Google', `?x=1 ${GOOGLE}`, GOOGLE],
        ['AWS access key id', `id ${AWS}.`, AWS],
        ['GitHub ghp_', `token ${GH}`, GH],
        ['GitHub fine grained', `${GHPAT}`, GHPAT],
        ['Slack', `bot ${SLACK}`, SLACK],
        ['Stripe', `${STRIPE}`, STRIPE],
        ['JWT', `cookie ${JWT};`, JWT],
        ['Bearer header', `Authorization: Bearer ${BEARER}`, BEARER],
        ['bare Bearer', `curl -H "Bearer ${BEARER}"`, BEARER],
        ['PEM block', `before\n${PEM}\nafter`, 'MIIEowIBAAKCAQEA'],
        ['password in URL query', 'GET https://x.test/a?user=bob&password=hunter2xyz&z=1', 'hunter2xyz'],
        ['token in URL query', 'https://x.test/cb?token=abcd1234efgh', 'abcd1234efgh'],
        ['user:pass@ in URL', 'postgres://admin:s3cr3tPw@db.internal:5432/app', 's3cr3tPw'],
        ['.env line', 'DATABASE_PASSWORD=correct-horse-battery\nOTHER=1', 'correct-horse-battery'],
        ['.env API_KEY', 'API_KEY=abcdef123456', 'abcdef123456'],
        ['quoted yaml-ish', 'password: "p4ssw0rd!x"', 'p4ssw0rd!x'],
    ]
    it.each(cases)('removes %s', (_name, input, secret) => {
        const { text, count } = run(input)
        expect(text).not.toContain(secret)
        expect(text).toContain('[removed:')
        expect(count).toBeGreaterThanOrEqual(1)
    })

    it('keeps the words around a removed key', () => {
        expect(run(`use ${OPENAI} now`).text).toBe('use [removed: key] now')
        expect(run(`Authorization: Bearer ${BEARER}`).text).toBe('Authorization: Bearer [removed: token]')
    })

    it('counts every occurrence', () => {
        const { count } = run(`${OPENAI} and ${AWS} and ${GH}`)
        expect(count).toBe(3)
    })

    it('is idempotent', () => {
        const once = run(`a ${OPENAI} b password=hunter2xyz ${PEM}`).text
        expect(run(once)).toEqual({ text: once, count: 0 })
    })
})

describe('redactText: split and nested', () => {
    it('removes a key split across lines', () => {
        const half = OPENAI.length >> 1
        const { text } = run(`key: ${OPENAI.slice(0, half)}\n${OPENAI.slice(half)}\nend`)
        expect(text).not.toContain(OPENAI.slice(0, half))
        expect(text).not.toContain(OPENAI.slice(half))
        expect(text).toContain('end')
    })
    it('removes a key split with an escaped newline inside JSON text', () => {
        const half = ANTHROPIC.length >> 1
        const { text } = run(`{"out":"${ANTHROPIC.slice(0, half)}\\n${ANTHROPIC.slice(half)}"}`)
        expect(text).not.toContain(ANTHROPIC.slice(half))
        expect(text).not.toContain(ANTHROPIC.slice(0, half))
    })
    it('removes a key inside a JSON string of a tool result', () => {
        const result = JSON.stringify({ stdout: JSON.stringify({ password: 'hunter2xyz', k: OPENAI }) })
        const { text } = run(result)
        expect(text).not.toContain('hunter2xyz')
        expect(text).not.toContain(OPENAI)
        expect(() => JSON.parse(text)).not.toThrow()
    })
    it('removes a PEM block written with escaped newlines', () => {
        const { text } = run(JSON.stringify({ k: PEM }))
        expect(text).not.toContain('MIIEowIBAAKCAQEA')
        expect(() => JSON.parse(text)).not.toThrow()
    })
    it('removes an unterminated PEM block to the end', () => {
        const { text } = run('x\n-----BEGIN PRIVATE KEY-----\nMIIEvQIBADANBgkqhkiG9w0BAQEFAASC')
        expect(text).toBe('x\n[removed: private key]')
    })
})

describe('redactValue: arguments and results', () => {
    it('walks objects and arrays and removes by key name', () => {
        const counts = newCounts()
        const out = redactValue({ command: `curl ${OPENAI}`, env: { API_KEY: 'plainvalue', password: 'x' }, list: [AWS, 3, null, true], authorization: 'Bearer abc' }, counts) as Record<string, unknown>
        const dump = JSON.stringify(out)
        for (const s of [OPENAI, 'plainvalue', AWS, 'Bearer abc']) expect(dump).not.toContain(s)
        expect(out.list).toEqual([REMOVED_KEY(), 3, null, true])
        expect(counts.credentials).toBe(5)
    })
    it('leaves numbers, booleans and nulls and does not mutate the input', () => {
        const input = { n: 1, b: false, z: null, s: 'hello' }
        const copy = JSON.parse(JSON.stringify(input))
        expect(redactValue(input, newCounts())).toEqual(copy)
        expect(input).toEqual(copy)
    })
    it('survives cycles and very deep values', () => {
        const a: Record<string, unknown> = { s: OPENAI }
        a.self = a
        expect(() => redactValue(a, newCounts())).not.toThrow()
        let deep: unknown = OPENAI
        for (let i = 0; i < 5000; i++) deep = [deep]
        expect(() => redactValue(deep, newCounts())).not.toThrow()
    })
})
const REMOVED_KEY = () => '[removed: key]'

describe('redactText: no false positives (TP-27)', () => {
    const keep = [
        'commit 3f786850e387550fdab836ed7e6dc881de23001b merged',
        'sha256 9f86d081884c7d659a2feaa0c55ad015a3bf4f1b2b0b822cd15d6c15b0f00a08',
        'id 123e4567-e89b-12d3-a456-426614174000 matches',
        'A token is a unit of text; tokens are counted per request. The token limit is 8k.',
        'Authorization: Bearer tokens are described below.',
        'the bearer token is passed in a header',
        'password: required for this account',
        'Set the password field, then click Save. password reset email sent.',
        'token=true and secret=null',
        'a task-force-related-long-hyphenated-word and disk-usage-statistics-by-folder',
        'ask-for-help-with-the-long-running-process',
        'data:image/png;base64,iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAYAAAAfFcSJAAAADUlEQVR42mNk+M9QDwADhgGAWjR9awAAAABJRU5ErkJggg==',
        'https://example.com/a/very/long/path/with-ids-1234567890/and/more?page=2&sort=asc',
        'The key to success is practice; press the Enter key.',
        'AKIA is a prefix; AKIAshort',
        'Use `eyJ` as the prefix of a JWT header.',
    ]
    it.each(keep)('keeps %s', (s) => {
        expect(run(s)).toEqual({ text: s, count: 0 })
    })
    it('keeps a long base64 image blob untouched, even with sk- and AKIA lookalikes inside', () => {
        const blob = 'iVBORw0KGgo' + 'AAAA' + 'sk-AAAABBBBCCCCDDDD1111/AKIAABCDEFGHIJKLMNOP/'.repeat(30) + '=='
        expect(run(blob)).toEqual({ text: blob, count: 0 })
        const inText = `see data:image/jpeg;base64,${blob} done`
        expect(run(inText)).toEqual({ text: inText, count: 0 })
    })
    it('does not touch a 100 KB unbroken line quickly', () => {
        const long = 'a'.repeat(100 * 1024)
        const t0 = Date.now()
        expect(run(long).count).toBe(0)
        expect(Date.now() - t0).toBeLessThan(1500)
    })
})
