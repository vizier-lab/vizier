import { describe, expect, it } from 'vitest'
import type { CommandsMenu, WorkspaceMember } from '../api/endpoints/workspaces'
import { leadingCommand, mentionSuggestion, slashItems } from './workspaceCompose'

const member = (kind: 'person' | 'agent', handle: string, available = true): WorkspaceMember => ({
    kind,
    handle,
    name: handle[0].toUpperCase() + handle.slice(1),
    role: 'member',
    available,
})
const MEMBERS = [member('person', 'rafi'), member('person', 'maya'), member('agent', 'alpha'), member('agent', 'beta'), member('agent', 'gone', false)]
const MENU: CommandsMenu = {
    commands: [{ name: 'abort', description: 'Stop' }],
    skills: [{ name: 'greet', description: 'Say hello', agents: ['alpha'] }],
}

describe('the slash menu', () => {
    it('lists commands, then skills with the agents that have them', () => {
        expect(slashItems(MENU)).toEqual([
            { name: 'abort', description: 'Stop' },
            { name: 'greet', description: 'Say hello', agents: ['alpha'] },
        ])
        expect(slashItems(null)).toEqual([])
    })

    it('finds the command a message starts with', () => {
        expect(leadingCommand('/greet @')).toBe('greet')
        expect(leadingCommand('/skill greet @a')).toBe('greet')
        expect(leadingCommand('/greet')).toBeNull()
        expect(leadingCommand('hi /greet @')).toBeNull()
    })
})

describe('the @ menu', () => {
    it('offers agents first, then people, leaving out yourself and an agent that is gone', () => {
        const s = mentionSuggestion('hello @', MEMBERS, MENU, 'rafi')!
        expect(s.items.map((i) => i.id)).toEqual(['alpha', 'beta', 'maya'])
        expect(s.start).toBe(6)
    })

    it('filters by what was typed, handle or name', () => {
        expect(mentionSuggestion('@al', MEMBERS, MENU)!.items.map((i) => i.id)).toEqual(['alpha'])
        expect(mentionSuggestion('@Maya', MEMBERS, MENU)!.items.map((i) => i.id)).toEqual(['maya'])
    })

    it('after a command, offers only agents that can take it', () => {
        expect(mentionSuggestion('/abort @', MEMBERS, MENU)!.items.map((i) => i.id)).toEqual(['alpha', 'beta'])
        // A skill goes only to the agents that have it.
        expect(mentionSuggestion('/greet @', MEMBERS, MENU)!.items.map((i) => i.id)).toEqual(['alpha'])
    })

    it('says who it looked for when nothing matches', () => {
        const s = mentionSuggestion('/greet @zed', MEMBERS, MENU)!
        expect(s.items).toEqual([])
        expect(s.emptyText).toBe('Nobody is an agent with /greet called “@zed”.')
    })

    it('offers nothing when no @ is being typed', () => {
        expect(mentionSuggestion('plain text', MEMBERS, MENU)).toBeNull()
        expect(mentionSuggestion('mail@x', MEMBERS, MENU)).toBeNull()
    })
})
