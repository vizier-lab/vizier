import { useState, useEffect } from 'react'
import { FaXmark, FaFilePdf, FaFileVideo, FaFileAudio } from 'react-icons/fa6'
import type { VizierAttachment } from '../interfaces/types'
import { base_url, api_protocol } from '~/services/vizier'

interface AttachmentPreviewModalProps {
  attachment: VizierAttachment | null
  onClose: () => void
}

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
    return (url.startsWith('http://') || url.startsWith('https://') ? '' : `${api_protocol}://${base_url}`) + url
  }
  if ('local' in att.content) {
    const path = att.content.local
    return (path.startsWith('http://') || path.startsWith('https://') ? '' : `${api_protocol}://${base_url}`) + path
  }
  if ('base64' in att.content) {
    return `data:${mime};base64,${att.content.base64}`
  }
  if ('bytes' in att.content) {
    return `data:${mime};base64,${bytesToBase64(att.content.bytes)}`
  }
  return undefined
}

function isImage(filename: string): boolean {
  return /\.(jpg|jpeg|png|gif|webp|svg|bmp)$/i.test(filename)
}

function isText(filename: string): boolean {
  return /\.(txt|md|json|csv|xml|yaml|yml|toml|js|ts|py|rs|go|java|c|cpp|h|rb|php|sh|bash)$/i.test(filename)
}

function isPdf(filename: string): boolean {
  return /\.pdf$/i.test(filename)
}

function isVideo(filename: string): boolean {
  return /\.(mp4|webm|ogg|mov|avi|mkv)$/i.test(filename)
}

function isAudio(filename: string): boolean {
  return /\.(mp3|wav|ogg|flac|aac|m4a|oga)$/i.test(filename)
}

export default function AttachmentPreviewModal({ attachment, onClose }: AttachmentPreviewModalProps) {
  useEffect(() => {
    const handleEscape = (e: KeyboardEvent) => {
      if (e.key === 'Escape') onClose()
    }
    if (attachment) {
      document.addEventListener('keydown', handleEscape)
      return () => document.removeEventListener('keydown', handleEscape)
    }
  }, [attachment, onClose])

  if (!attachment) return null

  const src = getAttachmentSrc(attachment)
  const isImg = isImage(attachment.filename)
  const isTxt = isText(attachment.filename)
  const isPdfFile = isPdf(attachment.filename)
  const isVid = isVideo(attachment.filename)
  const isAud = isAudio(attachment.filename)

  return (
    <>
      <div
        style={{
          position: 'fixed',
          top: 0,
          left: 0,
          right: 0,
          bottom: 0,
          background: 'rgba(0, 0, 0, 0.7)',
          zIndex: 2000,
          backdropFilter: 'blur(4px)',
          display: 'flex',
          alignItems: 'center',
          justifyContent: 'center',
        }}
        onClick={onClose}
      >
        <div
          style={{
            position: 'relative',
            maxWidth: '90vw',
            maxHeight: '90vh',
            background: 'var(--background)',
            borderRadius: '12px',
            border: '1px solid var(--border)',
            boxShadow: 'var(--shadow-xl)',
            overflow: 'hidden',
            display: 'flex',
            flexDirection: 'column',
          }}
          onClick={(e) => e.stopPropagation()}
        >
          {/* Header */}
          <div style={{
            display: 'flex',
            alignItems: 'center',
            justifyContent: 'space-between',
            padding: '12px 16px',
            borderBottom: '1px solid var(--border)',
          }}>
            <span style={{ fontSize: '14px', fontWeight: 500, color: 'var(--text-primary)' }}>
              {attachment.filename}
            </span>
            <button
              onClick={onClose}
              style={{
                background: 'none',
                border: 'none',
                color: 'var(--text-tertiary)',
                cursor: 'pointer',
                padding: '4px',
              }}
            >
              <FaXmark size={18} />
            </button>
          </div>

          {/* Content */}
          <div style={{ overflow: 'auto', maxHeight: 'calc(90vh - 60px)' }}>
            {isImg && src ? (
              <img
                src={src}
                alt={attachment.filename}
                style={{
                  maxWidth: '90vw',
                  maxHeight: '80vh',
                  objectFit: 'contain',
                  display: 'block',
                }}
              />
            ) : isPdfFile && src ? (
              <PdfPreview src={src} />
            ) : isVid && src ? (
              <VideoPreview src={src} />
            ) : isAud && src ? (
              <AudioPreview src={src} />
            ) : isTxt && src ? (
              <TextPreview src={src} />
            ) : isTxt && 'base64' in attachment.content ? (
              <TextPreviewContent content={atob(attachment.content.base64)} />
            ) : (
              <div style={{
                padding: '2rem',
                textAlign: 'center',
                color: 'var(--text-tertiary)',
              }}>
                <p>Preview not available for this file type.</p>
                {src && (
                  <a
                    href={src}
                    download={attachment.filename}
                    style={{ color: 'var(--accent-primary)', textDecoration: 'underline' }}
                  >
                    Download file
                  </a>
                )}
              </div>
            )}
          </div>
        </div>
      </div>
    </>
  )
}

function TextPreview({ src }: { src: string }) {
  const [content, setContent] = useState<string | null>(null)
  const [loading, setLoading] = useState(true)

  useEffect(() => {
    fetch(src)
      .then((r) => r.text())
      .then(setContent)
      .catch(() => setContent('Failed to load file.'))
      .finally(() => setLoading(false))
  }, [src])

  if (loading) {
    return (
      <div style={{ padding: '2rem', textAlign: 'center', color: 'var(--text-tertiary)' }}>
        Loading...
      </div>
    )
  }

  return <TextPreviewContent content={content || ''} />
}

function TextPreviewContent({ content }: { content: string }) {
  return (
    <pre style={{
      padding: '1rem',
      margin: 0,
      fontSize: '13px',
      lineHeight: '1.5',
      fontFamily: 'var(--font-mono)',
      color: 'var(--text-primary)',
      background: 'var(--surface)',
      whiteSpace: 'pre-wrap',
      wordBreak: 'break-word',
      maxHeight: '70vh',
      overflow: 'auto',
    }}>
      {content}
    </pre>
  )
}

function PdfPreview({ src }: { src: string }) {
  return (
    <embed
      src={src}
      type="application/pdf"
      style={{
        width: '80vw',
        height: '80vh',
        maxWidth: '1200px',
        display: 'block',
        border: 'none',
      }}
    />
  )
}

function VideoPreview({ src }: { src: string }) {
  return (
    <video
      controls
      autoPlay
      style={{
        maxWidth: '90vw',
        maxHeight: '80vh',
        display: 'block',
      }}
    >
      <source src={src} />
      Your browser does not support video playback.
    </video>
  )
}

function AudioPreview({ src }: { src: string }) {
  return (
    <div style={{
      padding: '3rem 2rem',
      display: 'flex',
      flexDirection: 'column',
      alignItems: 'center',
      gap: '1.5rem',
    }}>
      <FaFileAudio size={64} style={{ color: 'var(--text-tertiary)' }} />
      <audio controls autoPlay style={{ width: '100%', maxWidth: '480px' }}>
        <source src={src} />
        Your browser does not support audio playback.
      </audio>
    </div>
  )
}
