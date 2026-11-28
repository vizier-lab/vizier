import { describe, expect, it } from 'vitest'
import type { KindField, Trigger } from '../api/endpoints/triggers'
import {
    allowsMessaging, counts, fieldValues, holdSecret, latest, matches, outcome, scopeFor, scopeMode,
    settingsFrom, sorted, takeSecret, toggleScope,
} from './triggers'

const now = Date.parse('2026-01-02T12:00:00Z')
const trigger = (patch: Partial<Trigger> = {}): Trigger => ({
    id: 'trg', agent_id: 'a', slug: 'ci', kind: 'webhook', title: 'CI failures', instruction: 'Tell me',
    settings: {}, requester: { user: 'admin' }, is_active: true, tool_scope: ['memory_read'], overlap: 'wait',
    hourly_limit: 20, cooldown_secs: 0, triage: null, destination: null, summary_hours: 5,
    created_at: '2026-01-01T00:00:00Z', allows_messaging: false, runs_last_hour: 0, needs_person: false, ...patch,
})
const health = { failures: 0, last_check_at: '2026-01-02T11:59:00Z' }
const run = { id: 1, started_at: '2026-01-02T11:48:00Z', state: 'answered' as const }

describe('the list', () => {
    it('says what most needs saying about a trigger', () => {
        expect(latest(trigger({ awaiting_approval: true, latest_run: run }), now).label).toBe('Waiting for approval')
        expect(latest(trigger({ monitor: { failures: 10, failed_at: 'x' } }), now)).toMatchObject({ state: 'failed', detail: '10 failed checks' })
        expect(latest(trigger({ needs_attention: 'its triage model is not available' }), now).state).toBe('retrying')
        expect(latest(trigger({ monitor: { ...health, failures: 1, last_error: 'boom' }, latest_run: run }), now).label).toBe('Check failing')
        expect(latest(trigger({ latest_run: run }), now).label).toBe('Answered')
        expect(latest(trigger({ latest_run: { ...run, quiet_note: 'all green' } }), now).label).toBe('Nothing to report')
        expect(latest(trigger({ monitor: health }), now).label).toBe('Quiet')
        expect(latest(trigger({ is_active: false }), now).label).toBe('Paused')
    })

    it('counts and filters what is active and what needs a person', () => {
        const all = [
            trigger(),
            trigger({ slug: 'b', title: 'Disk', monitor: { failures: 10, failed_at: 'x' }, needs_person: true }),
            trigger({ slug: 'c', title: 'Release', awaiting_approval: true, is_active: false, needs_person: true }),
        ]
        expect(counts(all)).toEqual({ all: 3, active: 1, needs_you: 2 })
        expect(all.filter((t) => matches(t, 'needs_you', 'disk'))).toHaveLength(1)
        expect(sorted(all, 'name').map((t) => t.title)).toEqual(['CI failures', 'Disk', 'Release'])
        expect(sorted([all[0], trigger({ slug: 'z', title: 'Z', last_fired_at: run.started_at })], 'last_fired')[0].title).toBe('Z')
    })

    it('counts collapsed drops on their badge', () => {
        expect(outcome({ status: 'dropped', count: 14 })).toEqual({ label: 'Dropped ×14', tone: 'retrying' })
        expect(outcome({ status: 'handled', count: 1 }).label).toBe('Handled')
    })
})

describe('tool access', () => {
    it('keeps the three forms apart', () => {
        expect(scopeMode(null)).toBe('every')
        expect(scopeMode(['memory_read'])).toBe('only')
        expect(scopeMode([])).toBe('only')
        expect(scopeMode(['!telegram'])).toBe('except')
        expect(scopeFor('every')).toBeNull()
        expect(toggleScope(['!telegram'], 'shell', true, 'except')).toEqual(['!telegram', '!shell'])
        expect(toggleScope(['web', 'my_mcp'], 'web', false, 'only')).toEqual(['my_mcp'])
    })

    it('knows when runs may message, as the server does', () => {
        expect(allowsMessaging(null)).toBe(true)
        expect(allowsMessaging([])).toBe(false)
        expect(allowsMessaging(['memory_read'])).toBe(false)
        expect(allowsMessaging(['telegram'])).toBe(true)
        expect(allowsMessaging(scopeFor('except'))).toBe(false)
        expect(allowsMessaging(['!telegram'])).toBe(true)
    })
})

describe('the settings form', () => {
    const fields: KindField[] = [
        { key: 'token', label: 'Secret', type: 'secret', generated: true },
        { key: 'signing_secret', label: 'Signing secret', type: 'secret' },
        { key: 'filter', label: 'Filter', type: 'string_list' },
        { key: 'interval_secs', label: 'Seconds', type: 'number', default: 60 as never },
        { key: 'script', label: 'Script', type: 'code' },
        { key: 'tools', label: 'Tools', type: 'tool_categories' },
    ]

    it('never shows a secret and sends lists and numbers as such', () => {
        const saved = { token: '••••1234', signing_secret: '••••abcd', filter: ['a=1', 'b=2'], extra: 1 }
        const values = fieldValues(fields, saved)
        expect(values).toEqual({ signing_secret: '', filter: 'a=1\nb=2', interval_secs: '60', script: '', tools: [] })
        expect(settingsFrom(fields, { ...values, script: '  x = 1', tools: ['web'] }, saved)).toEqual({
            filter: ['a=1', 'b=2'], script: '  x = 1', tools: ['web'], extra: 1,
        })
        expect(settingsFrom(fields, { ...values, interval_secs: '90' }, saved)).toMatchObject({ interval_secs: 90 })
        // A saved value stays even when it equals the default.
        expect(settingsFrom(fields, values, { interval_secs: 60 })).toMatchObject({ interval_secs: 60 })
        expect(settingsFrom(fields, { ...values, signing_secret: 'new', filter: '' }, saved)).toMatchObject({ signing_secret: 'new' })
        expect(settingsFrom(fields, { ...values, filter: ' ' }, saved)).not.toHaveProperty('filter')
    })
})

describe('a secret shown once', () => {
    it('is handed over once, to its own page', () => {
        holdSecret('a', 'ci', 's3cret')
        expect(takeSecret('a', 'other')).toBeNull()
        expect(takeSecret('a', 'ci')).toBe('s3cret')
        expect(takeSecret('a', 'ci')).toBeNull()
    })
})
