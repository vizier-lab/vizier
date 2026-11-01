import { describe, expect, it } from 'vitest'
import type { Turn } from '../api/frames'
import { planActionsVisible, planOf } from './plans'

function turn(key: string, report: unknown): Turn {
    return {
        key,
        trail: [
            {
                kind: 'python',
                id: 'p1',
                intent: null,
                code: null,
                report: report as never,
            },
        ],
        live: false,
        clientCalls: [], questions: [],
    }
}

const report = {
    ok: true,
    result: null,
    stdout: '',
    tool_calls: [
        {
            seq: 1,
            name: 'propose_plan',
            ok: true,
            duration_ms: 1,
            arguments: { title: 'Rename greet', steps: ['Edit src/main.rs'] },
        },
    ],
    duration_ms: 1,
}

describe('plans', () => {
    it('finds a proposed plan in the ending report', () => {
        expect(planOf(turn('a', report))).toEqual({
            title: 'Rename greet',
            summary: undefined,
            steps: ['Edit src/main.rs'],
        })
        expect(planOf(turn('a', { ...report, ok: false }))).toBeNull()
        expect(planOf(turn('a', { ...report, tool_calls: [] }))).toBeNull()
    })

    it('only the newest turn keeps its actions', () => {
        expect(planActionsVisible('a', 'a')).toBe(true)
        expect(planActionsVisible('a', 'b')).toBe(false)
        expect(planActionsVisible('a', null)).toBe(false)
    })
})
