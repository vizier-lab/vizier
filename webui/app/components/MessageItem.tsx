import { memo, useState, useCallback } from 'react'
import ReactMarkdown from 'react-markdown'
import remarkGfm from 'remark-gfm'
import rehypeHighlight from 'rehype-highlight'
import { FaCopy, FaFile, FaFilePdf, FaFileImage, FaFileLines, FaFileVideo, FaFileAudio } from 'react-icons/fa6'
import type { VizierAttachment, VizierResponseStats, ReactionEntry } from '../interfaces/types'
import { base_url, api_protocol } from '~/services/vizier'
import { EmojiPickerPopup } from './EmojiPickerPopup'
import { ReactionBadges } from './ReactionBadges'
import { VoiceMessagePlayer } from './VoiceMessagePlayer'

interface MessageItemProps {
  uid: string
  isUserMessage: boolean
  senderName: string
  content: string
  stats?: VizierResponseStats
  onCopy: (content: string) => void
  onPreviewAttachment?: (attachment: VizierAttachment) => void
  attachments?: VizierAttachment[]
  reactions?: ReactionEntry[]
  currentUserId?: string
  onReact?: (messageUid: string, emoji: string) => void
  isVoiceMessage?: boolean
  voiceSrc?: string
  audioReplySrc?: string
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
  reactions,
  currentUserId,
  onReact,
  isVoiceMessage,
  voiceSrc,
  audioReplySrc,
}: MessageItemProps) {
  const [showPicker, setShowPicker] = useState(false)

  const isImage = (filename: string) => /\.(jpg|jpeg|png|gif|webp|svg|bmp)$/i.test(filename)

  const getMimeType = (filename: string): string => {
    const ext = filename.split('.').pop()?.toLowerCase()
    const map: Record<string, string> = {
      jpg: 'image/jpeg', jpeg: 'image/jpeg', png: 'image/png',
      gif: 'image/gif', webp: 'image/webp', svg: 'image/svg+xml',
      bmp: 'image/bmp', pdf: 'application/pdf',
      doc: 'application/msword', docx: 'application/vnd.openxmlformats-officedocument.wordprocessingml.document',
      txt: 'text/plain', rtf: 'application/rtf',
      mp4: 'video/mp4', webm: 'video/webm', ogg: 'video/ogg', mov: 'video/quicktime', avi: 'video/x-msvideo', mkv: 'video/x-matroska',
      mp3: 'audio/mpeg', wav: 'audio/wav', flac: 'audio/flac', aac: 'audio/aac', m4a: 'audio/mp4', oga: 'audio/ogg',
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
    if ('local' in att.content) {
      const path = att.content.local
      return (path.startsWith(`http://${base_url}`) || path.startsWith(`https://${base_url}`) ? '' : `${api_protocol}://${base_url}`) + path
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
    if (/\.(jpg|jpeg|png|gif|webp|svg|bmp)$/i.test(filename)) return <FaFileImage size={16} />
    if (/\.(mp4|webm|ogg|mov|avi|mkv)$/i.test(filename)) return <FaFileVideo size={16} />
    if (/\.(mp3|wav|ogg|flac|aac|m4a|oga)$/i.test(filename)) return <FaFileAudio size={16} />
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
      <div
        style={{
          padding: '12px 16px',
          background: isUserMessage ? 'var(--surface)' : 'transparent',
          borderRadius: '8px',
          borderLeft: isUserMessage ? 'none' : '3px solid var(--accent-primary)',
          boxShadow: isUserMessage ? 'var(--shadow-sm)' : 'none',
        }}
      >
        <div className="flex items-start justify-between">
          <div className='prose'>
            {audioReplySrc && (
              <VoiceMessagePlayer src={audioReplySrc} />
            )}
            {isVoiceMessage && voiceSrc ? (
              <VoiceMessagePlayer src={voiceSrc} />
            ) : (
              <ReactMarkdown remarkPlugins={[remarkGfm]} rehypePlugins={[rehypeHighlight]}>
                {content}
              </ReactMarkdown>
            )}
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

        {onReact && (
          <div style={{
            display: 'flex',
            alignItems: 'center',
            flexWrap: 'wrap',
            gap: '4px',
            marginTop: '6px',
          }}>
            {reactions && reactions.length > 0 && (
              <ReactionBadges
                reactions={reactions}
                currentUserId={currentUserId}
                onToggleReaction={(emoji) => onReact(uid, emoji)}
              />
            )}
            <div style={{ position: 'relative' }}>
              <button
                onClick={() => setShowPicker(!showPicker)}
                style={{
                  display: 'inline-flex',
                  alignItems: 'center',
                  gap: '4px',
                  padding: '2px 8px',
                  borderRadius: '12px',
                  border: '1px solid var(--border)',
                  background: 'var(--surface)',
                  color: 'var(--text-secondary)',
                  cursor: 'pointer',
                  fontSize: '12px',
                  lineHeight: '18px',
                  transition: 'all 0.15s',
                }}
                onMouseEnter={(e) => {
                  (e.target as HTMLElement).style.background = 'var(--border)'
                }}
                onMouseLeave={(e) => {
                  (e.target as HTMLElement).style.background = 'var(--surface)'
                }}
              >
                <span>+</span>
              </button>
              {showPicker && (
                <EmojiPickerPopup
                  onSelect={(emoji) => {
                    onReact(uid, emoji)
                    setShowPicker(false)
                  }}
                  onClose={() => setShowPicker(false)}
                />
              )}
            </div>
          </div>
        )}
      </div>
    </div>
  )
}

// Memoize component to prevent re-renders when parent re-renders
// Only re-render if the message UID, content, stats, attachments, or reactions change
export const MessageItem = memo(MessageItemComponent, (prevProps, nextProps) => {
  if (prevProps.uid !== nextProps.uid) return false
  if (prevProps.content !== nextProps.content) return false
  if (prevProps.stats !== nextProps.stats) return false
  if (prevProps.attachments !== nextProps.attachments) return false
  if (prevProps.reactions !== nextProps.reactions) return false
  if (prevProps.currentUserId !== nextProps.currentUserId) return false
  if (prevProps.audioReplySrc !== nextProps.audioReplySrc) return false
  return true
})
