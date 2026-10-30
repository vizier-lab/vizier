import type { ScheduleFilter } from '../../lib/schedule'

export const FILTER_LABELS: Record<ScheduleFilter, string> = {
    active: 'Active',
    last_run_failed: 'Last run failed',
    all: 'All',
}
