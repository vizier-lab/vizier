import { FaXmark, FaFile, FaFilePdf, FaFileImage, FaFileLines, FaFileVideo, FaFileAudio } from 'react-icons/fa6'
import type { VizierAttachment } from '../interfaces/types'
import { base_url, api_protocol } from '~/services/vizier'

function getMimeType(filename: string): string {
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

function bytesToBase64(bytes: number[]): string {
  let binary = ''
  for (let i = 0; i < bytes.length; i++) {
    binary += String.fromCharCode(bytes[i])
  }
  return btoa(binary)
}

function getAttachmentSrc(att: VizierAttachment): string | undefined {
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

function getFileIcon(filename: string) {
  if (/\.pdf$/i.test(filename)) return <FaFilePdf size={16} />
  if (/\.(jpg|jpeg|png|gif|webp|svg|bmp)$/i.test(filename)) return <FaFileImage size={16} />
  if (/\.(mp4|webm|ogg|mov|avi|mkv)$/i.test(filename)) return <FaFileVideo size={16} />
  if (/\.(mp3|wav|ogg|flac|aac|m4a|oga)$/i.test(filename)) return <FaFileAudio size={16} />
  if (/\.(doc|docx|txt|rtf)$/i.test(filename)) return <FaFileLines size={16} />
  return <FaFile size={16} />
}

function isImage(filename: string): boolean {
  return /\.(jpg|jpeg|png|gif|webp|svg|bmp)$/i.test(filename)
}

interface AttachmentChipProps {
  attachment: VizierAttachment
  previewUrl?: string | null
  onRemove?: () => void
  onClick?: () => void
}

export default function AttachmentChip({
  attachment,
  previewUrl,
  onRemove,
  onClick,
}: AttachmentChipProps) {
  const src = getAttachmentSrc(attachment)
  const showImage = isImage(attachment.filename) && (previewUrl || src)

  return (
    <div
      className="chat-attachment-chip"
      style={{ cursor: onClick ? 'pointer' : 'default' }}
      onClick={onClick}
    >
      {showImage && (
        <img
          src={previewUrl || src}
          alt={attachment.filename}
          className="chat-attachment-chip-thumbnail"
        />
      )}
      {!showImage && (
        <span style={{ display: 'flex', alignItems: 'center', padding: '4px', color: 'var(--text-tertiary)' }}>
          {getFileIcon(attachment.filename)}
        </span>
      )}
      <span>{attachment.filename}</span>
      {onRemove && (
        <button
          onClick={(e) => {
            e.stopPropagation()
            onRemove()
          }}
          className="chat-attachment-chip-remove"
        >
          <FaXmark size={10} />
        </button>
      )}
    </div>
  )
}

export { getAttachmentSrc, getFileIcon, isImage }
