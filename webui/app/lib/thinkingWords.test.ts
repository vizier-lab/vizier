import { describe, expect, it } from 'vitest'
import { thinkingWord } from './thinkingWords'

describe('thinkingWord', () => {
    it('changes on every rotation', () => {
        let word = thinkingWord()
        for (let i = 0; i < 100; i++) {
            const next = thinkingWord(word)
            expect(next).not.toBe(word)
            word = next
        }
    })

    it('always says something', () => {
        for (let i = 0; i < 100; i++) expect(thinkingWord().length).toBeGreaterThan(0)
    })
})
