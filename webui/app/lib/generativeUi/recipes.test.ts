import { describe, expect, it } from 'vitest'
import { RECIPES } from './recipes/index'
import { validateUi } from './validate'

describe('recipes', () => {
    for (const [name, recipe] of Object.entries(RECIPES)) {
        it(`${name} is a valid interface`, () => {
            const result = validateUi({ title: name, spec: recipe.spec })
            expect(result.ok, result.ok ? '' : JSON.stringify(result.issues, null, 1)).toBe(true)
            if (result.ok) expect(result.fixed).toEqual([])
        })

        it(`${name} says what it is for and what to adapt`, () => {
            expect(recipe.intent.length).toBeGreaterThan(10)
            expect(recipe.notes.length).toBeGreaterThan(0)
        })
    }
})
