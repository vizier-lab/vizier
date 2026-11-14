import { describe, expect, it } from 'vitest'
import type { SubagentSnapshot } from '../api/frames'
import {
    groupHeading,
    groupWorkers,
    splitDelivery,
    workerCounts,
    workerElapsed,
    workerMeta,
    workerStrip,
    workerTab,
} from './subagents'

const worker = (id: string, over: Partial<SubagentSnapshot> = {}): SubagentSnapshot => ({
    id,
    position: 1,
    group_size: 1,
    task: `task ${id}`,
    state: 'running',
    stop_reason: null,
    question: null,
    result: null,
    truncated: false,
    error: null,
    started_at: '2026-11-14T10:00:00Z',
    ended_at: null,
    topic: `w_${id}`,
    group: 'g_1',
    parent_detached_id: null,
    handed_over: false,
    updated_at: '2026-11-14T10:00:00Z',
    ...over,
})

describe('workerTab', () => {
    it('puts every final state that is not a stop under done', () => {
        expect(workerTab('answered')).toBe('done')
        expect(workerTab('failed')).toBe('done')
        expect(workerTab('timed_out')).toBe('done')
        expect(workerTab('stopped')).toBe('stopped')
        expect(workerTab('asking')).toBe('asking')
    })
})

describe('the strip and the meta line', () => {
    const ws = [worker('a'), worker('b', { state: 'asking' }), worker('c', { state: 'answered' })]

    it('counts the unfinished workers, amber while one asks', () => {
        expect(workerStrip(ws)).toEqual({ count: 2, dot: 'warn' })
        expect(workerStrip([worker('a')])).toEqual({ count: 1, dot: 'live' })
        expect(workerStrip([worker('c', { state: 'answered' })])).toEqual({ count: 0, dot: undefined })
    })

    it('says what is at work, or the total', () => {
        expect(workerMeta(ws)).toBe('1 running · 1 asking')
        expect(workerMeta([worker('c', { state: 'answered' })])).toBe('1')
        expect(workerCounts(ws)).toEqual({ all: 3, running: 1, asking: 1, done: 1, stopped: 0 })
    })
})

describe('groupWorkers', () => {
    const ws = [
        worker('a2', { group: 'g_old', position: 2, group_size: 2, started_at: '2026-11-14T09:00:00Z' }),
        worker('a1', { group: 'g_old', position: 1, group_size: 2, started_at: '2026-11-14T09:00:00Z', state: 'answered' }),
        worker('b1', { group: 'g_new', state: 'asking', question: 'Which sheet?' }),
    ]

    it('groups by spawn, newest spawn first, in position order', () => {
        const groups = groupWorkers(ws, '', 'all')
        expect(groups.map((g) => g.id)).toEqual(['g_new', 'g_old'])
        expect(groups[1].workers.map((w) => w.id)).toEqual(['a1', 'a2'])
    })

    it('filters by tab and query, and drops a group left empty', () => {
        expect(groupWorkers(ws, '', 'done').map((g) => g.id)).toEqual(['g_old'])
        expect(groupWorkers(ws, 'which', 'all').map((g) => g.workers.map((w) => w.id))).toEqual([['b1']])
        expect(groupWorkers(ws, 'nothing', 'all')).toEqual([])
    })

    it('keeps the spawn size in the heading when the filter hides some', () => {
        const [g] = groupWorkers(ws, 'a2', 'all')
        expect(g.size).toBe(2)
        expect(groupHeading(g, Date.parse('2026-11-14T09:01:12Z'))).toBe('2 workers · hired 1:12 ago')
    })
})

describe('workerElapsed', () => {
    it('stops at the end, or runs to now', () => {
        expect(workerElapsed({ started_at: '2026-11-14T10:00:00Z', ended_at: '2026-11-14T10:00:58Z' }, 0)).toBe('0:58')
        expect(
            workerElapsed({ started_at: '2026-11-14T10:00:00Z', ended_at: null }, Date.parse('2026-11-14T10:10:00Z'))
        ).toBe('10:00')
    })
})

describe('splitDelivery', () => {
    it('reads worker updates, dropping the lines addressed to the agent', () => {
        const text = [
            '[Subagent update] s_luogg1my — task 2 of 3 "find the "EU" sheet"',
            'state: asking',
            'question: Which one?',
            'reply with reply_subagent(id="s_luogg1my", text=...)',
            '',
            '[Subagent update] s_bkxogtzf — task 3 of 3 "sum invoices"',
            'state: answered',
            'result: 41 invoices',
            '',
            'six vendors',
            '(read the whole result with get_subagent(id="s_bkxogtzf"))',
        ].join('\n')
        const { scripts, workers } = splitDelivery(text)
        expect(scripts).toBe('')
        expect(workers).toEqual([
            { handle: 's_luogg1my', position: 2, size: 3, title: 'find the "EU" sheet', state: 'asking', body: 'question: Which one?' },
            { handle: 's_bkxogtzf', position: 3, size: 3, title: 'sum invoices', state: 'answered', body: 'result: 41 invoices\n\nsix vendors' },
        ])
    })

    it('keeps the scripts section of a mixed delivery for parseDelivery', () => {
        const { scripts, workers } = splitDelivery(
            '[Background work finished]\nd_k3x9a1b2  succeeded in 0:06  — fetch\nok\n\n[Subagent update] s_a — task 1 of 1 "t"\nstate: failed\nerror: boom'
        )
        expect(scripts).toBe('[Background work finished]\nd_k3x9a1b2  succeeded in 0:06  — fetch\nok')
        expect(workers.map((w) => [w.handle, w.state, w.body])).toEqual([['s_a', 'failed', 'error: boom']])
    })
})
