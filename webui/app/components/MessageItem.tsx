import { memo } from 'react'
import ReactMarkdown from 'react-markdown'
import remarkGfm from 'remark-gfm'
import rehypeHighlight from 'rehype-highlight'
import { FiCopy } from 'react-icons/fi'
import type { VizierResponseStats } from '../interfaces/types'

interface MessageItemProps {
  uid: string
  isUserMessage: boolean
  senderName: string
  content: string
  stats?: VizierResponseStats
  onCopy: (content: string) => void
}

function MessageItemComponent({
  uid,
  isUserMessage,
  senderName,
  content,
  stats,
  onCopy,
}: MessageItemProps) {
  return (
    <div
      style={{
        display: 'flex',
        flexDirection: 'column',
        gap: '8px',
      }}
    >
      <div style={{
        display: 'flex',
        alignItems: 'center',
        gap: '8px',
      }}>
        <div style={{
          fontWeight: '600',
          fontSize: '14px',
          color: isUserMessage ? 'var(--text-primary)' : 'var(--accent-primary)',
        }}>
          {senderName}
        </div>
      </div>
      <div style={{
        padding: '12px 16px',
        background: isUserMessage ? 'var(--surface)' : 'transparent',
        borderRadius: '8px',
        borderLeft: isUserMessage ? 'none' : '3px solid var(--accent-primary)',
        boxShadow: isUserMessage ? 'var(--shadow-sm)' : 'none',
      }}>
        <div className="flex items-start justify-between">
          <div className='prose'>
            <ReactMarkdown remarkPlugins={[remarkGfm]} rehypePlugins={[rehypeHighlight]}>
              {content}
            </ReactMarkdown>
          </div>
          <button
            onClick={() => onCopy(content)}
            className='sticky border flex items-center justify-center mt-1!'
            style={{
              color: 'var(--text-tertiary)',
            }}
            title="Copy to clipboard"
          >
            <FiCopy size={14} />
          </button>
        </div>

        {!isUserMessage && stats && (
          <div
            title={`Input: ${stats.total_input_tokens} | Output: ${stats.total_output_tokens} | Cached: ${stats.total_cached_input_tokens} `}
            style={{
              marginTop: '8px',
              padding: '4px 8px',
              background: 'var(--surface)',
              borderRadius: '4px',
              display: 'inline-flex',
              alignItems: 'center',
              gap: '8px',
              fontSize: '11px',
              color: 'var(--text-tertiary)',
            }}
          >
            <span>{stats.total_tokens} tokens</span>
            <span style={{ opacity: 0.5 }}>·</span>
            <span>in: {stats.total_input_tokens}</span>
            <span style={{ opacity: 0.5 }}>·</span>
            <span>out: {stats.total_output_tokens}</span>
            <span style={{ opacity: 0.5 }}>·</span>
            <span>{Math.round(stats.duration.secs * 1000 + stats.duration.nanos / 1000000)}ms</span>
          </div>
        )}
      </div>
    </div>
  )
}

// Memoize component to prevent re-renders when parent re-renders
// Only re-render if the message UID changes
export const MessageItem = memo(MessageItemComponent, (prevProps, nextProps) => {
  return prevProps.uid === nextProps.uid
})
