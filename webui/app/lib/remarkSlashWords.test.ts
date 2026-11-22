import { describe, expect, it } from 'vitest'
import { splitSlashWords } from './remarkSlashWords'

const slashes = (text: string) => splitSlashWords(text).filter((r) => r.slash).map((r) => r.text)

describe('slash words', () => {
    it('finds a command, a skill and an unknown name alike', () => {
        expect(slashes('/checkpoint')).toEqual(['/checkpoint'])
        expect(slashes('/code-review PR 418')).toEqual(['/code-review'])
        expect(slashes('try /nothing_here, then /skills.')).toEqual(['/nothing_here', '/skills'])
    })

    it('leaves paths and URLs alone', () => {
        expect(slashes('see src/app and /usr/bin and https://x.dev/a')).toEqual([])
    })

    it('keeps the text whole', () => {
        const text = 'run /lobotomy now'
        expect(splitSlashWords(text).map((r) => r.text).join('')).toBe(text)
    })
})
