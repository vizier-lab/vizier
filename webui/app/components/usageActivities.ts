import type { UsageActivity } from '../interfaces/types'

export interface ActivityMeta {
  id: UsageActivity
  label: string
  description: string
  /** Neighbouring series differ in lightness, not only hue. */
  color: string
}

/** The one palette the chart, chips, rows and drawer all read (spec 018 contracts/webui.md). */
export const USAGE_ACTIVITIES: ActivityMeta[] = [
  { id: 'chat', label: 'Chat', description: 'Replies to people on Discord, Telegram and the WebUI', color: 'var(--accent-dark)' },
  { id: 'dreams', label: 'Dreams', description: 'Nightly extraction and consolidation', color: '#7c3aed' },
  { id: 'compaction', label: 'Compaction', description: 'Checkpoint summaries of long conversations', color: '#d97706' },
  { id: 'background', label: 'Background', description: 'Subtasks, delegations and consults', color: '#0284c7' },
  { id: 'scheduled', label: 'Scheduled', description: 'Scheduled task runs', color: '#64748b' },
  { id: 'vision', label: 'Vision', description: 'Reading images with the vision model', color: '#db2777' },
  { id: 'other', label: 'Other', description: 'Calls with no known purpose', color: 'var(--color-text-muted)' },
]

export function activityMeta(id: UsageActivity | string): ActivityMeta {
  return USAGE_ACTIVITIES.find((a) => a.id === id) ?? USAGE_ACTIVITIES[USAGE_ACTIVITIES.length - 1]
}

/** Activities that weren't recorded before complete accounting started. */
export const MISSING_BEFORE_ACCOUNTING: UsageActivity[] = ['dreams', 'compaction', 'vision']
