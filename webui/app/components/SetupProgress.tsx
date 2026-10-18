import { useEffect, useRef, useState } from 'react'
import { createPortal } from 'react-dom'
import {
  FaCircleCheck,
  FaXmark,
  FaRotateRight,
  FaTriangleExclamation,
} from 'react-icons/fa6'
import { useBuildStore, type BuildJob } from '../hooks/buildStore'
import { getSetupState, type DownloadStatus, type SetupState } from '../services/vizier'

const POLL_ACTIVE_MS = 1000
// Idle polls only pick up builds started elsewhere (a boot build, another tab).
const POLL_IDLE_MS = 15000

/** The one loader used everywhere: the blinking dots the chat "thinking" state already uses. */
function Loader() {
  return (
    <span className="thinking-dots setup-loader" aria-hidden>
      <span>.</span>
      <span>.</span>
      <span>.</span>
    </span>
  )
}

function formatBytes(bytes: number): string {
  if (bytes <= 0) return '0 B'
  const units = ['B', 'KB', 'MB', 'GB', 'TB']
  const i = Math.min(units.length - 1, Math.floor(Math.log(bytes) / Math.log(1024)))
  const value = bytes / 1024 ** i
  return `${value >= 100 || i === 0 ? Math.round(value) : value.toFixed(1)} ${units[i]}`
}

function formatDuration(seconds: number): string {
  if (seconds < 60) return `${Math.max(1, Math.round(seconds))}s`
  if (seconds < 3600) return `${Math.round(seconds / 60)} min`
  return `${Math.round(seconds / 3600)}h`
}

/** The download worth showing for a setup: the measurable one if there is one. */
function currentDownload(setup: SetupState | undefined): DownloadStatus | undefined {
  if (!setup || setup.downloads.length === 0) return undefined
  return setup.downloads.find((d) => d.total > 0) ?? setup.downloads[0]
}

/** The one-line message and its detail for a setup. */
function describe(setup: SetupState | undefined, etas: Record<string, number>): {
  label: string
  detail?: string
} {
  const download = currentDownload(setup)
  if (!download) {
    return { label: setup?.message || 'Starting agent…' }
  }
  if (download.error) {
    return { label: download.label, detail: download.error }
  }
  if (download.total > 0) {
    const pct = Math.floor((download.completed / download.total) * 100)
    const eta = etas[`${setup?.agent_id}:${download.kind}`]
    const parts = [`${pct}%`, `${formatBytes(download.completed)} / ${formatBytes(download.total)}`]
    if (eta && eta > 0 && eta < 3600) parts.push(`about ${formatDuration(eta)} left`)
    return { label: download.label, detail: parts.join(' · ') }
  }
  return { label: download.label, detail: 'this may take a moment' }
}

