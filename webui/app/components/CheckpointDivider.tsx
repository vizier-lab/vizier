import React from 'react'
import { FaBookmark, FaChevronDown } from 'react-icons/fa6'
import ReactMarkdown from 'react-markdown'
import remarkGfm from 'remark-gfm'
import '../styles/checkpoint.css'

interface CheckpointDividerProps {
  handover: string | null
  timestamp: string
}

function formatTimestamp(ts: string): string {
  try {
    const date = new Date(ts)
    return new Intl.DateTimeFormat(undefined, {
      month: 'short',
      day: 'numeric',
      hour: 'numeric',
      minute: '2-digit',
    }).format(date)
  } catch {
    return ts
  }
}

export function CheckpointDivider({ handover, timestamp }: CheckpointDividerProps) {
  const [expanded, setExpanded] = React.useState(false)
  const hasHandover = handover != null && handover.length > 0

  return (
    <div className={`checkpoint-wrapper ${expanded ? 'checkpoint-wrapper--expanded' : ''}`}>
      <div
        className="checkpoint-divider checkpoint-divider--clickable"
        onClick={() => setExpanded(!expanded)}
        role="button"
        tabIndex={0}
        onKeyDown={(e) => {
          if (e.key === 'Enter' || e.key === ' ') {
            e.preventDefault()
            setExpanded(!expanded)
          }
        }}
      >
        <div className="checkpoint-line" />
        <div className="checkpoint-label">
          <FaBookmark className="checkpoint-icon" />
          <span className="checkpoint-text">Context checkpoint</span>
          <span className="checkpoint-sep">·</span>
          <span className="checkpoint-time">{formatTimestamp(timestamp)}</span>
          <FaChevronDown className={`checkpoint-chevron ${expanded ? 'checkpoint-chevron--open' : ''}`} />
        </div>
        <div className="checkpoint-line" />
      </div>
      {expanded && (
        <div className="checkpoint-expanded">
          <div className="checkpoint-expanded-header">Handover Summary</div>
          {hasHandover ? (
            <div className="checkpoint-expanded-content">
              <ReactMarkdown remarkPlugins={[remarkGfm]}>{handover}</ReactMarkdown>
            </div>
          ) : (
            <div className="checkpoint-expanded-content checkpoint-expanded-empty">
              No summary available for this checkpoint.
            </div>
          )}
        </div>
      )}
    </div>
  )
}
