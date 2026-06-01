import { useEffect, useState } from 'react'
import ReactMarkdown from 'react-markdown'
import remarkGfm from 'remark-gfm'
import rehypeHighlight from 'rehype-highlight'
import { useParams } from 'react-router'
import { listMemories, getMemory, createMemory, updateMemory, deleteMemory } from '../services/vizier'
import { autoCorrectSlug, autoCorrectSlugStrict } from '../utils/slug'
import { FaPlus, FaTrash, FaPenToSquare } from 'react-icons/fa6'
import { Skeleton } from '../components/Skeleton'
import { useToastStore } from '../hooks/toastStore'
import type { Memory, MemoryDetail } from '../interfaces/types'
import MarkdownEditor from '../components/MarkdownEditor'

function getErrorMessage(err: unknown): string {
  if (err && typeof err === 'object' && 'response' in err) {
    const resp = (err as { response?: { data?: { message?: string } } }).response
    return resp?.data?.message || 'An error occurred'
  }
  return 'An error occurred'
}

type ModalMode = 'create' | 'edit' | 'view' | null

export default function MemoryManagement() {
  const { agentId } = useParams()
  const [memories, setMemories] = useState<Memory[]>([])
  const [selectedMemory, setSelectedMemory] = useState<MemoryDetail | null>(null)
  const [loading, setLoading] = useState(true)
  const [modalMode, setModalMode] = useState<ModalMode>(null)

  const [formTitle, setFormTitle] = useState('')
  const [formContent, setFormContent] = useState('')
  const [formSlug, setFormSlug] = useState('')
  const [submitting, setSubmitting] = useState(false)

  const { addToast } = useToastStore()

  useEffect(() => {
    loadMemories()
  }, [agentId])

  const loadMemories = async () => {
    if (!agentId) return
    try {
      setLoading(true)
      const response = await listMemories(agentId)
      setMemories(response.data || [])
    } catch (error) {
      console.error('Failed to load memories:', error)
      addToast('error', 'Failed to load memories', 'Please try again')
    } finally {
      setLoading(false)
    }
  }

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

  const handleEditMemory = (memory: MemoryDetail) => {
    setSelectedMemory(memory)
    setFormTitle(memory.title)
    setFormContent(memory.content)
    setFormSlug(memory.slug)
    setModalMode('edit')
  }

  const handleCreateMemory = () => {
    setFormTitle('')
    setFormContent('')
    setFormSlug('')
    setModalMode('create')
  }

  const handleSubmit = async () => {
    if (!agentId || !formTitle.trim() || !formContent.trim()) return
    setSubmitting(true)
    try {
      const finalSlug = formSlug ? autoCorrectSlugStrict(formSlug) : undefined
      if (modalMode === 'create') {
        await createMemory(agentId, formTitle, formContent, finalSlug || undefined)
        addToast('success', 'Memory created successfully')
      } else if (modalMode === 'edit' && selectedMemory) {
        await updateMemory(agentId, selectedMemory.slug, formTitle, formContent)
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
  }

  return (
    <>
      <div className="main-header">
        <div style={{ flex: 1 }}>
          <h3 style={{ margin: 0 }}>Memory Management</h3>
        </div>
        <button className="btn btn-primary" onClick={handleCreateMemory}>
          <FaPlus size={16} />
          <span>New Memory</span>
        </button>
      </div>

      <div className="main-body">
        {loading ? (
          <table className="data-table">
            <thead>
              <tr>
                <th>Title</th>
                <th>Slug</th>
                <th>Updated</th>
                <th style={{ width: '80px' }}>Actions</th>
              </tr>
            </thead>
            <tbody>
              {[1, 2, 3, 4, 5].map((i) => (
                <tr key={i} style={{ cursor: 'default' }}>
                  <td><Skeleton variant="text" width="60%" /></td>
                  <td><Skeleton variant="text" width="40%" /></td>
                  <td><Skeleton variant="text" width="50%" /></td>
                  <td><Skeleton variant="text" width="60px" /></td>
                </tr>
              ))}
            </tbody>
          </table>
        ) : memories.length === 0 ? (
          <div style={{ textAlign: 'center', color: 'var(--text-tertiary)', padding: '3rem' }}>
            <p style={{ fontSize: '16px', marginBottom: '0.5rem' }}>No memories yet</p>
            <p style={{ fontSize: '14px', marginBottom: '1.5rem' }}>Create your first memory to get started</p>
            <button className="btn btn-primary" onClick={handleCreateMemory}>
              <FaPlus size={16} />
              Create Memory
            </button>
          </div>
        ) : (
          <table className="data-table">
            <thead>
              <tr>
                <th>Title</th>
                <th>Slug</th>
                <th>Updated</th>
                <th style={{ width: '80px' }}>Actions</th>
              </tr>
            </thead>
            <tbody>
              {memories.map((memory) => (
                <tr key={memory.slug} onClick={() => handleViewMemory(memory.slug)}>
                  <td style={{ fontWeight: 500 }}>{memory.title}</td>
                  <td style={{ fontFamily: 'var(--font-mono)', fontSize: '0.8rem', color: 'var(--text-tertiary)' }}>{memory.slug}</td>
                  <td style={{ color: 'var(--text-secondary)', fontSize: '0.8rem' }}>{new Date(memory.timestamp).toLocaleString()}</td>
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
        )}
      </div>

      {/* Modal */}
      {modalMode && (
        <>
          <div
            style={{ position: 'fixed', top: 0, left: 0, right: 0, bottom: 0, background: 'rgba(0, 0, 0, 0.5)', zIndex: 1000, backdropFilter: 'blur(4px)' }}
            onClick={closeModal}
          />
          <div
            style={{
              position: 'fixed', top: '50%', left: '50%', transform: 'translate(-50%, -50%)',
              background: 'var(--background)', borderRadius: '12px',
              maxWidth: '700px', width: '90%', maxHeight: '90vh',
              display: 'flex', flexDirection: 'column',
              zIndex: 1001, border: '1px solid var(--border)', boxShadow: 'var(--shadow-xl)',
            }}
          >
            {modalMode === 'view' && selectedMemory && (
              <div style={{ padding: '2rem', overflow: 'auto', flex: 1 }}>
                <div style={{ display: 'flex', justifyContent: 'space-between', alignItems: 'flex-start', marginBottom: '1.5rem' }}>
                  <div>
                    <h2 style={{ marginBottom: '0.5rem' }}>{selectedMemory.title}</h2>
                    <p style={{ fontSize: '12px', color: 'var(--text-tertiary)', fontFamily: 'var(--font-mono)' }}>
                      {selectedMemory.slug} &bull; {new Date(selectedMemory.timestamp).toLocaleString()}
                    </p>
                  </div>
                  <button className="btn btn-ghost" onClick={closeModal} style={{ padding: '8px' }}>&#10005;</button>
                </div>
                <div className="prose" style={{ marginBottom: '1.5rem', background: 'var(--surface)', padding: '1.5rem', borderRadius: '8px', border: '1px solid var(--border)' }}>
                  <ReactMarkdown remarkPlugins={[remarkGfm]} rehypePlugins={[rehypeHighlight]}>{selectedMemory.content}</ReactMarkdown>
                </div>
                <div style={{ display: 'flex', gap: '8px' }}>
                  <button className="btn btn-secondary" onClick={() => handleEditMemory(selectedMemory)}>
                    <FaPenToSquare size={16} />
                    Edit
                  </button>
                  <button className="btn btn-ghost" onClick={() => handleDeleteMemory(selectedMemory.slug, {} as React.MouseEvent)} style={{ color: '#ef4444', marginLeft: 'auto' }}>
                    <FaTrash size={16} />
                    Delete
                  </button>
                </div>
              </div>
            )}

            {(modalMode === 'create' || modalMode === 'edit') && (
              <>
                <div style={{ padding: '2rem', paddingBottom: 0 }}>
                  <div style={{ display: 'flex', justifyContent: 'space-between', alignItems: 'center', marginBottom: '1rem' }}>
                    <h2 style={{ margin: 0 }}>{modalMode === 'create' ? 'Create Memory' : 'Edit Memory'}</h2>
                    <button className="btn btn-ghost" onClick={closeModal} style={{ padding: '8px' }}>&#10005;</button>
                  </div>
                </div>
                <div style={{ padding: '0 2rem', overflow: 'auto', flex: 1 }}>
                  <div style={{ display: 'flex', flexDirection: 'column', gap: '1rem' }}>
                    {modalMode === 'create' && (
                      <div className="input-group" style={{ marginBottom: 0 }}>
                        <label htmlFor="slug">Slug (optional)</label>
                        <input id="slug" type="text" value={formSlug} onChange={(e) => setFormSlug(autoCorrectSlug(e.target.value))} placeholder="auto-generated if empty" />
                        {formSlug && (
                          <div style={{ fontSize: '12px', color: 'var(--text-tertiary)', marginTop: '4px', fontFamily: 'var(--font-mono)' }}>
                            Slug: {formSlug}
                          </div>
                        )}
                      </div>
                    )}
                    <div className="input-group" style={{ marginBottom: 0 }}>
                      <label htmlFor="title">Title</label>
                      <input id="title" type="text" value={formTitle} onChange={(e) => setFormTitle(e.target.value)} required autoFocus placeholder="Enter memory title" />
                    </div>
                    <div className="input-group" style={{ marginBottom: 0 }}>
                      <label htmlFor="content">Content</label>
                      <div style={{ height: '200px' }}>
                        <MarkdownEditor value={formContent} onChange={setFormContent} placeholder="Enter memory content..." className="modal-mdx-editor" />
                      </div>
                    </div>
                  </div>
                </div>
                <div style={{ padding: '1rem 2rem', borderTop: '1px solid var(--border)', display: 'flex', gap: '8px' }}>
                  <button className="btn btn-primary" onClick={handleSubmit} disabled={!formTitle.trim() || !formContent.trim() || submitting} style={{ flex: 1, justifyContent: 'center' }}>
                    {submitting ? 'Saving...' : 'Save'}
                  </button>
                  <button className="btn btn-secondary" onClick={closeModal} disabled={submitting}>
                    Cancel
                  </button>
                </div>
              </>
            )}
          </div>
        </>
      )}
    </>
  )
}
