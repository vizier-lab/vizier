import { create } from 'zustand'
import { getBackgroundJob, listBackgroundJobs } from '../services/vizier'
import type { BackgroundJobSnapshot } from '../interfaces/types'

// How a job that left `running` is shown before it is removed from the tray.
export type JobFinish = 'done' | 'cancelled' | 'lost'

// How long a finished job lingers in the tray.
const FINISH_LINGER_MS = 3000

interface BackgroundJobState {
  agentId: string | null
  topicId: string | null
  // In-flight jobs of the open topic, plus finished ones still lingering.
  jobs: Map<string, BackgroundJobSnapshot>
  finishing: Map<string, JobFinish>
  // The most recently applied snapshot and a counter, so a component can react to every
  // frame (the piece panel re-fetches on each one; the chat appends a report entry on
  // `reporting`) without diffing the map.
  lastApplied: { job: BackgroundJobSnapshot; n: number } | null
  applySnapshot: (job: BackgroundJobSnapshot) => void
  load: (agentId: string, topicId: string) => Promise<void>
}

const timers = new Map<string, ReturnType<typeof setTimeout>>()

export const useBackgroundJobStore = create<BackgroundJobState>()((set, get) => {
  const finish = (job: BackgroundJobSnapshot, how: JobFinish) => {
    set(state => {
      const jobs = new Map(state.jobs)
      const finishing = new Map(state.finishing)
      jobs.set(job.id, job)
      finishing.set(job.id, how)
      return { jobs, finishing }
    })
    if (timers.has(job.id)) clearTimeout(timers.get(job.id))
    timers.set(
      job.id,
      setTimeout(() => {
        timers.delete(job.id)
        set(state => {
          const jobs = new Map(state.jobs)
          const finishing = new Map(state.finishing)
          jobs.delete(job.id)
          finishing.delete(job.id)
          return { jobs, finishing }
        })
      }, FINISH_LINGER_MS)
    )
  }

  return {
    agentId: null,
    topicId: null,
    jobs: new Map(),
    finishing: new Map(),
    lastApplied: null,

    applySnapshot: (job: BackgroundJobSnapshot) => {
      const state = get()
      set({ lastApplied: { job, n: (state.lastApplied?.n ?? 0) + 1 } })
      const already = state.finishing.get(job.id)

      switch (job.state) {
        case 'running':
          // A late `running` frame cannot bring back a job that is already finishing.
          if (already) return
          set(s => {
            const jobs = new Map(s.jobs)
            jobs.set(job.id, job)
            return { jobs }
          })
          return
        case 'reporting':
          finish(job, 'done')
          return
        case 'reported':
          // Already shown as done from its `reporting` frame; only a job that missed that
          // frame is finished here.
          if (state.jobs.has(job.id) && !already) finish(job, 'done')
          return
        case 'cancelled':
          finish(job, 'cancelled')
          return
        case 'undelivered':
        case 'interrupted':
          finish(job, 'lost')
          return
      }
    },

    load: async (agentId: string, topicId: string) => {
      const sameTopic = get().agentId === agentId && get().topicId === topicId
      if (!sameTopic) {
        timers.forEach(timer => clearTimeout(timer))
        timers.clear()
        set({ agentId, topicId, jobs: new Map(), finishing: new Map() })
      }
      const previous = sameTopic ? get().jobs : new Map<string, BackgroundJobSnapshot>()

      let listed: BackgroundJobSnapshot[]
      try {
        const res = await listBackgroundJobs(agentId, topicId)
        listed = res.data ?? []
      } catch (err) {
        console.error('Failed to load background jobs', err)
        return
      }
      // The topic changed while the request was in flight.
      if (get().agentId !== agentId || get().topicId !== topicId) return

      set(state => {
        const jobs = new Map<string, BackgroundJobSnapshot>()
        for (const job of listed) jobs.set(job.id, job)
        // Keep jobs that are lingering in a finished state until their timer removes them.
        for (const [id, job] of state.jobs) {
          if (state.finishing.has(id) && !jobs.has(id)) jobs.set(id, job)
        }
        return { jobs }
      })

      // A job that was in the tray and is no longer running finished while we were not
      // listening. A report is already in history; anything else was lost.
      const listedIds = new Set(listed.map(job => job.id))
      for (const [id] of previous) {
        if (listedIds.has(id) || get().finishing.has(id)) continue
        try {
          const res = await getBackgroundJob(agentId, topicId, id)
          const job = res.data
          if (job.state === 'interrupted' || job.state === 'undelivered') {
            finish(job, 'lost')
          }
        } catch {
          // 404: the job is gone with its topic; nothing to show.
        }
      }
    },
  }
})
