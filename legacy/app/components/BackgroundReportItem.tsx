// A background report in the conversation: the request that woke the agent with the
// outcome of work it started earlier. It is not a message from a person, so it renders as a
// collapsible divider rather than a user bubble (FR-009, FR-021).

import { useState } from 'react'
import ReactMarkdown from 'react-markdown'
import remarkGfm from 'remark-gfm'
import type { BackgroundPieceState, BackgroundReport } from '../interfaces/types'
import '../styles/background-jobs.css'

export const PIECE_STATE_LABEL: Record<BackgroundPieceState, string> = {
  running: 'running',
  answered: 'answered',
  failed: 'failed',
  timed_out: 'timed out',
  cancelled: 'cancelled',
  interrupted: 'interrupted',
}

export const PIECE_STATE_ICON: Record<BackgroundPieceState, string> = {
  running: '⟳',
  answered: '✓',
  failed: '✕',
  timed_out: '⧖',
  cancelled: '⊘',
  interrupted: '⚠',
}

interface BackgroundReportItemProps {
  report: BackgroundReport
  // Opens a piece's own conversation.
  onOpenPiece?: (jobId: string, ordinal: number) => void
  // Shown while a turn is still running and the report waits its turn.
  queued?: boolean
}

export function BackgroundReportItem({ report, onOpenPiece, queued }: BackgroundReportItemProps) {
  const [expanded, setExpanded] = useState(false)

  const count = (state: BackgroundPieceState) =>
    report.entries.filter((entry) => entry.state === state).length
  const answered = count('answered')
  const failed = count('failed')
  const timedOut = count('timed_out')

  const title =
    report.kind === 'delegation' && report.delegated_to
      ? `Background delegation to ${report.delegated_to}`
      : 'Background batch'

  const toggle = () => setExpanded((open) => !open)

  return (
    <div className="bg-report">
      <div
        className="bg-report-divider"
        onClick={toggle}
        role="button"
        tabIndex={0}
        aria-expanded={expanded}
        onKeyDown={(e) => {
          if (e.key === 'Enter' || e.key === ' ') {
            e.preventDefault()
            toggle()
          }
        }}
      >
        <div className="bg-report-line" />
        <div className="bg-report-label">
          <span aria-hidden="true">⚙</span>
          <span className="bg-report-title">
            {title} <span className="bg-tray-id">{report.job_id}</span> finished
          </span>
          {answered > 0 && (
            <>
              <span className="bg-report-sep">·</span>
              <span className="bg-state--answered">{answered} ✓</span>
            </>
          )}
          {failed > 0 && (
            <>
              <span className="bg-report-sep">·</span>
              <span className="bg-state--failed">{failed} ✕ failed</span>
            </>
          )}
          {timedOut > 0 && (
            <>
              <span className="bg-report-sep">·</span>
              <span className="bg-state--timed_out">{timedOut} ⧖ timed out</span>
            </>
          )}
          {queued && (
            <span className="queued-badge">
              <span className="queued-badge-icon">⏳</span>
              Queued
            </span>
          )}
          <span className={`bg-report-caret ${expanded ? 'bg-report-caret--open' : ''}`}>▸</span>
        </div>
        <div className="bg-report-line" />
      </div>

      {expanded && (
        <div className="bg-report-body">
          {report.entries.map((entry) => (
            <div key={entry.ordinal} className="bg-report-entry">
              <div className="bg-report-entry-head">
                <span className={`bg-state--${entry.state}`}>
                  {PIECE_STATE_ICON[entry.state]} {PIECE_STATE_LABEL[entry.state]}
                </span>
                <span className="bg-report-entry-prompt" title={entry.prompt}>
                  {entry.ordinal + 1}. {entry.prompt}
                </span>
                {onOpenPiece && (
                  <button
                    type="button"
                    className="bg-open-btn"
                    title="Open this piece's conversation"
                    onClick={() => onOpenPiece(report.job_id, entry.ordinal)}
                  >
                    ›
                  </button>
                )}
              </div>
              {entry.text && (
                <div className="prose bg-report-entry-text">
                  <ReactMarkdown remarkPlugins={[remarkGfm]}>{entry.text}</ReactMarkdown>
                </div>
              )}
            </div>
          ))}
        </div>
      )}
    </div>
  )
}

export default BackgroundReportItem
