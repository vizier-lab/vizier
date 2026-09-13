import { useEffect, useState, useCallback, useRef } from 'react'
import { Link, useParams, useSearchParams } from 'react-router'
import ReactMarkdown from 'react-markdown'
import remarkGfm from 'remark-gfm'
import rehypeHighlight from 'rehype-highlight'
import {
  getMemory,
  createMemory,
  updateMemory,
  deleteMemory,
  getMemoryGraph,
  exportBundle,
  importBundle,
  deleteBundle,
  getAgentDetail,
} from '../services/vizier'
import { autoCorrectSlug, autoCorrectSlugStrict } from '../utils/slug'
import { FaPlus, FaTrash, FaPenToSquare, FaMagnifyingGlass, FaArrowLeft, FaDownload, FaUpload, FaTrashCan } from 'react-icons/fa6'
import { useToastStore } from '../hooks/toastStore'
import { useFileAttachments } from '../hooks/useFileAttachments'
import AttachmentChip from '../components/AttachmentChip'
import AttachmentPreviewModal from '../components/AttachmentPreviewModal'
import type {
  AgentDetail,
  MemoryDetail,
  MemoryGraph as MemoryGraphType,
  ImportReport,
  VizierAttachment,
} from '../interfaces/types'
import MarkdownEditor from '../components/MarkdownEditor'
import MemoryGraph from '../components/MemoryGraph'
import SlideOver from '../components/SlideOver'

function getErrorMessage(err: unknown): string {
  if (err && typeof err === 'object' && 'response' in err) {
    const resp = (err as { response?: { data?: { message?: string } } }).response
    return resp?.data?.message || 'An error occurred'
  }
  return 'An error occurred'
}

