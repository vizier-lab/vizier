import { useState } from 'react'
import { useParams, useNavigate } from 'react-router'
import { deleteAgent } from '../services/vizier'
import { useToastStore } from '../hooks/toastStore'

export default function DangerZone() {
  const { agentId } = useParams()
  const navigate = useNavigate()
  const addToast = useToastStore((s) => s.addToast)

  const [deleteWorkspace, setDeleteWorkspace] = useState(false)
  const [deleting, setDeleting] = useState(false)
  const [deleteConfirm, setDeleteConfirm] = useState('')

  const handleDeleteAgent = async () => {
    if (!agentId || deleteConfirm !== agentId) return

    setDeleting(true)
    try {
      await deleteAgent(agentId, deleteWorkspace)
      addToast('success', `Agent "${agentId}" deleted`)
      navigate('/')
      window.location.reload()
    } catch (err: any) {
      addToast('error', err?.response?.data?.message || 'Failed to delete agent')
    } finally {
      setDeleting(false)
    }
  }

  if (!agentId) {
    return (
      <>
        <div className="main-header">
          <div><h3 style={{ margin: 0, color: '#ef4444' }}>Danger Zone</h3></div>
        </div>
        <div style={{ padding: '24px', color: 'var(--text-tertiary)' }}>
          No agent selected.
        </div>
      </>
    )
  }

  return (
    <>
      <div className="main-header">
        <div><h3 style={{ margin: 0, color: '#ef4444' }}>Danger Zone</h3></div>
      </div>

      <div style={{ padding: '24px' }}>
        <div style={{ maxWidth: '600px' }}>
          <p style={{
            color: 'var(--text-secondary)',
            marginBottom: '2rem',
            fontSize: '14px',
          }}>
            Irreversible actions for this agent.
          </p>

          <div style={{
            border: '1px solid #ef4444',
            borderRadius: '8px',
            padding: '1.5rem',
          }}>
            <h3 style={{ margin: '0 0 0.5rem 0', fontSize: '1rem' }}>Delete Agent</h3>
            <p style={{
              color: 'var(--text-secondary)',
              fontSize: '13px',
              marginBottom: '1rem',
            }}>
              Permanently delete <strong>{agentId}</strong> and all its data. This cannot be undone.
            </p>

            <label style={{
              display: 'flex',
              alignItems: 'center',
              gap: '0.5rem',
              fontSize: '13px',
              marginBottom: '1rem',
              cursor: 'pointer',
              color: 'var(--text-secondary)',
            }}>
              <input
                type="checkbox"
                checked={deleteWorkspace}
                onChange={(e) => setDeleteWorkspace(e.target.checked)}
              />
              Also delete workspace files (AGENT.md, IDENTITY.md, etc.)
            </label>

            <div style={{
              display: 'flex',
              alignItems: 'center',
              gap: '0.75rem',
              marginBottom: '0.5rem',
            }}>
              <span style={{ fontSize: '13px', color: 'var(--text-secondary)' }}>
                Type <code style={{ background: 'var(--surface)', padding: '2px 6px', borderRadius: '4px' }}>{agentId}</code> to confirm:
              </span>
            </div>
            <input
              style={{
                width: '100%',
                padding: '0.5rem 0.75rem',
                borderRadius: '0.375rem',
                border: '1px solid var(--border)',
                background: 'var(--bg-primary)',
                color: 'var(--text-primary)',
                fontSize: '0.875rem',
                marginBottom: '1rem',
                outline: 'none',
              }}
              placeholder={agentId}
              value={deleteConfirm}
              onChange={(e) => setDeleteConfirm(e.target.value)}
            />

            <button
              onClick={handleDeleteAgent}
              disabled={deleting || deleteConfirm !== agentId}
              style={{
                padding: '0.5rem 1.5rem',
                borderRadius: '0.375rem',
                border: 'none',
                background: deleteConfirm === agentId ? '#ef4444' : 'var(--border)',
                color: '#fff',
                cursor: deleteConfirm === agentId && !deleting ? 'pointer' : 'not-allowed',
                fontSize: '0.85rem',
                fontWeight: 500,
              }}
            >
              {deleting ? 'Deleting...' : 'Delete Agent'}
            </button>
          </div>
        </div>
      </div>
    </>
  )
}
