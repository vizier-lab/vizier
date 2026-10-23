import { useState, useRef, useCallback } from 'react'
import { uploadFile } from '../services/vizier'
import type { VizierAttachment } from '../interfaces/types'

const DEFAULT_ACCEPT = 'image/*,.pdf,.doc,.docx,.txt,video/*,audio/*'

interface PendingAttachment {
  file: File
  previewUrl: string | null
}

interface UseFileAttachmentsOptions {
  accept?: string
}

interface UseFileAttachmentsReturn {
  attachments: PendingAttachment[]
  isDragOver: boolean
  fileInputRef: React.RefObject<HTMLInputElement | null>
  processFiles: (files: File[]) => void
  removeAttachment: (index: number) => void
  clearAttachments: () => void
  handleFileSelect: (e: React.ChangeEvent<HTMLInputElement>) => void
  handleDragEnter: (e: React.DragEvent) => void
  handleDragLeave: (e: React.DragEvent) => void
  handleDragOver: (e: React.DragEvent) => void
  handleDrop: (e: React.DragEvent) => void
  handlePaste: (e: React.ClipboardEvent) => void
  uploadAll: () => Promise<VizierAttachment[]>
}

export function useFileAttachments(
  options?: UseFileAttachmentsOptions
): UseFileAttachmentsReturn {
  const accept = options?.accept ?? DEFAULT_ACCEPT
  const [attachments, setAttachments] = useState<PendingAttachment[]>([])
  const [isDragOver, setIsDragOver] = useState(false)
  const fileInputRef = useRef<HTMLInputElement | null>(null)
  const dragCounterRef = useRef(0)

  const processFiles = useCallback((files: File[]) => {
    const newAttachments: PendingAttachment[] = []
    for (const file of files) {
      const previewUrl =
        file.type.startsWith('image/') || file.type.startsWith('video/')
          ? URL.createObjectURL(file)
          : null
      newAttachments.push({ file, previewUrl })
    }
    setAttachments((prev) => [...prev, ...newAttachments])
  }, [])

  const removeAttachment = useCallback((index: number) => {
    setAttachments((prev) => {
      const removed = prev[index]
      if (removed?.previewUrl) {
        URL.revokeObjectURL(removed.previewUrl)
      }
      return prev.filter((_, i) => i !== index)
    })
  }, [])

  const clearAttachments = useCallback(() => {
    setAttachments((prev) => {
      for (const att of prev) {
        if (att.previewUrl) URL.revokeObjectURL(att.previewUrl)
      }
      return []
    })
  }, [])

  const handleFileSelect = useCallback(
    (e: React.ChangeEvent<HTMLInputElement>) => {
      const files = e.target.files
      if (!files) return
      processFiles(Array.from(files))
      if (fileInputRef.current) {
        fileInputRef.current.value = ''
      }
    },
    [processFiles]
  )

  const handleDragEnter = useCallback((e: React.DragEvent) => {
    e.preventDefault()
    e.stopPropagation()
    dragCounterRef.current++
    if (e.dataTransfer.types.includes('Files')) {
      setIsDragOver(true)
    }
  }, [])

  const handleDragLeave = useCallback((e: React.DragEvent) => {
    e.preventDefault()
    e.stopPropagation()
    dragCounterRef.current--
    if (dragCounterRef.current === 0) {
      setIsDragOver(false)
    }
  }, [])

  const handleDragOver = useCallback((e: React.DragEvent) => {
    e.preventDefault()
    e.stopPropagation()
  }, [])

  const handleDrop = useCallback(
    (e: React.DragEvent) => {
      e.preventDefault()
      e.stopPropagation()
      dragCounterRef.current = 0
      setIsDragOver(false)
      const files = Array.from(e.dataTransfer.files).filter((f) => {
        if (accept === '*') return true
        const ext = f.name.toLowerCase()
        return (
          f.type.startsWith('image/') ||
          f.type.startsWith('video/') ||
          f.type.startsWith('audio/') ||
          ext.endsWith('.pdf') ||
          ext.endsWith('.doc') ||
          ext.endsWith('.docx') ||
          ext.endsWith('.txt')
        )
      })
      if (files.length > 0) {
        processFiles(files)
      }
    },
    [processFiles, accept]
  )

  const handlePaste = useCallback(
    (e: React.ClipboardEvent) => {
      const items = Array.from(e.clipboardData.items)
      const imageItems = items.filter((item) => item.type.startsWith('image/'))
      if (imageItems.length > 0) {
        const files = imageItems
          .map((item) => item.getAsFile())
          .filter((f): f is File => f !== null)
        processFiles(files)
      }
    },
    [processFiles]
  )

  const uploadAll = useCallback(async (): Promise<VizierAttachment[]> => {
    const results: VizierAttachment[] = []
    for (const att of attachments) {
      try {
        const res = await uploadFile(att.file)
        results.push({ filename: att.file.name, content: { local: res.url } })
      } catch (err) {
        console.error('File upload failed:', err)
      }
    }
    return results
  }, [attachments])

  return {
    attachments,
    isDragOver,
    fileInputRef,
    processFiles,
    removeAttachment,
    clearAttachments,
    handleFileSelect,
    handleDragEnter,
    handleDragLeave,
    handleDragOver,
    handleDrop,
    handlePaste,
    uploadAll,
  }
}
