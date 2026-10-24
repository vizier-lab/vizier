import { describe, expect, it } from 'vitest'
import { COMPONENT_NAMES } from './catalog/index'
import { registry } from './registry/index'

describe('the registry', () => {
    it('has a renderer for every catalogue entry, and none without one', () => {
        expect(Object.keys(registry).sort()).toEqual([...COMPONENT_NAMES].sort())
    })
})
