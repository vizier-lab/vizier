import { create } from 'zustand'

/**
 * The lifecycle of the modal shown while an agent is created or updated. The route that
 * submits the form calls `begin` (which shows the modal at once), then `ready` or `fail`
 * once the blocking API call returns. A fast build still shows the modal briefly before it
 * auto-closes, so submitting always gives visible feedback.
 */
export type BuildPhase = 'pending' | 'ready' | 'failed'

export interface BuildJob {
  id: string
  agentId: string
  title: string
  phase: BuildPhase
  error?: string
  /** Re-run the submit, offered on failure. */
  retry?: () => void
}

/**
 * Whether a create/update error came from building the agent (worth a Retry in the modal),
 * rather than from a refused request (a taken id, a bad field), which belongs on the form.
 * These are the messages `VizierAgents::finish_build` (src/agents/mod.rs) answers with.
 */
export function isBuildFailure(message: string): boolean {
  return (
    message.startsWith('failed to start agent') ||
    message.startsWith('the new config failed to start')
  )
}

/** How long "Agent ready" stays up before the modal closes itself. */
const READY_LINGER_MS = 900

const timers = new Map<string, ReturnType<typeof setTimeout>>()

interface BuildStore {
  jobs: BuildJob[]
  begin: (agentId: string, title: string, retry?: () => void) => string
  ready: (id: string) => void
  fail: (id: string, error: string) => void
  close: (id: string) => void
}

export const useBuildStore = create<BuildStore>()((set, get) => ({
  jobs: [],
  begin: (agentId, title, retry) => {
    const id = `${agentId}-${Date.now()}-${Math.random().toString(36).slice(2, 6)}`
    set((s) => ({ jobs: [...s.jobs, { id, agentId, title, phase: 'pending', retry }] }))
    return id
  },
  ready: (id) => {
    set((s) => ({ jobs: s.jobs.map((j) => (j.id === id ? { ...j, phase: 'ready' } : j)) }))
    const timer = setTimeout(() => {
      timers.delete(id)
      get().close(id)
    }, READY_LINGER_MS)
    timers.set(id, timer)
  },
  fail: (id, error) => {
    set((s) => ({
      jobs: s.jobs.map((j) => (j.id === id ? { ...j, phase: 'failed', error } : j)),
    }))
  },
  close: (id) => {
    const timer = timers.get(id)
    if (timer) {
      clearTimeout(timer)
      timers.delete(id)
    }
    set((s) => ({ jobs: s.jobs.filter((j) => j.id !== id) }))
  },
}))
