import { expect, it } from 'vitest'
import { runDuration, runStatus, scheduleText, slugOf } from './tasks'

it('names a run by its state, and a task that never ran', () => {
    expect(runStatus(null)).toBe('not-run')
    expect(runStatus({ state: 'no_response' } as never)).toBe('no-response')
    expect(runStatus({ state: 'answered' } as never)).toBe('answered')
})

it('times a run', () => {
    expect(runDuration({ ran_at: '2026-10-20T09:00:00Z', finished_at: '2026-10-20T09:00:41Z' })).toBe('41s')
    expect(runDuration({ ran_at: '2026-10-20T09:00:00Z', finished_at: '2026-10-20T09:01:02Z' })).toBe('1m 02s')
    expect(runDuration({ ran_at: '2026-10-20T09:00:00Z' })).toBe('–')
})

it('shows a cron schedule as it is', () => {
    expect(scheduleText({ CronTask: '0 9 * * 1' })).toBe('0 9 * * 1')
})

it('makes a slug from a title', () => {
    expect(slugOf('Nightly Backup — S3!')).toBe('nightly-backup-s3')
    expect(slugOf('  --  ')).toBe('')
})
