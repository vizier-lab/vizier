import { describe, expect, it } from 'vitest'
import { eta, setupProgress } from './agents'
import type { SetupState } from '../api/endpoints/agents'

function setup(
    downloads: SetupState['downloads'],
    phase = 'downloading',
    name?: string
): SetupState {
    return { agent_id: 'scribe', name, phase, message: 'Pulling', downloads }
}

describe('setup progress', () => {
    it('averages the known download fractions from bytes', () => {
        expect(setupProgress(setup([]))).toBeNull()
        expect(
            setupProgress(
                setup([
                    { kind: 'model', label: 'qwen', completed: 50, total: 100 },
                    { kind: 'embedding', label: 'bge', completed: 25, total: 100 },
                ])
            )
        ).toBeCloseTo(0.375)
    })

    it('is indeterminate while every size is unknown', () => {
        expect(
            setupProgress(
                setup([{ kind: 'model', label: 'qwen', completed: 12, total: 0 }])
            )
        ).toBeNull()
    })

    it('reports the agent name when the build carries one', () => {
        expect(setup([]).name).toBeUndefined()
        expect(setup([], 'starting', 'Scribe').name).toBe('Scribe')
    })
})

describe('eta', () => {
    it('is unknown until two samples moved', () => {
        expect(eta(undefined)).toBeNull()
        expect(eta([{ at: 0, progress: 0.2 }])).toBeNull()
        expect(eta([{ at: 1000, progress: 0.2 }, { at: 2000, progress: 0.2 }])).toBeNull()
    })

    it('extrapolates the remaining time from the recent rate', () => {
        // 0.2 → 0.4 over two seconds is 0.1 a second, so 0.6 left is about 6s.
        expect(eta([{ at: 1000, progress: 0.2 }, { at: 3000, progress: 0.4 }])).toBe(6)
    })
})
