// The background jobs launched from the open topic, docked above the message input
// (contracts/webui-tray.md). Renders nothing while there are none.

import { useEffect, useState, useSyncExternalStore } from 'react'
import { useBackgroundJobStore, type JobFinish } from '../hooks/backgroundJobStore'
import { cancelBackgroundJob } from '../services/vizier'
import type { BackgroundJobSnapshot } from '../interfaces/types'
import { PIECE_STATE_ICON, PIECE_STATE_LABEL } from './BackgroundReportItem'
import '../styles/background-jobs.css'

const PHONE_QUERY = '(max-width: 639px)'

function useIsPhone(): boolean {
  return useSyncExternalStore(
    (onChange) => {
      const media = window.matchMedia(PHONE_QUERY)
      media.addEventListener('change', onChange)
      return () => media.removeEventListener('change', onChange)
    },
    () => window.matchMedia(PHONE_QUERY).matches,
    () => false
  )
}

function elapsed(from: string, to: string | null, now: number): string {
  const end = to ? new Date(to).getTime() : now
  const secs = Math.max(0, Math.floor((end - new Date(from).getTime()) / 1000))
  return `${Math.floor(secs / 60)}:${String(secs % 60).padStart(2, '0')}`
}

function finishLabel(job: BackgroundJobSnapshot, how: JobFinish): string {
  if (how === 'cancelled') return '⊘ cancelled'
  if (how === 'lost') return '⚠ lost'
  const count = (state: string) => job.pieces.filter((p) => p.state === state).length
  const parts = [
    count('answered') && `${count('answered')} answered`,
    count('failed') && `${count('failed')} failed`,
    count('timed_out') && `${count('timed_out')} timed out`,
  ].filter(Boolean)
  return `✓ ${parts.join(' · ') || 'finished'}`
}

type CancelStage = 'confirm' | 'cancelling'

interface BackgroundJobTrayProps {
  agentId: string
  topicId: string
  onJump: (jobId: string) => void
  onOpenPiece: (jobId: string, ordinal: number) => void
}

