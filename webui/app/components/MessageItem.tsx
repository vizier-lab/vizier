import { memo } from 'react'
import ReactMarkdown from 'react-markdown'
import remarkGfm from 'remark-gfm'
import rehypeHighlight from 'rehype-highlight'
import { FaCopy, FaFile, FaFilePdf, FaFileImage, FaFileLines } from 'react-icons/fa6'
import type { VizierAttachment, VizierResponseStats } from '../interfaces/types'
import { base_url } from '~/services/vizier'

interface MessageItemProps {
  uid: string
  isUserMessage: boolean
  senderName: string
  content: string
  stats?: VizierResponseStats
  onCopy: (content: string) => void
  attachments?: VizierAttachment[]
}

function MessageItemComponent({
  uid,
  isUserMessage,
  senderName,
  content,
  stats,
  onCopy,
  attachments,
}: MessageItemProps) {
  const isImage = (filename: string) => /\.(jpg|jpeg|png|gif|webp)$/i.test(filename)

  const getFileIcon = (filename: string) => {
    if (/\.pdf$/i.test(filename)) return <FaFilePdf size={16} />
    if (/\.(jpg|jpeg|png|gif|webp)$/i.test(filename)) return <FaFileImage size={16} />
    if (/\.(doc|docx|txt|rtf)$/i.test(filename)) return <FaFileLines size={16} />
    return <FaFile size={16} />
  }

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
            <FaCopy size={14} />
          </button>
        </div>

        {attachments && attachments.length > 0 && (
          <div style={{
            marginTop: '12px',
            display: 'flex',
            flexDirection: 'column',
            gap: '8px',
          }}>
            {attachments.map((att, idx) => {
              if (att.content.url) {
                if (isImage(att.filename)) {
                  return (
                    <div key={idx} style={{ display: 'flex', flexDirection: 'column', gap: '4px' }}>
                      <img
                        src={(att.content.url?.startsWith(`http://${base_url}`) ? '' : `http://${base_url}`) + `${att.content.url}`}
                        alt={att.filename}
                        style={{
                          maxWidth: '300px',
                          maxHeight: '200px',
                          borderRadius: '8px',
                          objectFit: 'cover',
                        }}
                      />
                      <span style={{
                        fontSize: '12px',
                        color: 'var(--text-tertiary)',
                        display: 'flex',
                        alignItems: 'center',
                        gap: '4px',
                      }}>
                        📎 {att.filename}
                      </span>
                    </div>
                  )
                }
                return (
                  <div
                    key={idx}
                    style={{
                      display: 'flex',
                      alignItems: 'center',
                      gap: '8px',
                      padding: '8px 12px',
                      background: 'var(--surface)',
                      borderRadius: '6px',
                      fontSize: '13px',
                    }}
                  >
                    {getFileIcon(att.filename)}
                    <span>{att.filename}</span>
                  </div>
                )
              }
              return null
            })}
          </div>
        )}

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
// Only re-render if the message UID or attachments change
export const MessageItem = memo(MessageItemComponent, (prevProps, nextProps) => {
  if (prevProps.uid !== nextProps.uid) return false
  if (prevProps.content !== nextProps.content) return false
  if (prevProps.stats !== nextProps.stats) return false
  if (prevProps.attachments !== nextProps.attachments) return false
  return true
})
