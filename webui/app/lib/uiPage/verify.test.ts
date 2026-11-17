import { describe, expect, it } from 'vitest'
import { MAX_REPORTED, ProblemList, reportFrame, type VerifyCheck } from './verify'

const check: VerifyCheck = { requestId: 'r1', agentId: 'a', topicId: 't', pageId: 'pg_1', version: 2, observeMs: 2000 }

describe('a page check', () => {
    it('keeps problems in order, once each, up to one more than the server keeps', () => {
        const list = new ProblemList()
        list.add({ kind: 'error', message: 'boom', source: 'page', line: 3 })
        list.add({ kind: 'error', message: 'boom', source: 'page', line: 3 })
        list.add({ kind: 'error', message: 'boom', source: 'page', line: 4 })
        expect(list.problems.map((p) => p.line)).toEqual([3, 4])
        for (let i = 0; i < 40; i++) list.add({ kind: 'console', message: `m${i}`, source: 'page' })
        expect(list.problems).toHaveLength(MAX_REPORTED)
    })

    it('reports with its request id and a whole number of milliseconds', () => {
        expect(reportFrame(check, [], 2003.4)).toEqual({ ui_verify_report: { request_id: 'r1', problems: [], observed_ms: 2003 } })
    })
})