// Mirrors the backend's same_bundle_link_target (src/storage/memory_bundle.rs): recognizes a
// same-bundle concept link whether or not the agent included the `.md` extension, and returns
// the bare concept path (no extension) to open — or null if this is a URL, mailto:, an anchor,
// or a relative link to something with a *different* extension (an attachment, an image),
// which should behave like an ordinary link instead of being treated as a memory reference.
function sameBundleLinkTarget(href: string): string | null {
  if (!href || href.startsWith('#')) return null
  const firstSegment = href.split('/')[0] ?? ''
  if (firstSegment.includes(':')) return null
  const pathOnly = href.split(/[?#]/)[0] ?? href
  if (pathOnly.endsWith('.md')) return pathOnly.slice(0, -3)
  const leaf = pathOnly.split('/').pop() ?? pathOnly
  if (!pathOnly || leaf.includes('.')) return null
  return pathOnly
}

type ModalMode = 'create' | 'edit' | 'view' | null

function BundleBadge({ bundle }: { bundle: string }) {
  return (
    <span
      style={{
        display: 'inline-block',
        padding: '2px 8px',
        borderRadius: '12px',
        fontSize: '11px',
        fontWeight: 500,
        fontFamily: 'var(--font-mono)',
        background: 'var(--surface)',
        color: 'var(--text-secondary)',
      }}
    >
      {bundle}
    </span>
  )
}

export default function MemoryManagement() {
  const { agentId } = useParams()
  const [searchParams, setSearchParams] = useSearchParams()
  const urlSearch = searchParams.get('search') ?? ''
  const [searchQuery, setSearchQuery] = useState(urlSearch)
  const [selectedMemory, setSelectedMemory] = useState<MemoryDetail | null>(null)
  const [modalMode, setModalMode] = useState<ModalMode>(null)

  // `null` = top-level view (bundles as nodes); a name = that bundle's concept-level view.
  const [currentBundle, setCurrentBundle] = useState<string | null>(null)

  const [formTitle, setFormTitle] = useState('')
  const [formContent, setFormContent] = useState('')
  const [formBundle, setFormBundle] = useState('')
  const [formPath, setFormPath] = useState('')
  const [formTags, setFormTags] = useState('')
  const [submitting, setSubmitting] = useState(false)

  const [graph, setGraph] = useState<MemoryGraphType | null>(null)
  // Concept nodes in the currently-open bundle, excluding synthetic boundary nodes that point
  // to other bundles — used to gate the "Delete Bundle" action (must be empty of concepts).
  const currentBundleConceptCount = graph?.nodes.filter((n) => !n.boundary).length ?? 0
  const [graphLoading, setGraphLoading] = useState(false)
  const [graphVersion, setGraphVersion] = useState(0)
  const [agentDetail, setAgentDetail] = useState<AgentDetail | null>(null)

  const [existingAttachments, setExistingAttachments] = useState<VizierAttachment[]>([])
  const [previewAttachment, setPreviewAttachment] = useState<VizierAttachment | null>(null)

  const [importOpen, setImportOpen] = useState(false)
  const [importDestBundle, setImportDestBundle] = useState('')
  const [importFile, setImportFile] = useState<File | null>(null)
  const [importSubmitting, setImportSubmitting] = useState(false)
  const [importReport, setImportReport] = useState<ImportReport | null>(null)
  const importInputRef = useRef<HTMLInputElement | null>(null)

  const abortRef = useRef<AbortController | null>(null)

  const {
    attachments: pendingAttachments,
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
  } = useFileAttachments()

  const { addToast } = useToastStore()

  useEffect(() => {
    if (!agentId) return
    getAgentDetail(agentId)
      .then((res) => setAgentDetail(res.data))
      .catch(() => setAgentDetail(null))
  }, [agentId])

  useEffect(() => {
    if (searchQuery === urlSearch) return
    const next = new URLSearchParams(searchParams)
    if (searchQuery) {
      next.set('search', searchQuery)
    } else {
      next.delete('search')
    }
    setSearchParams(next, { replace: true })
  }, [searchQuery, urlSearch, searchParams, setSearchParams])

  useEffect(() => {
    if (!agentId) return
    if (abortRef.current) abortRef.current.abort()
    const controller = new AbortController()
    abortRef.current = controller
    setGraphLoading(true)
    const trimmed = urlSearch.trim()
    const opts: { search?: string } = {}
    if (trimmed) opts.search = trimmed

    getMemoryGraph(agentId, currentBundle ?? undefined, opts)
      .then((graphRes) => {
        if (controller.signal.aborted) return
        setGraph(graphRes.data)
      })
      .catch((err) => {
        if (controller.signal.aborted) return
        console.error('Failed to load graph:', err)
        addToast('error', 'Failed to load graph', 'Please try again')
      })
      .finally(() => {
        if (!controller.signal.aborted) setGraphLoading(false)
      })

    return () => controller.abort()
  }, [agentId, urlSearch, graphVersion, currentBundle, addToast])

  const handleSearchChange = useCallback((value: string) => {
    setSearchQuery(value)
  }, [])

  const handleGraphNodeClick = useCallback(
    (node: { slug: string; bundle: string; boundary: boolean }) => {
      if (currentBundle === null) {
        // Top level: every node is a bundle — open it.
        setCurrentBundle(node.slug)
        return
      }
      if (node.boundary) {
        // A pointer to a different bundle — follow it.
        setCurrentBundle(node.bundle)
        return
      }
      handleViewMemory(node.slug, currentBundle)
    },
    // eslint-disable-next-line react-hooks/exhaustive-deps
    [currentBundle]
  )

  const handleGraphNodeDelete = useCallback(
    (node: { slug: string; bundle: string; boundary: boolean }) => {
      if (node.boundary) return
      if (currentBundle === null) {
        // Top level: every node is a bundle. We don't know its concept count from here, so
        // always force — the confirmation dialog already warns about permanent deletion.
        void performDeleteBundle(node.slug, true)
        return
      }
      void performDeleteMemory(node.slug, node.bundle)
    },
    // eslint-disable-next-line react-hooks/exhaustive-deps
    [currentBundle]
  )

  const handleViewMemory = async (path: string, bundle: string) => {
    if (!agentId) return
    try {
      const response = await getMemory(agentId, path, bundle)
      setSelectedMemory(response.data)
      setModalMode('view')
    } catch (error) {
      console.error('Failed to load memory:', error)
      addToast('error', 'Failed to load memory', 'Please try again')
    }
  }

  const handleEditMemory = async (memory: MemoryDetail) => {
    let detail = memory
    if (!memory.content && agentId) {
      try {
        const response = await getMemory(agentId, memory.path, memory.bundle)
        detail = response.data
      } catch (error) {
        console.error('Failed to load memory:', error)
        addToast('error', 'Failed to load memory', 'Please try again')
        return
      }
    }
    setSelectedMemory(detail)
    setFormTitle(detail.title)
    setFormContent(detail.content)
    setFormBundle(detail.bundle)
    setFormPath(detail.path)
    setFormTags(detail.tags?.join(', ') || '')
    setExistingAttachments(detail.attachments || [])
    clearAttachments()
    setModalMode('edit')
  }

  const handleCreateMemory = () => {
    setFormTitle('')
    setFormContent('')
    setFormBundle(currentBundle ?? 'default')
    setFormPath('')
    setFormTags('')
    setExistingAttachments([])
    clearAttachments()
    setModalMode('create')
  }

  const handleSubmit = async () => {
    if (!agentId || !formTitle.trim() || !formContent.trim()) return
    setSubmitting(true)
    try {
      const finalPath = formPath ? autoCorrectSlugStrict(formPath) : undefined
      const tags = formTags
        .split(',')
        .map((s) => s.trim())
        .filter((s) => s.length > 0)

      const sanitizedContent = formContent.replace(/\\\[\\\[(.+?)\]\]/g, '[[$1]]')

      const newAttachments = await uploadAll()
      const allAttachments = [...existingAttachments, ...newAttachments]

      if (modalMode === 'create') {
        await createMemory(
          agentId,
          formTitle,
          sanitizedContent,
          formBundle || undefined,
          finalPath,
          tags,
          allAttachments.length > 0 ? allAttachments : undefined
        )
        addToast('success', 'Memory created successfully')
      } else if (modalMode === 'edit' && selectedMemory) {
        await updateMemory(
          agentId,
          selectedMemory.path,
          formTitle,
          sanitizedContent,
          selectedMemory.bundle,
          tags,
          allAttachments.length > 0 ? allAttachments : undefined
        )
        addToast('success', 'Memory updated successfully')
      }
      setGraphVersion((v) => v + 1)
      closeModal()
    } catch (error: unknown) {
      console.error('Failed to save memory:', error)
      addToast('error', 'Failed to save memory', getErrorMessage(error))
    } finally {
      setSubmitting(false)
    }
  }

  const performDeleteMemory = async (path: string, bundle: string) => {
    if (!agentId) return
    if (!confirm(`Delete memory "${bundle}/${path}"? This cannot be undone.`)) return
    try {
      await deleteMemory(agentId, path, bundle)
      addToast('success', 'Memory deleted successfully')
      setGraphVersion((v) => v + 1)
      closeModal()
    } catch (error: unknown) {
      console.error('Failed to delete memory:', error)
      addToast('error', 'Failed to delete memory', getErrorMessage(error))
    }
  }

  const handleDeleteMemory = (path: string, bundle: string, e: React.MouseEvent) => {
    e.stopPropagation()
    void performDeleteMemory(path, bundle)
  }

  const closeModal = () => {
    setModalMode(null)
    setSelectedMemory(null)
    setFormTitle('')
    setFormContent('')
    setFormBundle('')
    setFormPath('')
    setFormTags('')
    setExistingAttachments([])
    clearAttachments()
  }

  const handleExportBundle = async () => {
    if (!agentId || !currentBundle) return
    try {
      const blob = await exportBundle(agentId, currentBundle)
      const url = URL.createObjectURL(blob)
      const a = document.createElement('a')
      a.href = url
      a.download = `${currentBundle}.zip`
      document.body.appendChild(a)
      a.click()
      a.remove()
      URL.revokeObjectURL(url)
    } catch (error) {
      console.error('Failed to export bundle:', error)
      addToast('error', 'Failed to export bundle', getErrorMessage(error))
    }
  }

  const performDeleteBundle = async (bundle: string, force: boolean) => {
    if (!agentId) return
    const message = force
      ? `Delete bundle "${bundle}"? This will permanently delete everything in it. This cannot be undone.`
      : `Delete bundle "${bundle}"? This cannot be undone.`
    if (!confirm(message)) return
    try {
      await deleteBundle(agentId, bundle, force)
      addToast('success', `Bundle "${bundle}" deleted`)
      if (currentBundle === bundle) setCurrentBundle(null)
      setGraphVersion((v) => v + 1)
    } catch (error) {
      console.error('Failed to delete bundle:', error)
      addToast('error', 'Failed to delete bundle', getErrorMessage(error))
    }
  }

  const handleDeleteBundle = () => {
    if (!currentBundle) return
    void performDeleteBundle(currentBundle, currentBundleConceptCount > 0)
  }

  const openImportDialog = () => {
    setImportDestBundle(currentBundle ?? '')
    setImportFile(null)
    setImportReport(null)
    setImportOpen(true)
  }

  const handleImportSubmit = async () => {
    if (!agentId || !importFile) return
    setImportSubmitting(true)
    try {
      const res = await importBundle(agentId, importDestBundle || 'default', importFile)
      setImportReport(res.data)
      addToast('success', 'Bundle import finished')
      setGraphVersion((v) => v + 1)
    } catch (error) {
      console.error('Failed to import bundle:', error)
      addToast('error', 'Failed to import bundle', getErrorMessage(error))
    } finally {
      setImportSubmitting(false)
    }
  }

  return (
    <>
      <div className="main-header">
        <div style={{ flex: 1, display: 'flex', alignItems: 'center', gap: '10px' }}>
          {currentBundle !== null && (
            <button
              className="btn btn-ghost"
              onClick={() => setCurrentBundle(null)}
              style={{ padding: '4px 8px' }}
              title="Back to bundles"
            >
              <FaArrowLeft size={14} />
            </button>
          )}
          <h3 style={{ margin: 0 }}>
            Memory Management
            {currentBundle !== null && (
              <>
                {' '}
                <BundleBadge bundle={currentBundle} />
              </>
            )}
          </h3>
        </div>

        <div style={{ position: 'relative' }}>
          <FaMagnifyingGlass
            size={14}
            style={{
              position: 'absolute',
              left: '10px',
              top: '50%',
              transform: 'translateY(-50%)',
              color: 'var(--text-tertiary)',
            }}
          />
          <input
            type="text"
            value={searchQuery}
            onChange={(e) => handleSearchChange(e.target.value)}
            placeholder={currentBundle ? 'Search this bundle...' : 'Search bundles...'}
            style={{
              padding: '8px 12px 8px 32px',
              borderRadius: '6px',
              border: '1px solid var(--border)',
              background: 'var(--background)',
              color: 'var(--text)',
              width: '240px',
              fontSize: '13px',
            }}
          />
        </div>

        {currentBundle !== null && (
          <>
            <button className="btn btn-secondary" onClick={handleExportBundle}>
              <FaDownload size={14} />
              <span>Export</span>
            </button>
            <button
              className="btn btn-secondary"
              onClick={handleDeleteBundle}
              title={
                currentBundleConceptCount > 0
                  ? `Delete this bundle and its ${currentBundleConceptCount} remaining concept(s)`
                  : 'Delete this empty bundle'
              }
              style={{ color: '#ef4444' }}
            >
              <FaTrashCan size={14} />
              <span>Delete Bundle</span>
            </button>
          </>
        )}
        <button className="btn btn-secondary" onClick={openImportDialog}>
          <FaUpload size={14} />
          <span>Import</span>
        </button>

        <button className="btn btn-primary" onClick={handleCreateMemory}>
          <FaPlus size={16} />
          <span>New Memory</span>
        </button>
      </div>

      <div className="main-body">
        {agentDetail && !agentDetail.embedding && (
          <div
            style={{
              padding: '12px 16px',
              marginBottom: '16px',
              borderRadius: '8px',
              border: '1px solid #f59e0b',
              background: 'rgba(245, 158, 11, 0.08)',
              color: 'var(--text-primary)',
              fontSize: '0.85rem',
              display: 'flex',
              alignItems: 'center',
              gap: '12px',
            }}
          >
            <span style={{ flex: 1 }}>
              This agent has no embedding configured. Semantic memory search
              and the knowledge graph are unavailable until an embedding model
              is set up.
            </span>
            <Link
              to={`/${agentId}/settings`}
              className="btn btn-primary"
              style={{ fontSize: '0.8rem', padding: '6px 12px' }}
            >
              Configure embedding
            </Link>
          </div>
        )}
        <div style={{ height: '100%', minHeight: '500px' }}>
          {graphLoading && !graph ? (
            <div style={{ display: 'flex', justifyContent: 'center', alignItems: 'center', height: '100%', color: 'var(--text-tertiary)' }}>
              Loading graph…
            </div>
          ) : graph ? (
            <MemoryGraph
              graph={graph}
              searchQuery={searchQuery}
              onNodeClick={handleGraphNodeClick}
              onNodeDelete={handleGraphNodeDelete}
            />
          ) : (
            <div style={{ textAlign: 'center', color: 'var(--text-tertiary)', padding: '3rem' }}>
              <p>Failed to load graph</p>
            </div>
          )}
        </div>
      </div>

      {/* SlideOver */}
      <SlideOver
        open={modalMode !== null}
        onClose={closeModal}
        title={
          modalMode === 'view' ? selectedMemory?.title ?? '' :
            modalMode === 'create' ? 'Create Memory' :
              'Edit Memory'
        }
      >
        {modalMode === 'view' && selectedMemory && (
          <div style={{ display: 'flex', flexDirection: 'column', gap: '1.5rem', flex: 1 }}>
            <div>
              <p style={{ fontSize: '12px', color: 'var(--text-tertiary)', fontFamily: 'var(--font-mono)' }}>
                {selectedMemory.bundle}/{selectedMemory.path} &bull; {new Date(selectedMemory.updated_at).toLocaleString()}
              </p>
              <div style={{ marginTop: '0.5rem', display: 'flex', gap: '8px', alignItems: 'center', flexWrap: 'wrap' }}>
                <BundleBadge bundle={selectedMemory.bundle} />
                {selectedMemory.tags?.map((tag) => (
                  <span
                    key={tag}
                    style={{
                      fontSize: '11px',
                      padding: '2px 8px',
                      borderRadius: '8px',
                      background: 'var(--surface)',
                      color: 'var(--text-secondary)',
                    }}
                  >
                    {tag}
                  </span>
                ))}
              </div>
            </div>
            <div
              className="prose"
              style={{
                background: 'var(--surface)',
                padding: '1.5rem',
                borderRadius: '8px',
                border: '1px solid var(--border)',
              }}
            >
              <ReactMarkdown
                remarkPlugins={[remarkGfm]}
                rehypePlugins={[rehypeHighlight]}
                components={{
                  a: ({ href, children, ...props }) => {
                    const target = href ? sameBundleLinkTarget(href) : null
                    if (target === null) {
                      return (
                        <a href={href} target="_blank" rel="noreferrer" {...props}>
                          {children}
                        </a>
                      )
                    }
                    return (
                      <a
                        href={href}
                        {...props}
                        onClick={(e) => {
                          e.preventDefault()
                          handleViewMemory(target, selectedMemory.bundle)
                        }}
                      >
                        {children}
                      </a>
                    )
                  },
                }}
              >
                {selectedMemory.content}
              </ReactMarkdown>
            </div>

            {selectedMemory.attachments && selectedMemory.attachments.length > 0 && (
              <div>
                <h4 style={{ marginBottom: '0.5rem', fontSize: '13px', color: 'var(--text-secondary)' }}>
                  Attachments ({selectedMemory.attachments.length})
                </h4>
                <div style={{ display: 'flex', gap: '8px', flexWrap: 'wrap' }}>
                  {selectedMemory.attachments.map((att, idx) => (
                    <AttachmentChip
                      key={idx}
                      attachment={att}
                      onClick={() => setPreviewAttachment(att)}
                    />
                  ))}
                </div>
              </div>
            )}

            {selectedMemory.relations && selectedMemory.relations.length > 0 && (
              <div>
                <h4 style={{ marginBottom: '0.5rem', fontSize: '13px', color: 'var(--text-secondary)' }}>
                  Linked Memories
                </h4>
                <div style={{ display: 'flex', gap: '8px', flexWrap: 'wrap' }}>
                  {selectedMemory.relations.map((rel) => (
                    <button
                      key={rel}
                      className="btn btn-ghost"
                      style={{ padding: '4px 10px', fontSize: '12px', fontFamily: 'var(--font-mono)' }}
                      onClick={() => {
                        // Same-bundle link ("path.md"), cross-bundle concept ("bundle/slug"),
                        // or whole-bundle reference ("bundle") — resolve to a (bundle, path)
                        // best-effort for direct navigation from this chip.
                        if (rel.endsWith('.md')) {
                          handleViewMemory(rel.slice(0, -3), selectedMemory.bundle)
                        } else if (rel.includes('/')) {
                          const [b, ...rest] = rel.split('/')
                          handleViewMemory(rest.join('/'), b)
                        } else {
                          setCurrentBundle(rel)
                          setModalMode(null)
                        }
                      }}
                    >
                      {rel}
                    </button>
                  ))}
                </div>
              </div>
            )}

            <div style={{ display: 'flex', gap: '8px' }}>
              <button className="btn btn-secondary" onClick={() => handleEditMemory(selectedMemory)}>
                <FaPenToSquare size={16} />
                Edit
              </button>
              <button
                className="btn btn-ghost"
                onClick={(e) => handleDeleteMemory(selectedMemory.path, selectedMemory.bundle, e)}
                style={{ color: '#ef4444', marginLeft: 'auto' }}
              >
                <FaTrash size={16} />
                Delete
              </button>
            </div>
          </div>
        )}

        {(modalMode === 'create' || modalMode === 'edit') && (
          <div
            style={{ display: 'flex', flexDirection: 'column', gap: '1rem', flex: 1, height: '100%', position: 'relative' }}
            onDragEnter={handleDragEnter}
            onDragLeave={handleDragLeave}
            onDragOver={handleDragOver}
            onDrop={handleDrop}
          >
            {isDragOver && (
              <div style={{
                position: 'absolute',
                inset: 0,
                display: 'flex',
                alignItems: 'center',
                justifyContent: 'center',
                background: 'var(--surface)',
                border: '2px dashed var(--accent-primary)',
                borderRadius: '8px',
                zIndex: 10,
                color: 'var(--accent-primary)',
                fontSize: '14px',
              }}>
                Drop files here
              </div>
            )}
            {modalMode === 'create' && (
              <>
                <div className="input-group" style={{ marginBottom: 0 }}>
                  <label htmlFor="bundle">Bundle</label>
                  <input
                    id="bundle"
                    type="text"
                    value={formBundle}
                    onChange={(e) => setFormBundle(e.target.value)}
                    placeholder="default"
                  />
                  <div style={{ fontSize: '12px', color: 'var(--text-tertiary)', marginTop: '4px' }}>
                    Naming a new bundle creates it automatically.
                  </div>
                </div>
                <div className="input-group" style={{ marginBottom: 0 }}>
                  <label htmlFor="path">Path (optional)</label>
                  <input
                    id="path"
                    type="text"
                    value={formPath}
                    onChange={(e) => setFormPath(autoCorrectSlug(e.target.value))}
                    placeholder="auto-generated from title if empty; use e.g. friends/bred to nest"
                  />
                </div>
              </>
            )}
            {modalMode === 'edit' && (
              <div className="input-group" style={{ marginBottom: 0 }}>
                <label>Location</label>
                <div style={{ fontFamily: 'var(--font-mono)', fontSize: '13px', color: 'var(--text-secondary)' }}>
                  {formBundle}/{formPath}
                </div>
              </div>
            )}
            <div className="input-group" style={{ marginBottom: 0 }}>
              <label htmlFor="title">Title</label>
              <input
                id="title"
                type="text"
                value={formTitle}
                onChange={(e) => setFormTitle(e.target.value)}
                required
                autoFocus
                placeholder="Enter memory title"
              />
            </div>
            <div className="input-group h-full overflow-hidden" style={{ marginBottom: 0 }}>
              <label htmlFor="content">
                Content
                <span style={{ fontSize: '11px', color: 'var(--text-tertiary)', marginLeft: '8px' }}>
                  Same bundle: [label](path.md) &bull; other bundle: [[bundle/slug]] or [[bundle]]
                </span>
              </label>
              <div style={{ overflow: 'hidden' }}>
                <MarkdownEditor
                  value={formContent}
                  onChange={setFormContent}
                  placeholder="Enter memory content..."
                  className="modal-mdx-editor"
                />
              </div>
            </div>
            <input
              type="file"
              ref={fileInputRef}
              onChange={handleFileSelect}
              multiple
              accept="image/*,.pdf,.doc,.docx,.txt,video/*,audio/*"
              style={{ display: 'none' }}
            />
            {modalMode === 'edit' && existingAttachments.length > 0 && (
              <div className="input-group" style={{ marginBottom: 0 }}>
                <label>Existing Attachments</label>
                <div style={{ display: 'flex', gap: '8px', flexWrap: 'wrap' }}>
                  {existingAttachments.map((att, idx) => (
                    <AttachmentChip key={idx} attachment={att} />
                  ))}
                </div>
              </div>
            )}
            {pendingAttachments.length > 0 && (
              <div className="input-group" style={{ marginBottom: 0 }}>
                <label>New Attachments</label>
                <div style={{ display: 'flex', gap: '8px', flexWrap: 'wrap' }}>
                  {pendingAttachments.map((att, idx) => (
                    <AttachmentChip
                      key={idx}
                      attachment={{ filename: att.file.name, content: { local: '' } }}
                      previewUrl={att.previewUrl}
                      onRemove={() => removeAttachment(idx)}
                    />
                  ))}
                  <button onClick={clearAttachments} className="btn btn-ghost" style={{ fontSize: '12px' }}>
                    Clear all
                  </button>
                </div>
              </div>
            )}
            <div className="input-group" style={{ marginBottom: 0 }}>
              <label htmlFor="tags">
                Tags
                <span style={{ fontSize: '11px', color: 'var(--text-tertiary)', marginLeft: '8px' }}>
                  Comma-separated
                </span>
              </label>
              <input
                id="tags"
                type="text"
                value={formTags}
                onChange={(e) => setFormTags(e.target.value)}
                placeholder="e.g. rust, architecture, project-x"
              />
            </div>
            <div style={{ display: 'flex', gap: '8px', marginTop: '0.5rem' }}>
              <button
                className="btn btn-primary"
                onClick={handleSubmit}
                disabled={!formTitle.trim() || !formContent.trim() || submitting}
                style={{ flex: 1, justifyContent: 'center' }}
              >
                {submitting ? 'Saving...' : 'Save'}
              </button>
              <button className="btn btn-secondary" onClick={closeModal} disabled={submitting}>
                Cancel
              </button>
            </div>
          </div>
        )}
      </SlideOver>

      <SlideOver open={importOpen} onClose={() => setImportOpen(false)} title="Import Bundle">
        <div style={{ display: 'flex', flexDirection: 'column', gap: '1rem', flex: 1 }}>
          <div className="input-group" style={{ marginBottom: 0 }}>
            <label htmlFor="import-bundle">Destination bundle</label>
            <input
              id="import-bundle"
              type="text"
              value={importDestBundle}
              onChange={(e) => setImportDestBundle(e.target.value)}
              placeholder="default"
            />
          </div>
          <div className="input-group" style={{ marginBottom: 0 }}>
            <label htmlFor="import-file">Zip file</label>
            <input
              id="import-file"
              ref={importInputRef}
              type="file"
              accept=".zip"
              onChange={(e) => setImportFile(e.target.files?.[0] ?? null)}
            />
          </div>
          {importReport && (
            <div style={{ fontSize: '13px' }}>
              <div>Imported: {importReport.imported.length}</div>
              {importReport.skipped.length > 0 && (
                <div style={{ color: '#b45309' }}>
                  Skipped (already existed): {importReport.skipped.join(', ')}
                </div>
              )}
            </div>
          )}
          <div style={{ display: 'flex', gap: '8px', marginTop: '0.5rem' }}>
            <button
              className="btn btn-primary"
              onClick={handleImportSubmit}
              disabled={!importFile || importSubmitting}
              style={{ flex: 1, justifyContent: 'center' }}
            >
              {importSubmitting ? 'Importing...' : 'Import'}
            </button>
            <button className="btn btn-secondary" onClick={() => setImportOpen(false)}>
              Close
            </button>
          </div>
        </div>
      </SlideOver>

      <AttachmentPreviewModal
        attachment={previewAttachment}
        onClose={() => setPreviewAttachment(null)}
      />
    </>
  )
}
