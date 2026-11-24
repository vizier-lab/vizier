import { describe, expect, it } from 'vitest'
import { singleFlight } from './singleFlight'

describe('singleFlight (FR-050, SC-015)', () => {
    it('two confirmations in the same instant run the action once and share its result', async () => {
        const once = singleFlight<string>()
        let runs = 0
        let release!: () => void
        const gate = new Promise<void>((r) => (release = r))
        const action = async () => {
            runs++
            await gate
            return 'link'
        }
        const first = once(action)
        const second = once(action)
        release()
        expect(await Promise.all([first, second])).toEqual(['link', 'link'])
        expect(runs).toBe(1)
    })
    it('confirming again later, on purpose, runs again', async () => {
        const once = singleFlight<number>()
        let runs = 0
        await once(async () => ++runs)
        await once(async () => ++runs)
        expect(runs).toBe(2)
    })
    it('a failure frees the slot', async () => {
        const once = singleFlight<void>()
        await expect(once(async () => { throw new Error('x') })).rejects.toThrow('x')
        await expect(once(async () => undefined)).resolves.toBeUndefined()
    })
})
