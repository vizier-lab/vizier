import { renderToString } from 'react-dom/server'
import { describe, expect, it, vi } from 'vitest'
import { MemoryRouter } from 'react-router'
import { ChatStatusStrip } from './ChatStatusStrip'
import type { DetachedScriptSnapshot, SessionFileRecord } from '../../api/frames'
import type { Recalled } from '../../lib/recall'

// renderToString reads a zustand store's initial state, so the strip is drawn against a stand-in.
const fake = vi.hoisted(() => ({
  state: { lsp: {} as Record<string, unknown>, watchLsp: () => () => {} },
}))
vi.mock('../../stores/projects', () => ({
  useProjects: Object.assign((select: (s: typeof fake.state) => unknown) => select(fake.state), {
    getState: () => fake.state,
  }),
}))

const recalled: Recalled[] = [
  { bundle: 'ops', path: 'on-call.md', ordinal: 1, score: 0.84, origin: 'auto', uid: 'u1' },
]

const job = (state: DetachedScriptSnapshot['state']): DetachedScriptSnapshot => ({
  id: `d_${state}`,
  intent: 'run the tests',
  state,
  created_at: '2026-11-04T09:00:00Z',
  finished_at: state === 'running' ? null : '2026-11-04T09:01:00Z',
  timeout_secs: 600,
  parent_id: null,
  delivered: state !== 'running',
  tool_call_count: 1,
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
  jobs: DetachedScriptSnapshot[] = [],
  files: SessionFileRecord[] = [],
  passages = recalled
) =>
  renderToString(
    <MemoryRouter initialEntries={['/oz/chat/t1']}>
      <ChatStatusStrip
        agentId="oz"
        topicId="t1"
        recalled={passages}
        scripts={jobs}
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
    expect(html).toContain('aria-label="1 background work running"')
    expect(html).toContain('aria-label="1 session files"')
  })

  it('shows every section, even the empty ones', () => {
    const html = draw([], [])
    expect(html).toContain('aria-label="1 recalled passages"')
    expect(html).toContain('aria-label="0 background work running"')
    expect(html).toContain('aria-label="0 session files"')
  })

  it('counts only the scripts that are running, and marks them live', () => {
    const html = draw([job('running'), job('succeeded')])
    expect(html).toContain('aria-label="1 background work running"')
    expect(html).toContain('bg-live')
  })

  it('shows zeroes when nothing has happened yet', () => {
    const html = draw([], [], [])
    expect(html).toContain('aria-label="0 recalled passages"')
    expect(html).toContain('aria-label="0 background work running"')
    expect(html).toContain('aria-label="0 session files"')
  })
})

describe('the language-server item', () => {
  const server = (state: 'ready' | 'starting' | 'missing' | 'failed', language = 'rust') => ({
    language,
    name: language,
    root: '.',
    state,
    detail: null,
    reason: null,
    install_hint: null,
    overridden: false,
  })
  const drawProject = (lsp?: { projectId: string; enabled: boolean }) => {
    return renderToString(
      <MemoryRouter initialEntries={['/oz/projects/p1/chat/t1']}>
        <ChatStatusStrip
          agentId="oz"
          topicId="t1"
          recalled={[]}
          scripts={[]}
          files={[]}
          canPromote={false}
          onFilesCleared={() => {}}
          lsp={lsp}
        />
      </MemoryRouter>
    )
  }

  it('is absent from an ordinary chat', () => {
    expect(draw()).not.toContain('language servers')
  })

  it('reads the ready servers and carries the worst state as its dot', () => {
    fake.state.lsp = {
        p1: {
          project_id: 'p1',
          enabled: true,
          capacity: { in_use: 2, max: 6 },
          can_manage: true,
          servers: [server('ready'), server('ready', 'go'), server('missing', 'lua')],
        },
    }
    const html = drawProject({ projectId: 'p1', enabled: true })
    expect(html).toContain('aria-label="2 language servers ready"')
    expect(html).toContain('bg-retrying')
  })

  it('reads off when the switch is off', () => {
    fake.state.lsp = {}
    const html = drawProject({ projectId: 'p1', enabled: false })
    expect(html).toContain('aria-label="off: language servers off"')
    expect(html).not.toContain('bg-failed')
  })

  it('shows zero before the first read', () => {
    fake.state.lsp = {}
    expect(drawProject({ projectId: 'p1', enabled: true })).toContain('aria-label="0 language servers ready"')
  })
})