export function BackgroundJobTray({ agentId, topicId, onJump, onOpenPiece }: BackgroundJobTrayProps) {
  const jobs = useBackgroundJobStore((s) => s.jobs)
  const finishing = useBackgroundJobStore((s) => s.finishing)
  const storeTopic = useBackgroundJobStore((s) => s.topicId)
  const storeAgent = useBackgroundJobStore((s) => s.agentId)
  const applySnapshot = useBackgroundJobStore((s) => s.applySnapshot)
  const isPhone = useIsPhone()

  const [expanded, setExpanded] = useState<Set<string>>(new Set())
  const [cancelStage, setCancelStage] = useState<Record<string, CancelStage>>({})
  const [sheetOpen, setSheetOpen] = useState(false)
  const [now, setNow] = useState(() => Date.now())

  // One shared timer for every elapsed label, running only while there is something to time.
  const hasJobs = jobs.size > 0
  useEffect(() => {
    if (!hasJobs) return
    setNow(Date.now())
    const timer = setInterval(() => setNow(Date.now()), 1000)
    return () => clearInterval(timer)
  }, [hasJobs])

  useEffect(() => {
    if (!hasJobs) setSheetOpen(false)
  }, [hasJobs])

  // The store follows the socket's topic; until it has loaded this one, show nothing.
  if (!hasJobs || storeAgent !== agentId || storeTopic !== topicId) return null

  const list = [...jobs.values()].sort(
    (a, b) => new Date(a.created_at).getTime() - new Date(b.created_at).getTime()
  )

  const toggle = (id: string) =>
    setExpanded((prev) => {
      const next = new Set(prev)
      if (next.has(id)) next.delete(id)
      else next.add(id)
      return next
    })

  const setStage = (id: string, stage: CancelStage | null) =>
    setCancelStage((prev) => {
      const next = { ...prev }
      if (stage) next[id] = stage
      else delete next[id]
      return next
    })

  const cancel = async (id: string) => {
    setStage(id, 'cancelling')
    try {
      const { data } = await cancelBackgroundJob(agentId, topicId, id)
      if (data?.data) applySnapshot(data.data)
    } catch (err) {
      console.error('Failed to cancel background job', err)
    } finally {
      setStage(id, null)
    }
  }

  const renderJob = (job: BackgroundJobSnapshot) => {
    const how = finishing.get(job.id)
    const open = expanded.has(job.id)
    const stage = cancelStage[job.id]
    const done = job.pieces.filter((p) => p.state !== 'running').length
    const total = job.pieces.length
    const cancellable = !how && job.state === 'running'

    return (
      <div key={job.id} className="bg-tray-job">
        <div
          className="bg-tray-row bg-tray-row--head"
          onClick={() => toggle(job.id)}
          role="button"
          tabIndex={0}
          aria-expanded={open}
          onKeyDown={(e) => {
            if (e.key === 'Enter' || e.key === ' ') {
              e.preventDefault()
              toggle(job.id)
            }
          }}
        >
          <span className="bg-tray-title">
            {how ? (
              <span className={`bg-state--${how === 'done' ? 'answered' : how}`}>
                {job.kind === 'delegation' ? `Delegated to ${job.delegated_to}` : 'Batch'}{' '}
                <span className="bg-tray-id">{job.id}</span> {finishLabel(job, how)}
              </span>
            ) : job.kind === 'delegation' ? (
              <>
                <span className="bg-spin">⟳</span>
                <span>Delegated to {job.delegated_to}</span>
                <span className="bg-muted">·</span>
                <span className="bg-tray-elapsed">{elapsed(job.created_at, job.finished_at, now)}</span>
              </>
            ) : (
              <>
                <span className="bg-spin">⟳</span>
                <span>Batch</span>
                <span className="bg-tray-id">{job.id}</span>
                <span className="bg-progress" aria-hidden="true">
                  <span className="bg-progress-fill" style={{ width: `${(done / Math.max(total, 1)) * 100}%` }} />
                </span>
                <span>
                  {done}/{total} done
                </span>
                <span className="bg-muted">·</span>
                <span className="bg-tray-elapsed">{elapsed(job.created_at, job.finished_at, now)}</span>
              </>
            )}
          </span>

          <span onClick={(e) => e.stopPropagation()} style={{ display: 'flex', gap: 4 }}>
            <button type="button" className="bg-tray-btn" title="Jump to where this job started" onClick={() => onJump(job.id)}>
              ↥ jump
            </button>
            {cancellable && stage === 'cancelling' && <span className="bg-muted">cancelling…</span>}
            {cancellable && stage === 'confirm' && (
              <span className="bg-tray-confirm">
                <span>Cancel this job?</span>
                <button type="button" className="bg-tray-btn" onClick={() => setStage(job.id, null)}>
                  Keep
                </button>
                <button type="button" className="bg-tray-btn bg-tray-btn--danger" onClick={() => cancel(job.id)}>
                  Cancel job
                </button>
              </span>
            )}
            {cancellable && !stage && (
              <button type="button" className="bg-tray-btn" title="Cancel this job" onClick={() => setStage(job.id, 'confirm')}>
                ✕
              </button>
            )}
          </span>
        </div>

        {open && (
          <div className="bg-tray-pieces">
            {job.pieces.map((piece) => (
              <div key={piece.ordinal} className="bg-tray-piece">
                <span className={`bg-state--${piece.state}`}>
                  {piece.state === 'running' ? <span className="bg-spin">⟳</span> : PIECE_STATE_ICON[piece.state]}
                </span>
                <span className="bg-tray-piece-prompt" title={piece.prompt}>
                  {piece.prompt}
                </span>
                <span className={`bg-state--${piece.state}`}>{PIECE_STATE_LABEL[piece.state]}</span>
                <span className="bg-tray-elapsed">{elapsed(piece.started_at, piece.finished_at, now)}</span>
                <button
                  type="button"
                  className="bg-open-btn"
                  title="Open this piece's conversation"
                  onClick={() => onOpenPiece(job.id, piece.ordinal)}
                >
                  ›
                </button>
              </div>
            ))}
          </div>
        )}
      </div>
    )
  }

  const tray = <div className="bg-tray">{list.map(renderJob)}</div>

  if (isPhone) {
    const running = list.filter((job) => !finishing.has(job.id)).length
    return (
      <>
        <button type="button" className="bg-tray-pill" onClick={() => setSheetOpen(true)}>
          {running > 0 ? (
            <>
              <span className="bg-spin">⟳</span> {running} running
            </>
          ) : (
            'Background jobs'
          )}
        </button>
        {sheetOpen && (
          <>
            <div className="bg-sheet-backdrop" onClick={() => setSheetOpen(false)} />
            <div className="bg-sheet" role="dialog" aria-label="Background jobs">
              {tray}
            </div>
          </>
        )}
      </>
    )
  }

  return tray
}

export default BackgroundJobTray
