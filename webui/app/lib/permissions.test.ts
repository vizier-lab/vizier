import { describe, expect, it } from 'vitest'
import { groupsFor, permissionCount } from './permissions'

describe('groupsFor', () => {
    it('keeps only what the server offers and puts the unknown last', () => {
        const groups = groupsFor(['all_agents:view', 'users:manage', 'brand:new'])
        expect(groups.map((g) => g.title)).toEqual(['Agents', 'Administration', 'Other'])
        expect(groups[2].items[0]).toEqual({ permission: 'brand:new', label: 'brand:new' })
    })

    it('shows model override as a named Chat permission', () => {
        expect(groupsFor(['model_override'])).toEqual([
            { title: 'Chat', items: [{ permission: 'model_override', label: 'Choose a model for chat' }] },
        ])
    })

    it('shows the two sharing permissions under Sharing, between Chat and Account', () => {
        const groups = groupsFor(['owned_agents:share_conversations', 'all_agents:share_conversations', 'settings:password', 'model_override'])
        expect(groups.map((g) => g.title)).toEqual(['Chat', 'Sharing', 'Account'])
        expect(groups[1].items).toEqual([
            { permission: 'owned_agents:share_conversations', label: 'Share conversations with agents they own' },
            { permission: 'all_agents:share_conversations', label: 'Share conversations with any agent they can see' },
        ])
    })

    it('does not put a sharing permission under Other once the server offers it', () => {
        expect(groupsFor(['all_agents:share_conversations']).some((g) => g.title === 'Other')).toBe(false)
    })
})

describe('permissionCount', () => {
    it('words a count', () => {
        expect(permissionCount(4, 15)).toBe('4 permissions')
        expect(permissionCount(1, 15)).toBe('1 permission')
        expect(permissionCount(15, 15)).toBe('all permissions')
    })
})
