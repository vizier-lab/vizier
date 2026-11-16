import { describe, expect, it } from 'vitest'
import type { DetachedScriptDetail } from '../api/frames'
import { deliveredHandles, deliveryBody, detachedReport, scriptElapsed } from './detached'

const detail = (over: Partial<DetachedScriptDetail>): DetachedScriptDetail => ({
  id: 'd_1',
  intent: 'check',
  state: 'running',
  created_at: '2026-11-04T09:00:00Z',
  finished_at: null,
  timeout_secs: 600,
  parent_id: null,
  delivered: false,
  tool_call_count: 1,
  code: '1',
  tool_calls: [{ seq: 1, name: 'web_fetch', arguments: { url: 'x' }, ok: true, duration_ms: 1200 }],
  report: null,
  ...over,
})

describe('detachedReport', () => {
  it('shows the calls so far and no result while running', () => {
    const r = detachedReport(detail({}))
    expect(r.tool_calls.map((c) => c.name)).toEqual(['web_fetch'])
    expect(r.result).toBeNull()
    expect(r.error).toBeNull()
  })

  it('draws the stored report once it ended, with the stored calls', () => {
    const r = detachedReport(
      detail({ state: 'succeeded', report: { ok: true, result: { count: 3 }, stdout: 'hi', error: null, duration_ms: 9 } }),
    )
    expect(r.ok).toBe(true)
    expect(r.result).toEqual({ count: 3 })
    expect(r.stdout).toBe('hi')
    expect(r.tool_calls).toHaveLength(1)
  })

  it('turns a stop reason into a limit block', () => {
    const r = detachedReport(detail({ state: 'timed_out', report: "the script's time limit (600s) was reached" }))
    expect(r.error?.kind).toBe('limit')
    expect(r.error?.message).toBe('Ran out of time')
    expect(r.error?.limit).toContain('600s')
    const c = detachedReport(detail({ state: 'cancelled' }))
    expect(c.error?.message).toBe('Cancelled by the agent')
  })
})

describe('scriptElapsed', () => {
  it('counts to the end, or to now while running', () => {
    const s = detail({})
    expect(scriptElapsed(s, Date.parse('2026-11-04T09:00:41Z'))).toBe('0:41')
    expect(scriptElapsed({ ...s, finished_at: '2026-11-04T09:12:05Z' }, 0)).toBe('12:05')
  })
})

describe('deliveries', () => {
  const text = `[Background work finished]
d_k3x9a1b2  succeeded in 41s  — check
result: 1
d_aaaaaaaa  failed in 2s  — parse
error: x`
  it('names each script it carries', () => {
    expect(deliveredHandles(text)).toEqual(['d_k3x9a1b2', 'd_aaaaaaaa'])
  })
  it('drops the heading line', () => {
    expect(deliveryBody(text).startsWith('d_k3x9a1b2')).toBe(true)
  })
})
