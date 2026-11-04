import { renderToString } from 'react-dom/server'
import { describe, expect, it } from 'vitest'
import { MemoryRouter } from 'react-router'
import { ChatStatusStrip } from './ChatStatusStrip'
import type { BackgroundJobSnapshot, SessionFileRecord } from '../../api/frames'
import type { Recalled } from '../../lib/recall'

const recalled: Recalled[] = [
  { bundle: 'ops', path: 'on-call.md', ordinal: 1, score: 0.84, origin: 'auto', uid: 'u1' },
]

const job = (state: BackgroundJobSnapshot['state']): BackgroundJobSnapshot => ({
  id: 'j1',
  kind: 'batch',
  delegated_to: null,
  state,
  created_at: '2026-11-04T09:00:00Z',
  finished_at: null,
  timeout_secs: 600,
  pieces: [
    {
      ordinal: 0,
      prompt: 'draft the follow-up',
      state: state === 'running' ? 'running' : 'answered',
      reason: null,
      agent_id: 'a',
      topic: null,
      started_at: '2026-11-04T09:00:00Z',
      finished_at: null,
    },
  ],
})

const file: SessionFileRecord = {
  id: 'f1',
  session_slug: 'vizier-webui',
  agent_id: 'a',
  filename: 'incident-report.pdf',
  mime_type: 'application/pdf',
  size: 1024,
  file_id: 'f1',
  added_at: '2026-11-04T09:00:00Z',
}

const draw = (
  jobs: BackgroundJobSnapshot[] = [],
  files: SessionFileRecord[] = [],
  passages = recalled
) =>
  renderToString(
    <MemoryRouter initialEntries={['/oz/chat/t1']}>
      <ChatStatusStrip
        agentId="oz"
        topicId="t1"
        recalled={passages}
        jobs={jobs}
        files={files}
        canPromote={false}
        onFilesCleared={() => {}}
      />
    </MemoryRouter>
  )

describe("the chat's status strip", () => {
  it('draws a count for what the agent recalled, what it runs and what it left', () => {
    const html = draw([job('running')], [file])
    expect(html).toContain('aria-label="1 recalled passages"')
    expect(html).toContain('aria-label="1 background jobs running"')
    expect(html).toContain('aria-label="1 session files"')
  })

  it('shows every section, even the empty ones', () => {
    const html = draw([], [])
    expect(html).toContain('aria-label="1 recalled passages"')
    expect(html).toContain('aria-label="0 background jobs running"')
    expect(html).toContain('aria-label="0 session files"')
  })

  it('counts only the jobs that are running, and marks them live', () => {
    const html = draw([job('running'), job('reported')])
    expect(html).toContain('aria-label="1 background jobs running"')
    expect(html).toContain('bg-live')
  })

  it('shows zeroes when nothing has happened yet', () => {
    const html = draw([], [], [])
    expect(html).toContain('aria-label="0 recalled passages"')
    expect(html).toContain('aria-label="0 background jobs running"')
    expect(html).toContain('aria-label="0 session files"')
  })
})
