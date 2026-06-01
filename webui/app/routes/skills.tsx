import { useEffect, useState } from 'react'
import ReactMarkdown from 'react-markdown'
import remarkGfm from 'remark-gfm'
import rehypeHighlight from 'rehype-highlight'
import { useParams } from 'react-router'
import { FaPlus, FaTrash, FaPenToSquare } from 'react-icons/fa6'
import { Skeleton } from '../components/Skeleton'
import MarkdownEditor from '../components/MarkdownEditor'
import { useToastStore } from '../hooks/toastStore'
import { useSkillStore } from '../hooks/skillStore'
import type { Skill, SkillActivation } from '../interfaces/types'

function getErrorMessage(err: unknown): string {
  if (err && typeof err === 'object' && 'response' in err) {
    const resp = (err as { response?: { data?: { message?: string } } }).response
    return resp?.data?.message || 'An error occurred'
  }
  return 'An error occurred'
}

type ModalMode = 'create' | 'edit' | 'view' | null

export default function SkillsManagement() {
  const { agentId } = useParams()
  const { skills, loading, loadSkills, selectSkill, selectedSkill, clearSelection, createSkill, updateSkill, deleteSkill } = useSkillStore()
  const [modalMode, setModalMode] = useState<ModalMode>(null)
  const [filterActivation, setFilterActivation] = useState<SkillActivation | ''>('')

  const [formName, setFormName] = useState('')
  const [formDescription, setFormDescription] = useState('')
  const [formContent, setFormContent] = useState('')
  const [formKeywords, setFormKeywords] = useState('')
  const [formActivation, setFormActivation] = useState<SkillActivation>('OnDemand')
  const [submitting, setSubmitting] = useState(false)

  const { addToast } = useToastStore()

  useEffect(() => {
    loadSkills(agentId)
  }, [loadSkills, agentId])

  const filteredSkills = skills.filter(skill => {
    if (filterActivation && skill.activation !== filterActivation) return false
    return true
  })

  const handleViewSkill = async (slug: string) => {
    // Check if this is a global skill (no agent_id)
    const skill = skills.find(s => s.name === slug)
    if (skill && !skill.agent_id) {
      await selectSkill(slug)  // Fetch from global endpoint
    } else {
      await selectSkill(slug, agentId)  // Fetch from agent endpoint
    }
    setModalMode('view')
  }

  const handleEditSkill = async (skill: Skill) => {
    // Fetch full skill content if not already loaded
    if (!skill.content) {
      // Use global endpoint for global skills, agent endpoint for agent skills
      if (!skill.agent_id) {
        await selectSkill(skill.name)
      } else {
        await selectSkill(skill.name, agentId)
      }
      const fullSkill = useSkillStore.getState().selectedSkill
      if (fullSkill) {
        skill = fullSkill
      }
    }
    setFormName(skill.name)
    setFormDescription(skill.description)
    setFormContent(skill.content || '')
    setFormKeywords(skill.keywords.join(', '))
    setFormActivation(skill.activation)
    setModalMode('edit')
  }

  const handleCreateSkill = () => {
    setFormName('')
    setFormDescription('')
    setFormContent('')
    setFormKeywords('')
    setFormActivation('OnDemand')
    setModalMode('create')
  }

  const handleSubmit = async () => {
    if (!formName.trim() || !formDescription.trim()) return
    setSubmitting(true)
    try {
      const keywords = formKeywords.split(',').map(k => k.trim()).filter(k => k)
      if (modalMode === 'create') {
        await createSkill({
          name: formName,
          description: formDescription,
          content: formContent,
          keywords,
          activation: formActivation,
        }, agentId)
        addToast('success', 'Skill created successfully')
      } else if (modalMode === 'edit' && selectedSkill) {
        await updateSkill(selectedSkill.name, {
          description: formDescription,
          content: formContent,
          keywords,
          activation: formActivation,
        }, agentId)
        addToast('success', 'Skill updated successfully')
      }
      await loadSkills(agentId)
      closeModal()
    } catch (error: unknown) {
      console.error('Failed to save skill:', error)
      addToast('error', 'Failed to save skill', getErrorMessage(error))
    } finally {
      setSubmitting(false)
    }
  }

  const handleDeleteSkill = async (slug: string, e: React.MouseEvent) => {
    e.stopPropagation()
    if (!confirm('Are you sure you want to delete this skill?')) return
    try {
      await deleteSkill(slug, agentId)
      addToast('success', 'Skill deleted successfully')
      await loadSkills(agentId)
      closeModal()
    } catch (error) {
      console.error('Failed to delete skill:', error)
      addToast('error', 'Failed to delete skill', getErrorMessage(error))
    }
  }

  const closeModal = () => {
    setModalMode(null)
    clearSelection()
    setFormName('')
    setFormDescription('')
    setFormContent('')
    setFormKeywords('')
    setFormActivation('OnDemand')
  }

  const getActivationColor = (activation: SkillActivation) => {
    switch (activation) {
      case 'Always': return { bg: '#e8f5e9', color: '#2e7d32' }
      case 'OnDemand': return { bg: '#e3f2fd', color: '#1565c0' }
      case 'Contextual': return { bg: '#fff3e0', color: '#e65100' }
    }
  }

  return (
    <>
      <div className="main-header">
        <div style={{ flex: 1 }}>
          <h3 style={{ margin: 0 }}>Skill Management</h3>
        </div>
        <div style={{ display: 'flex', gap: '8px', alignItems: 'center' }}>
          <select
            value={filterActivation}
            onChange={(e) => setFilterActivation(e.target.value as SkillActivation | '')}
            style={{ padding: '8px 16px', borderRadius: '4px', border: '1px solid var(--border)', background: 'var(--background)' }}
          >
            <option value="">All Activations</option>
            <option value="Always">Always</option>
            <option value="OnDemand">On Demand</option>
            <option value="Contextual">Contextual</option>
          </select>
          <button className="btn btn-primary" onClick={handleCreateSkill}>
            <FaPlus size={16} />
            <span>New Skill</span>
          </button>
        </div>
      </div>

      <div className="main-body">
        {loading ? (
          <table className="data-table">
            <thead>
              <tr>
                <th>Name</th>
                <th>Description</th>
                <th>Keywords</th>
                <th>Source</th>
                <th>Activation</th>
                <th>Version</th>
                <th style={{ width: '80px' }}>Actions</th>
              </tr>
            </thead>
            <tbody>
              {[1, 2, 3, 4, 5].map((i) => (
                <tr key={i} style={{ cursor: 'default' }}>
                  <td><Skeleton variant="text" width="60%" /></td>
                  <td><Skeleton variant="text" width="80%" /></td>
                  <td><Skeleton variant="text" width="40%" /></td>
                  <td><Skeleton variant="text" width="50px" /></td>
                  <td><Skeleton variant="text" width="50px" /></td>
                  <td><Skeleton variant="text" width="30px" /></td>
                  <td><Skeleton variant="text" width="60px" /></td>
                </tr>
              ))}
            </tbody>
          </table>
        ) : filteredSkills.length === 0 ? (
          <div style={{ textAlign: 'center', color: 'var(--text-tertiary)', padding: '3rem' }}>
            <p style={{ fontSize: '16px', marginBottom: '0.5rem' }}>No skills yet</p>
            <p style={{ fontSize: '14px', marginBottom: '1.5rem' }}>Create your first skill to get started</p>
            <button className="btn btn-primary" onClick={handleCreateSkill}>
              <FaPlus size={16} />
              Create Skill
            </button>
          </div>
        ) : (
          <table className="data-table">
            <thead>
              <tr>
                <th>Name</th>
                <th>Description</th>
                <th>Keywords</th>
                <th>Source</th>
                <th>Activation</th>
                <th>Version</th>
                <th style={{ width: '80px' }}>Actions</th>
              </tr>
            </thead>
            <tbody>
              {filteredSkills.map((skill) => (
                <tr key={skill.name} onClick={() => handleViewSkill(skill.name)}>
                  <td style={{ fontWeight: 500 }}>{skill.name}</td>
                  <td style={{ color: 'var(--text-secondary)', fontSize: '0.9rem' }}>{skill.description}</td>
                  <td>
                    <div style={{ display: 'flex', gap: '4px', flexWrap: 'wrap' }}>
                      {skill.keywords.slice(0, 3).map((keyword) => (
                        <span key={keyword} style={{
                          padding: '2px 6px',
                          borderRadius: '4px',
                          fontSize: '11px',
                          background: 'var(--surface)',
                          border: '1px solid var(--border)',
                        }}>
                          {keyword}
                        </span>
                      ))}
                      {skill.keywords.length > 3 && (
                        <span style={{ fontSize: '11px', color: 'var(--text-tertiary)' }}>
                          +{skill.keywords.length - 3}
                        </span>
                      )}
                    </div>
                  </td>
                  <td>
                    <span style={{
                      padding: '2px 8px',
                      borderRadius: '12px',
                      fontSize: '11px',
                      fontWeight: 600,
                      background: skill.agent_id ? '#e8f5e9' : '#e3f2fd',
                      color: skill.agent_id ? '#2e7d32' : '#1565c0',
                    }}>
                      {skill.agent_id ? `Agent` : 'Global'}
                    </span>
                  </td>
                  <td>
                    <span style={{
                      padding: '2px 8px',
                      borderRadius: '12px',
                      fontSize: '11px',
                      fontWeight: 600,
                      ...getActivationColor(skill.activation),
                    }}>
                      {skill.activation}
                    </span>
                  </td>
                  <td style={{ fontFamily: 'var(--font-mono)', fontSize: '0.8rem' }}>{skill.version}</td>
                  <td>
                    <div style={{ display: 'flex', gap: '4px' }}>
                      <button
                        className="btn btn-ghost"
                        style={{ padding: '4px 6px' }}
                        onClick={(e) => {
                          e.stopPropagation()
                          handleEditSkill(skill)
                        }}
                      >
                        <FaPenToSquare size={14} />
                      </button>
                      <button
                        className="btn btn-ghost"
                        style={{ padding: '4px 6px', color: '#ef4444' }}
                        onClick={(e) => handleDeleteSkill(skill.name, e)}
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
            {modalMode === 'view' && selectedSkill && (
              <div style={{ padding: '2rem', overflow: 'auto', flex: 1 }}>
                <div style={{ display: 'flex', justifyContent: 'space-between', alignItems: 'flex-start', marginBottom: '1.5rem' }}>
                  <div>
                    <h2 style={{ marginBottom: '0.5rem' }}>{selectedSkill.name}</h2>
                    <p style={{ fontSize: '12px', color: 'var(--text-tertiary)' }}>
                      {selectedSkill.description} &bull; v{selectedSkill.version}
                    </p>
                  </div>
                  <button className="btn btn-ghost" onClick={closeModal} style={{ padding: '8px' }}>&#10005;</button>
                </div>
                <div style={{ marginBottom: '1.5rem' }}>
                  <div style={{ fontSize: '12px', fontWeight: 600, color: 'var(--text-secondary)', marginBottom: '8px' }}>Keywords</div>
                  <div style={{ display: 'flex', gap: '4px', flexWrap: 'wrap' }}>
                    {selectedSkill.keywords.map((keyword) => (
                      <span key={keyword} style={{
                        padding: '4px 8px',
                        borderRadius: '4px',
                        fontSize: '12px',
                        background: 'var(--surface)',
                        border: '1px solid var(--border)',
                      }}>
                        {keyword}
                      </span>
                    ))}
                  </div>
                </div>
                <div style={{ marginBottom: '1.5rem' }}>
                  <div style={{ fontSize: '12px', fontWeight: 600, color: 'var(--text-secondary)', marginBottom: '8px' }}>Activation</div>
                  <span style={{
                    padding: '4px 12px',
                    borderRadius: '12px',
                    fontSize: '12px',
                    fontWeight: 600,
                    ...getActivationColor(selectedSkill.activation),
                  }}>
                    {selectedSkill.activation}
                  </span>
                </div>
                <div style={{ marginBottom: '1.5rem' }}>
                  <div style={{ fontSize: '12px', fontWeight: 600, color: 'var(--text-secondary)', marginBottom: '8px' }}>Content</div>
                  <div className="prose" style={{
                    padding: '12px',
                    borderRadius: '4px',
                    background: 'var(--surface)',
                    border: '1px solid var(--border)',
                    maxHeight: '300px',
                    overflow: 'auto',
                  }}>
                    <ReactMarkdown remarkPlugins={[remarkGfm]} rehypePlugins={[rehypeHighlight]}>
                      {selectedSkill.content || 'No content'}
                    </ReactMarkdown>
                  </div>
                </div>
                {selectedSkill.resources.length > 0 && (
                  <div style={{ marginBottom: '1.5rem' }}>
                    <div style={{ fontSize: '12px', fontWeight: 600, color: 'var(--text-secondary)', marginBottom: '8px' }}>Resources</div>
                    <div style={{ display: 'flex', flexDirection: 'column', gap: '4px' }}>
                      {selectedSkill.resources.map((resource) => (
                        <div key={resource} style={{
                          padding: '8px 12px',
                          borderRadius: '4px',
                          background: 'var(--surface)',
                          border: '1px solid var(--border)',
                          fontFamily: 'var(--font-mono)',
                          fontSize: '12px',
                        }}>
                          {resource}
                        </div>
                      ))}
                    </div>
                  </div>
                )}
                <div style={{ display: 'flex', gap: '8px' }}>
                  <button className="btn btn-secondary" onClick={() => handleEditSkill(selectedSkill)}>
                    <FaPenToSquare size={16} />
                    Edit
                  </button>
                  <button className="btn btn-ghost" onClick={(e) => handleDeleteSkill(selectedSkill.name, e)} style={{ color: '#ef4444', marginLeft: 'auto' }}>
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
                    <h2 style={{ margin: 0 }}>{modalMode === 'create' ? 'Create Skill' : 'Edit Skill'}</h2>
                    <button className="btn btn-ghost" onClick={closeModal} style={{ padding: '8px' }}>&#10005;</button>
                  </div>
                </div>
                <div style={{ padding: '0 2rem', overflow: 'auto', flex: 1 }}>
                  <div style={{ display: 'flex', flexDirection: 'column', gap: '1rem' }}>
                    {modalMode === 'create' && (
                      <div className="input-group" style={{ marginBottom: 0 }}>
                        <label htmlFor="name">Name</label>
                        <input id="name" type="text" value={formName} onChange={(e) => setFormName(e.target.value)} required autoFocus placeholder="my-skill-name" />
                      </div>
                    )}
                    <div className="input-group" style={{ marginBottom: 0 }}>
                      <label htmlFor="description">Description</label>
                      <input id="description" type="text" value={formDescription} onChange={(e) => setFormDescription(e.target.value)} required placeholder="Short description of the skill" />
                    </div>
                    <div className="input-group" style={{ marginBottom: 0 }}>
                      <label htmlFor="keywords">Keywords (comma-separated)</label>
                      <input id="keywords" type="text" value={formKeywords} onChange={(e) => setFormKeywords(e.target.value)} placeholder="review, quality, security" />
                    </div>
                    <div className="input-group" style={{ marginBottom: 0 }}>
                      <label htmlFor="activation">Activation Mode</label>
                      <select
                        id="activation"
                        value={formActivation}
                        onChange={(e) => setFormActivation(e.target.value as SkillActivation)}
                        style={{ padding: '8px 16px', borderRadius: '4px', border: '1px solid var(--border)', background: 'var(--background)' }}
                      >
                        <option value="OnDemand">On Demand</option>
                        <option value="Always">Always</option>
                        <option value="Contextual">Contextual</option>
                      </select>
                    </div>
                    <div className="input-group" style={{ marginBottom: 0 }}>
                      <label htmlFor="content">Content (Markdown)</label>
                      <div style={{ height: '200px' }}>
                        <MarkdownEditor value={formContent} onChange={setFormContent} placeholder="Skill instructions in markdown..." className="modal-mdx-editor" />
                      </div>
                    </div>
                  </div>
                </div>
                <div style={{ padding: '1rem 2rem', borderTop: '1px solid var(--border)', display: 'flex', gap: '8px' }}>
                  <button className="btn btn-primary" onClick={handleSubmit} disabled={!formName.trim() || !formDescription.trim() || submitting} style={{ flex: 1, justifyContent: 'center' }}>
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