/** Polls the setup state while anything is happening, and renders the modal and the chip. */
export default function SetupProgress() {
  const jobs = useBuildStore((s) => s.jobs)
  const close = useBuildStore((s) => s.close)
  const [setups, setSetups] = useState<SetupState[]>([])
  const [etas, setEtas] = useState<Record<string, number>>({})
  const [dismissed, setDismissed] = useState<Set<string>>(new Set())
  const samples = useRef<Map<string, { completed: number; at: number }>>(new Map())

  const active = jobs.length > 0 || setups.length > 0

  useEffect(() => {
    let cancelled = false
    const poll = async () => {
      let next: SetupState[]
      try {
        next = await getSetupState()
      } catch {
        return
      }
      if (cancelled) return

      const now = Date.now()
      const map: Record<string, number> = {}
      for (const setup of next) {
        for (const download of setup.downloads) {
          const key = `${setup.agent_id}:${download.kind}`
          const prev = samples.current.get(key)
          if (prev && download.total > 0 && download.completed > prev.completed) {
            const rate = (download.completed - prev.completed) / ((now - prev.at) / 1000)
            if (rate > 0) map[key] = Math.round((download.total - download.completed) / rate)
          }
          samples.current.set(key, { completed: download.completed, at: now })
        }
      }
      setEtas(map)
      setSetups(next)
      // Forget dismissals for setups that have finished.
      setDismissed((prev) => {
        if (prev.size === 0) return prev
        const ids = new Set(next.map((s) => s.agent_id))
        const kept = new Set([...prev].filter((id) => ids.has(id)))
        return kept.size === prev.size ? prev : kept
      })
    }
    // A hidden tab doesn't poll; it catches up the moment it is shown again.
    const tick = () => {
      if (document.visibilityState === 'visible') poll()
    }
    tick()
    const timer = setInterval(tick, active ? POLL_ACTIVE_MS : POLL_IDLE_MS)
    document.addEventListener('visibilitychange', tick)
    return () => {
      cancelled = true
      clearInterval(timer)
      document.removeEventListener('visibilitychange', tick)
    }
  }, [active])

  const visibleJob = jobs[0]
  const modalAgentIds = new Set(jobs.map((j) => j.agentId))
  const chipSetups = setups.filter(
    (s) =>
      (s.phase === 'starting' || s.phase === 'downloading') &&
      !modalAgentIds.has(s.agent_id) &&
      !dismissed.has(s.agent_id),
  )

  return createPortal(
    <>
      {visibleJob && (
        <SetupModal
          job={visibleJob}
          setup={setups.find((s) => s.agent_id === visibleJob.agentId)}
          etas={etas}
          onClose={() => close(visibleJob.id)}
          onRetry={
            visibleJob.retry
              ? () => {
                  close(visibleJob.id)
                  visibleJob.retry?.()
                }
              : undefined
          }
        />
      )}
      {chipSetups.length > 0 && (
        <div className="setup-chips">
          {chipSetups.slice(0, 4).map((setup) => {
            const { label, detail } = describe(setup, etas)
            return (
              <div className="setup-chip" key={setup.agent_id}>
                <Loader />
                <div className="setup-chip-text">
                  <span className="setup-chip-label">{label}</span>
                  {detail && <span className="setup-chip-detail">{detail}</span>}
                </div>
                <button
                  className="setup-chip-close"
                  aria-label="Hide"
                  onClick={() =>
                    setDismissed((prev) => new Set(prev).add(setup.agent_id))
                  }
                >
                  <FaXmark size={12} />
                </button>
              </div>
            )
          })}
          {chipSetups.length > 4 && (
            <div className="setup-chip-more">+{chipSetups.length - 4} more</div>
          )}
        </div>
      )}
    </>,
    document.body,
  )
}

function SetupModal({
  job,
  setup,
  etas,
  onClose,
  onRetry,
}: {
  job: BuildJob
  setup: SetupState | undefined
  etas: Record<string, number>
  onClose: () => void
  onRetry?: () => void
}) {
  useEffect(() => {
    const onKey = (e: KeyboardEvent) => {
      if (e.key === 'Escape') onClose()
    }
    document.addEventListener('keydown', onKey)
    return () => document.removeEventListener('keydown', onKey)
  }, [onClose])

  const { label, detail } = describe(setup, etas)

  return (
    <div className="setup-overlay" onClick={onClose}>
      <div
        className="setup-modal"
        role="dialog"
        aria-modal="true"
        aria-label={job.title}
        onClick={(e) => e.stopPropagation()}
      >
        <div className="setup-modal-header">
          <span className="setup-modal-title">{job.title}</span>
          <button className="setup-modal-close" aria-label="Close" onClick={onClose}>
            <FaXmark size={14} />
          </button>
        </div>

        <div className="setup-modal-body">
          {job.phase === 'ready' ? (
            <div className="setup-state setup-state-ready">
              <FaCircleCheck size={22} />
              <span>Agent ready</span>
            </div>
          ) : job.phase === 'failed' ? (
            <div className="setup-state setup-state-failed">
              <FaTriangleExclamation size={22} />
              <span className="setup-modal-label">Couldn’t start agent</span>
              {job.error && <span className="setup-modal-detail">{job.error}</span>}
            </div>
          ) : (
            <div className="setup-state">
              <Loader />
              <span className="setup-modal-label">{label}</span>
              {detail && <span className="setup-modal-detail">{detail}</span>}
            </div>
          )}
        </div>

        {job.phase !== 'ready' && (
          <div className="setup-modal-footer">
            {job.phase === 'failed' ? (
              <>
                {onRetry && (
                  <button className="btn btn-primary" onClick={onRetry}>
                    <FaRotateRight size={12} /> Retry
                  </button>
                )}
                <button className="btn btn-ghost" onClick={onClose}>
                  Close
                </button>
              </>
            ) : (
              <button className="btn btn-ghost" onClick={onClose}>
                Continue in background
              </button>
            )}
          </div>
        )}
      </div>
    </div>
  )
}
