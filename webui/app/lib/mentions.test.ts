import { describe, expect, it } from 'vitest'
import { insertMention, isCurrent, mentionQuery, pickedMentions } from './mentions'

describe('mentions', () => {
    it('finds the token being typed only at the end', () => {
        expect(mentionQuery('look at @age')).toEqual({ start: 8, query: 'age' })
        expect(mentionQuery('@src/li')).toEqual({ start: 0, query: 'src/li' })
        expect(mentionQuery('an email a@b sent')).toBeNull()
        expect(mentionQuery('no mention')).toBeNull()
    })

    it('keeps only picked mentions still present in the text', () => {
        const text = 'explain @src/main.rs and @src/lib.rs'
        expect(pickedMentions(text, ['src/main.rs', 'docs/readme.md'])).toEqual(['src/main.rs'])
        expect(pickedMentions('no mentions', ['src/main.rs'])).toEqual([])
    })

    it('inserts the picked path in place of the token', () => {
        expect(insertMention('look at @age', 8, 'src/agents/agent.rs')).toBe(
            'look at @src/agents/agent.rs '
        )
    })

    it('drops a stale response', () => {
        expect(isCurrent(3, 4)).toBe(false)
        expect(isCurrent(4, 4)).toBe(true)
    })
})
