import { useEffect, useState, useMemo, useCallback } from 'react'
import ReactMarkdown from 'react-markdown'
import remarkGfm from 'remark-gfm'
import rehypeHighlight from 'rehype-highlight'
import { useParams } from 'react-router'
import {
  listMemories,
  getMemory,
  createMemory,
  updateMemory,
  deleteMemory,
  getMemoryGraph,
  getRelatedMemories,
} from '../services/vizier'
import { autoCorrectSlug, autoCorrectSlugStrict } from '../utils/slug'
import { FaPlus, FaTrash, FaPenToSquare, FaMagnifyingGlass, FaList, FaDiagramProject } from 'react-icons/fa6'
import { Skeleton } from '../components/Skeleton'
import { useToastStore } from '../hooks/toastStore'
import type {
  Memory,
  MemoryDetail,
  MemoryVisibility,
  MemoryGraph as MemoryGraphType,
  PaginatedMemoryResponse,
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

type ModalMode = 'create' | 'edit' | 'view' | null
type ViewMode = 'list' | 'graph'

function VisibilityBadge({ visibility }: { visibility: MemoryVisibility }) {
  const styles: Record<MemoryVisibility, { bg: string; text: string; label: string }> = {
    private: { bg: 'var(--surface)', text: 'var(--text-secondary)', label: 'Private' },
    global: { bg: '#dbeafe', text: '#1d4ed8', label: 'Global' },
    shared: { bg: '#fef3c7', text: '#b45309', label: 'Shared' },
  }
  const style = styles[visibility]
  return (
    <span
      style={{
        display: 'inline-block',
        padding: '2px 8px',
        borderRadius: '12px',
        fontSize: '11px',
        fontWeight: 500,
        background: style.bg,
        color: style.text,
      }}
    >
      {style.label}
    </span>
  )
}

export default function MemoryManagement() {
  const { agentId } = useParams()
  const [memories, setMemories] = useState<Memory[]>([])
  const [selectedMemory, setSelectedMemory] = useState<MemoryDetail | null>(null)
  const [loading, setLoading] = useState(true)
  const [modalMode, setModalMode] = useState<ModalMode>(null)
  const [viewMode, setViewMode] = useState<ViewMode>('list')

  const [formTitle, setFormTitle] = useState('')
  const [formContent, setFormContent] = useState('')
  const [formSlug, setFormSlug] = useState('')
  const [formVisibility, setFormVisibility] = useState<MemoryVisibility>('private')
  const [formSharedTo, setFormSharedTo] = useState('')
  const [formTags, setFormTags] = useState('')
  const [submitting, setSubmitting] = useState(false)

  const [searchQuery, setSearchQuery] = useState('')
  const [filterVisibility, setFilterVisibility] = useState<MemoryVisibility | 'all'>('all')
  const [currentPage, setCurrentPage] = useState(1)
  const [totalMemories, setTotalMemories] = useState(0)
  const pageSize = 20

  const [graph, setGraph] = useState<MemoryGraphType | null>(null)
  const [graphLoading, setGraphLoading] = useState(false)

  const { addToast } = useToastStore()

  useEffect(() => {
    loadMemories()
  }, [agentId, filterVisibility, currentPage])

  useEffect(() => {
    if (viewMode === 'graph' && agentId) {
      loadGraph()
    }
  }, [viewMode, agentId])

  const loadMemories = async () => {
    if (!agentId) return
    try {
      setLoading(true)
      const response: { data: PaginatedMemoryResponse } = await listMemories(agentId, {
        visibility: filterVisibility === 'all' ? undefined : filterVisibility,
        offset: (currentPage - 1) * pageSize,
        limit: pageSize,
      })
      setMemories(response.data?.memories || [])
      setTotalMemories(response.data?.total || 0)
    } catch (error) {
      console.error('Failed to load memories:', error)
      addToast('error', 'Failed to load memories', 'Please try again')
    } finally {
      setLoading(false)
    }
  }

  const loadGraph = async () => {
    if (!agentId) return
    try {
      setGraphLoading(true)
      const response = await getMemoryGraph(agentId)
      setGraph(response.data)
    } catch (error) {
      console.error('Failed to load graph:', error)
      addToast('error', 'Failed to load graph', 'Please try again')
    } finally {
      setGraphLoading(false)
    }
  }

  const filteredMemories = useMemo(() => {
    if (!searchQuery.trim()) return memories
    const q = searchQuery.toLowerCase()
    return memories.filter(
      (m) =>
        m.title.toLowerCase().includes(q) ||
        m.slug.toLowerCase().includes(q) ||
        m.tags?.some((t) => t.toLowerCase().includes(q))
    )
  }, [memories, searchQuery])

  const totalPages = Math.ceil(totalMemories / pageSize)

  const handleViewMemory = async (slug: string) => {
    if (!agentId) return
    try {
      const response = await getMemory(agentId, slug)
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
        const response = await getMemory(agentId, memory.slug)
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
    setFormSlug(detail.slug)
    setFormVisibility(detail.visibility)
    setFormSharedTo(detail.shared_to?.join(', ') || '')
    setFormTags(detail.tags?.join(', ') || '')
    setModalMode('edit')
  }

  const handleCreateMemory = () => {
    setFormTitle('')
    setFormContent('')
    setFormSlug('')
    setFormVisibility('private')
    setFormSharedTo('')
    setFormTags('')
    setModalMode('create')
  }

  const handleSubmit = async () => {
    if (!agentId || !formTitle.trim() || !formContent.trim()) return
    setSubmitting(true)
    try {
      const finalSlug = formSlug ? autoCorrectSlugStrict(formSlug) : undefined
      const sharedTo = formSharedTo
        .split(',')
        .map((s) => s.trim())
        .filter((s) => s.length > 0)
      const tags = formTags
        .split(',')
        .map((s) => s.trim())
        .filter((s) => s.length > 0)

      const sanitizedContent = formContent.replace(/\\\[\\\[(.+?)\]\]/g, '[[$1]]')

      if (modalMode === 'create') {
        await createMemory(agentId, formTitle, sanitizedContent, finalSlug || undefined, formVisibility, sharedTo, tags)
        addToast('success', 'Memory created successfully')
      } else if (modalMode === 'edit' && selectedMemory) {
        await updateMemory(agentId, selectedMemory.slug, formTitle, sanitizedContent, formVisibility, sharedTo, tags)
        addToast('success', 'Memory updated successfully')
      }
      await loadMemories()
      closeModal()
    } catch (error: unknown) {
      console.error('Failed to save memory:', error)
      addToast('error', 'Failed to save memory', getErrorMessage(error))
    } finally {
      setSubmitting(false)
    }
  }

  const handleDeleteMemory = async (slug: string, e: React.MouseEvent) => {
    e.stopPropagation()
    if (!agentId) return
    if (!confirm('Are you sure you want to delete this memory?')) return
    try {
      await deleteMemory(agentId, slug)
      addToast('success', 'Memory deleted successfully')
      await loadMemories()
      closeModal()
    } catch (error: unknown) {
      console.error('Failed to delete memory:', error)
      addToast('error', 'Failed to delete memory', getErrorMessage(error))
    }
  }

  const closeModal = () => {
    setModalMode(null)
    setSelectedMemory(null)
    setFormTitle('')
    setFormContent('')
    setFormSlug('')
    setFormVisibility('private')
    setFormSharedTo('')
    setFormTags('')
  }

  const handleSearchChange = useCallback((value: string) => {
    setSearchQuery(value)
  }, [])

  return (
    <>
      <div className="main-header">
        <div style={{ flex: 1 }}>
          <h3 style={{ margin: 0 }}>Memory Management</h3>
        </div>

        <div className="pill-tabs">
          <button
            className={`pill-tab ${viewMode === 'list' ? 'active' : ''}`}
            onClick={() => setViewMode('list')}
          >
            <FaList size={14} />
            List
          </button>
          <button
            className={`pill-tab ${viewMode === 'graph' ? 'active' : ''}`}
            onClick={() => setViewMode('graph')}
          >
            <FaDiagramProject size={14} />
            Graph
          </button>
        </div>

        <div style={{ display: 'flex', gap: '8px', alignItems: 'center' }}>
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
              placeholder="Search memories..."
              style={{
                padding: '8px 12px 8px 32px',
                borderRadius: '6px',
                border: '1px solid var(--border)',
                background: 'var(--background)',
                color: 'var(--text)',
                width: '200px',
                fontSize: '13px',
              }}
            />
          </div>

          <select
            value={filterVisibility}
            onChange={(e) => {
              setFilterVisibility(e.target.value as MemoryVisibility | 'all')
              setCurrentPage(1)
            }}
            style={{
              padding: '8px 12px',
              borderRadius: '6px',
              border: '1px solid var(--border)',
              background: 'var(--background)',
              color: 'var(--text)',
              fontSize: '13px',
            }}
          >
            <option value="all">All Visibility</option>
            <option value="private">Private</option>
            <option value="global">Global</option>
            <option value="shared">Shared</option>
          </select>
        </div>

        <button className="btn btn-primary" onClick={handleCreateMemory}>
          <FaPlus size={16} />
          <span>New Memory</span>
        </button>
      </div>

      <div className="main-body">
        {viewMode === 'list' ? (
          loading ? (
            <table className="data-table">
              <thead>
                <tr>
                  <th>Title</th>
                  <th>Slug</th>
                  <th>Tags</th>
                  <th>Visibility</th>
                  <th>Updated</th>
                  <th style={{ width: '80px' }}>Actions</th>
                </tr>
              </thead>
              <tbody>
                {[1, 2, 3, 4, 5].map((i) => (
                  <tr key={i} style={{ cursor: 'default' }}>
                    <td><Skeleton variant="text" width="60%" /></td>
                    <td><Skeleton variant="text" width="40%" /></td>
                    <td><Skeleton variant="text" width="60px" /></td>
                    <td><Skeleton variant="text" width="60px" /></td>
                    <td><Skeleton variant="text" width="50%" /></td>
                    <td><Skeleton variant="text" width="60px" /></td>
                  </tr>
                ))}
              </tbody>
            </table>
          ) : filteredMemories.length === 0 ? (
            <div style={{ textAlign: 'center', color: 'var(--text-tertiary)', padding: '3rem' }}>
              <p style={{ fontSize: '16px', marginBottom: '0.5rem' }}>
                {searchQuery || filterVisibility !== 'all'
                  ? 'No matching memories'
                  : 'No memories yet'}
              </p>
              <p style={{ fontSize: '14px', marginBottom: '1.5rem' }}>
                {searchQuery || filterVisibility !== 'all'
                  ? 'Try adjusting your filters'
                  : 'Create your first memory to get started'}
              </p>
              {!searchQuery && filterVisibility === 'all' && (
                <button className="btn btn-primary" onClick={handleCreateMemory}>
                  <FaPlus size={16} />
                  Create Memory
                </button>
              )}
            </div>
          ) : (
            <>
              <table className="data-table">
                <thead>
                  <tr>
                    <th>Title</th>
                    <th>Slug</th>
                    <th>Tags</th>
                    <th>Visibility</th>
                    <th>Updated</th>
                    <th style={{ width: '80px' }}>Actions</th>
                  </tr>
                </thead>
                <tbody>
                  {filteredMemories.map((memory) => (
                    <tr key={memory.slug} onClick={() => handleViewMemory(memory.slug)}>
                      <td style={{ fontWeight: 500 }}>{memory.title}</td>
                      <td style={{ fontFamily: 'var(--font-mono)', fontSize: '0.8rem', color: 'var(--text-tertiary)' }}>
                        {memory.slug}
                      </td>
                      <td>
                        <div style={{ display: 'flex', gap: '4px', flexWrap: 'wrap' }}>
                          {memory.tags?.slice(0, 2).map((tag) => (
                            <span
                              key={tag}
                              style={{
                                fontSize: '10px',
                                padding: '2px 6px',
                                borderRadius: '8px',
                                background: 'var(--surface)',
                                color: 'var(--text-secondary)',
                              }}
                            >
                              {tag}
                            </span>
                          ))}
                          {memory.tags?.length > 2 && (
                            <span style={{ fontSize: '10px', color: 'var(--text-tertiary)' }}>
                              +{memory.tags.length - 2}
                            </span>
                          )}
                        </div>
                      </td>
                      <td><VisibilityBadge visibility={memory.visibility} /></td>
                      <td style={{ color: 'var(--text-secondary)', fontSize: '0.8rem' }}>
                        {new Date(memory.timestamp).toLocaleString()}
                      </td>
                      <td>
                        <div style={{ display: 'flex', gap: '4px' }}>
                          <button
                            className="btn btn-ghost"
                            style={{ padding: '4px 6px' }}
                            onClick={(e) => {
                              e.stopPropagation()
                              handleEditMemory({ ...memory, content: '' } as MemoryDetail)
                            }}
                          >
                            <FaPenToSquare size={14} />
                          </button>
                          <button
                            className="btn btn-ghost"
                            style={{ padding: '4px 6px', color: '#ef4444' }}
                            onClick={(e) => handleDeleteMemory(memory.slug, e)}
                          >
                            <FaTrash size={14} />
                          </button>
                        </div>
                      </td>
                    </tr>
                  ))}
                </tbody>
              </table>

              {totalPages > 1 && (
                <div
                  style={{
                    display: 'flex',
                    justifyContent: 'center',
                    alignItems: 'center',
                    gap: '8px',
                    padding: '16px',
                    fontSize: '13px',
                    color: 'var(--text-secondary)',
                  }}
                >
                  <button
                    className="btn btn-ghost"
                    disabled={currentPage === 1}
                    onClick={() => setCurrentPage((p) => p - 1)}
                    style={{ padding: '6px 12px' }}
                  >
                    Prev
                  </button>
                  <span>
                    Page {currentPage} of {totalPages} ({totalMemories} memories)
                  </span>
                  <button
                    className="btn btn-ghost"
                    disabled={currentPage === totalPages}
                    onClick={() => setCurrentPage((p) => p + 1)}
                    style={{ padding: '6px 12px' }}
                  >
                    Next
                  </button>
                </div>
              )}
            </>
          )
        ) : (
          <div style={{ height: '100%', minHeight: '500px' }}>
            {graphLoading ? (
              <div style={{ display: 'flex', justifyContent: 'center', alignItems: 'center', height: '100%' }}>
                <Skeleton variant="text" width="200px" />
              </div>
            ) : graph ? (
              <MemoryGraph
                graph={graph}
                searchQuery={searchQuery}
                onNodeClick={handleViewMemory}
              />
            ) : (
              <div style={{ textAlign: 'center', color: 'var(--text-tertiary)', padding: '3rem' }}>
                <p>Failed to load graph</p>
                <button className="btn btn-secondary" onClick={loadGraph}>
                  Retry
                </button>
              </div>
            )}
          </div>
        )}
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
                {selectedMemory.slug} &bull; {new Date(selectedMemory.timestamp).toLocaleString()}
              </p>
              <div style={{ marginTop: '0.5rem', display: 'flex', gap: '8px', alignItems: 'center', flexWrap: 'wrap' }}>
                <VisibilityBadge visibility={selectedMemory.visibility} />
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
                {selectedMemory.visibility === 'shared' && selectedMemory.shared_to?.length > 0 && (
                  <span style={{ fontSize: '12px', color: 'var(--text-tertiary)' }}>
                    Shared with: {selectedMemory.shared_to.join(', ')}
                  </span>
                )}
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
              <ReactMarkdown remarkPlugins={[remarkGfm]} rehypePlugins={[rehypeHighlight]}>
                {selectedMemory.content}
              </ReactMarkdown>
            </div>

            {selectedMemory.relations && selectedMemory.relations.length > 0 && (
              <div>
                <h4 style={{ marginBottom: '0.5rem', fontSize: '13px', color: 'var(--text-secondary)' }}>
                  Linked Memories
                </h4>
                <div style={{ display: 'flex', gap: '8px', flexWrap: 'wrap' }}>
                  {selectedMemory.relations.map((relSlug) => (
                    <button
                      key={relSlug}
                      className="btn btn-ghost"
                      style={{ padding: '4px 10px', fontSize: '12px', fontFamily: 'var(--font-mono)' }}
                      onClick={() => handleViewMemory(relSlug)}
                    >
                      [[{relSlug}]]
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
                onClick={() => handleDeleteMemory(selectedMemory.slug, {} as React.MouseEvent)}
                style={{ color: '#ef4444', marginLeft: 'auto' }}
              >
                <FaTrash size={16} />
                Delete
              </button>
            </div>
          </div>
        )}

        {(modalMode === 'create' || modalMode === 'edit') && (
          <div style={{ display: 'flex', flexDirection: 'column', gap: '1rem', flex: 1, height: '100%' }}>
            {modalMode === 'create' && (
              <div className="input-group" style={{ marginBottom: 0 }}>
                <label htmlFor="slug">Slug (optional)</label>
                <input
                  id="slug"
                  type="text"
                  value={formSlug}
                  onChange={(e) => setFormSlug(autoCorrectSlug(e.target.value))}
                  placeholder="auto-generated if empty"
                />
                {formSlug && (
                  <div
                    style={{
                      fontSize: '12px',
                      color: 'var(--text-tertiary)',
                      marginTop: '4px',
                      fontFamily: 'var(--font-mono)',
                    }}
                  >
                    Slug: {formSlug}
                  </div>
                )}
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
                  Use [[slug]] to link memories
                </span>
              </label>
              <div style={{ overflow: 'hidden' }}>
                <MarkdownEditor
                  value={formContent}
                  onChange={setFormContent}
                  placeholder="Enter memory content... Use [[slug]] to link to other memories"
                  className="modal-mdx-editor"
                />
              </div>
            </div>
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
            <div className="input-group" style={{ marginBottom: 0 }}>
              <label htmlFor="visibility">Visibility</label>
              <select
                id="visibility"
                value={formVisibility}
                onChange={(e) => setFormVisibility(e.target.value as MemoryVisibility)}
                style={{
                  padding: '8px 12px',
                  borderRadius: '6px',
                  border: '1px solid var(--border)',
                  background: 'var(--background)',
                  color: 'var(--text)',
                }}
              >
                <option value="private">Private (only you)</option>
                <option value="global">Global (all agents)</option>
                <option value="shared">Shared (specific agents)</option>
              </select>
            </div>
            {formVisibility === 'shared' && (
              <div className="input-group" style={{ marginBottom: 0 }}>
                <label htmlFor="shared_to">Shared Agent IDs (comma-separated)</label>
                <input
                  id="shared_to"
                  type="text"
                  value={formSharedTo}
                  onChange={(e) => setFormSharedTo(e.target.value)}
                  placeholder="agent-id-1, agent-id-2"
                />
                <div style={{ fontSize: '12px', color: 'var(--text-tertiary)', marginTop: '4px' }}>
                  Enter agent IDs separated by commas
                </div>
              </div>
            )}
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
    </>
  )
}
