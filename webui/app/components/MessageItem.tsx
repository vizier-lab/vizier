import { memo } from 'react'
import ReactMarkdown from 'react-markdown'
import remarkGfm from 'remark-gfm'
import rehypeHighlight from 'rehype-highlight'
import { FaCopy, FaFile, FaFilePdf, FaFileImage, FaFileLines } from 'react-icons/fa6'
import type { VizierAttachment, VizierResponseStats } from '../interfaces/types'
import { base_url, api_protocol } from '~/services/vizier'

interface MessageItemProps {
  uid: string
  isUserMessage: boolean
  senderName: string
  content: string
  stats?: VizierResponseStats
  onCopy: (content: string) => void
  onPreviewAttachment?: (attachment: VizierAttachment) => void
  attachments?: VizierAttachment[]
}

function MessageItemComponent({
  uid,
  isUserMessage,
  senderName,
  content,
  stats,
  onCopy,
  onPreviewAttachment,
  attachments,
}: MessageItemProps) {
  const isImage = (filename: string) => /\.(jpg|jpeg|png|gif|webp)$/i.test(filename)

  const getMimeType = (filename: string): string => {
    const ext = filename.split('.').pop()?.toLowerCase()
    const map: Record<string, string> = {
      jpg: 'image/jpeg', jpeg: 'image/jpeg', png: 'image/png',
      gif: 'image/gif', webp: 'image/webp', svg: 'image/svg+xml',
      bmp: 'image/bmp', pdf: 'application/pdf',
      doc: 'application/msword', docx: 'application/vnd.openxmlformats-officedocument.wordprocessingml.document',
      txt: 'text/plain', rtf: 'application/rtf',
    }
    return map[ext || ''] || 'application/octet-stream'
  }

  const bytesToBase64 = (bytes: number[]): string => {
    let binary = ''
    for (let i = 0; i < bytes.length; i++) {
      binary += String.fromCharCode(bytes[i])
    }
    return btoa(binary)
  }

  const getAttachmentSrc = (att: VizierAttachment): string | undefined => {
    const mime = getMimeType(att.filename)
    if ('url' in att.content) {
      const url = att.content.url
      return (url.startsWith(`http://${base_url}`) || url.startsWith(`https://${base_url}`) ? '' : `${api_protocol}://${base_url}`) + url
    }
    if ('base64' in att.content) {
      return `data:${mime};base64,${att.content.base64}`
    }
    if ('bytes' in att.content) {
      return `data:${mime};base64,${bytesToBase64(att.content.bytes)}`
    }
    return undefined
  }

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
            flexDirection: 'row',
            flexWrap: 'wrap',
            gap: '8px',
            width: 'fit-content',
          }}>
            {attachments.map((att, idx) => {
              const src = getAttachmentSrc(att)
              const isImg = isImage(att.filename)
              return (
                <div
                  key={idx}
                  className="chat-attachment-chip"
                  style={{ cursor: onPreviewAttachment ? 'pointer' : 'default' }}
                  onClick={() => onPreviewAttachment?.(att)}
                >
                  {isImg && src ? (
                    <img
                      src={src}
                      alt={att.filename}
                      className="chat-attachment-chip-thumbnail"
                    />
                  ) : (
                    <span style={{ display: 'flex', alignItems: 'center', padding: '4px', color: 'var(--text-tertiary)' }}>
                      {getFileIcon(att.filename)}
                    </span>
                  )}
                  <span>{att.filename}</span>
                </div>
              )
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
