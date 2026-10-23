import { describe, expect, it } from 'vitest'
import { groupsFor, permissionCount } from './permissions'

describe('groupsFor', () => {
    it('keeps only what the server offers and puts the unknown last', () => {
        const groups = groupsFor(['all_agents:view', 'users:manage', 'brand:new'])
        expect(groups.map((g) => g.title)).toEqual(['Agents', 'Administration', 'Other'])
        expect(groups[2].items[0]).toEqual({ permission: 'brand:new', label: 'brand:new' })
    })
})

describe('permissionCount', () => {
    it('words a count', () => {
        expect(permissionCount(4, 15)).toBe('4 permissions')
        expect(permissionCount(1, 15)).toBe('1 permission')
        expect(permissionCount(15, 15)).toBe('all permissions')
    })
})
