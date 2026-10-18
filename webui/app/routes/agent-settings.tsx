import { useState, useEffect } from 'react'
import { useParams, useNavigate } from 'react-router'
import { FaTriangleExclamation, FaUserGroup } from 'react-icons/fa6'
import {
  getAgentDetail,
  updateAgent,
  deleteAgent,
  getAgentSharing,
  updateAgentSharing,
  listUsers,
} from '../services/vizier'
import AgentForm from '../components/AgentForm'
import { useToastStore } from '../hooks/toastStore'
import { useAgentStore } from '../hooks/agentStore'
import { isBuildFailure, useBuildStore } from '../hooks/buildStore'
import type { CreateAgentRequest, AgentDetail, User } from '../interfaces/types'

function getErrorMessage(err: unknown): string {
  if (err && typeof err === 'object' && 'response' in err) {
    const resp = (err as { response?: { data?: { message?: string } } })
      .response
    return resp?.data?.message || 'An error occurred'
  }
  return 'An error occurred'
}

const inputStyle: React.CSSProperties = {
  width: '100%',
  padding: '0.5rem 0.75rem',
  borderRadius: '0.375rem',
  border: '1px solid var(--border)',
  background: 'var(--surface)',
  color: 'var(--text-primary)',
  fontSize: '0.875rem',
  outline: 'none',
}

const fieldStyle: React.CSSProperties = {
  display: 'flex',
  flexDirection: 'column',
  gap: '0.25rem',
}

/** Runs `run` once when first shown: the Sharing tab loads its data on first open. */
function OnFirstShow({ run, children }: { run: () => void; children: React.ReactNode }) {
  useEffect(run, [])
  return <>{children}</>
}

const labelStyle: React.CSSProperties = {
  display: 'block',
  marginBottom: '0.25rem',
  fontSize: '0.8rem',
  fontWeight: 500,
  color: 'var(--text-secondary)',
}

export function meta() {
  return [{ title: 'Config - Vizier' }]
}

export default function AgentSettings() {
  const { agentId } = useParams()
  const navigate = useNavigate()
  const addToast = useToastStore((s) => s.addToast)
  const loadAgents = useAgentStore((s) => s.loadAgents)
  const begin = useBuildStore((s) => s.begin)
  const ready = useBuildStore((s) => s.ready)
  const fail = useBuildStore((s) => s.fail)
  const close = useBuildStore((s) => s.close)

  const [detail, setDetail] = useState<AgentDetail | null>(null)
  const [sharingOpened, setSharingOpened] = useState(false)

  // ── Danger state ──
  const [deleteWorkspace, setDeleteWorkspace] = useState(false)
  const [deleting, setDeleting] = useState(false)
  const [deleteConfirm, setDeleteConfirm] = useState('')

  // ── Sharing state ──
  const [sharedTo, setSharedTo] = useState<string[]>([])
  const [sharingLoading, setSharingLoading] = useState(false)
  const [sharingSaving, setSharingSaving] = useState(false)
  const [allUsers, setAllUsers] = useState<User[]>([])
  const [newUserUsername, setNewUserUsername] = useState('')

  useEffect(() => {
    if (!agentId) return
    getAgentDetail(agentId)
      .then((res) => setDetail(res.data))
      .catch(() => {
        addToast('error', 'Failed to load agent config')
        navigate('/')
      })
  }, [agentId])

  // Sharing data loads the first time its tab is opened.
  useEffect(() => {
    if (!sharingOpened || !agentId) return
    const loadSharing = async () => {
      setSharingLoading(true)
      try {
        const [sharingRes, usersRes] = await Promise.all([
          getAgentSharing(agentId),
          listUsers(),
        ])
        setSharedTo(sharingRes.data?.shared_to || [])
        setAllUsers(usersRes.data || [])
      } catch {
        addToast('error', 'Failed to load sharing data')
      } finally {
        setSharingLoading(false)
      }
    }
    loadSharing()
  }, [agentId, sharingOpened])

  const handleSave = async (form: CreateAgentRequest) => {
    if (!agentId) return
    // Retry is fired from the modal, which has no one to hand a rejection to.
    const buildId = begin(agentId, `Updating agent "${form.name}"`, () => {
      handleSave(form).catch(() => {})
    })
    try {
      const res = await updateAgent(agentId, form)
      for (const w of res.data?.warnings || []) addToast('warning', w.message)
      addToast('success', `Agent "${form.name}" updated`)
      ready(buildId)
      loadAgents()
    } catch (err: unknown) {
      const message = getErrorMessage(err) || 'Failed to update agent'
      if (isBuildFailure(message)) {
        // Shown in the modal, with Retry; a toast as well would say it twice.
        fail(buildId, message)
      } else {
        close(buildId)
        addToast('error', message)
      }
      throw err
    }
  }

  // ── Sharing helpers ──
  const handleAddSharedUser = async () => {
    if (!agentId || !newUserUsername.trim()) return
    const user = allUsers.find((u) => u.username === newUserUsername.trim())
    if (!user) {
      addToast('error', 'User not found')
      return
    }
    if (sharedTo.includes(user.user_id)) {
      addToast('error', 'User already has access')
      return
    }
    setSharingSaving(true)
    try {
      const res = await updateAgentSharing(agentId, {
        add: [user.user_id],
      })
      setSharedTo(res.data?.shared_to || [])
      setNewUserUsername('')
      addToast('success', `Shared with ${user.username}`)
    } catch (err: unknown) {
      addToast('error', getErrorMessage(err) || 'Failed to share agent')
    } finally {
      setSharingSaving(false)
    }
  }

  const handleRemoveSharedUser = async (userId: string) => {
    if (!agentId) return
    setSharingSaving(true)
    try {
      const res = await updateAgentSharing(agentId, {
        remove: [userId],
      })
      setSharedTo(res.data?.shared_to || [])
      addToast('success', 'Access removed')
    } catch (err: unknown) {
      addToast('error', getErrorMessage(err) || 'Failed to remove access')
    } finally {
      setSharingSaving(false)
    }
  }

  // ── Danger helpers ──
  const handleDeleteAgent = async () => {
    if (!agentId || deleteConfirm !== agentId) return
    setDeleting(true)
    try {
      await deleteAgent(agentId, deleteWorkspace)
      addToast('success', `Agent "${agentId}" deleted`)
      navigate('/')
      window.location.reload()
    } catch (err: unknown) {
      addToast('error', getErrorMessage(err) || 'Failed to delete agent')
    } finally {
      setDeleting(false)
    }
  }

  const sharingTab = () => {
    return (
      <OnFirstShow run={() => setSharingOpened(true)}>
        <div style={{ maxWidth: '720px' }}>
          <p
            style={{
              color: 'var(--text-secondary)',
              marginBottom: '1.5rem',
              fontSize: '14px',
            }}
          >
            Share this agent with other users. Shared users can view and use the agent but cannot edit its configuration.
          </p>

          {sharingLoading ? (
            <p style={{ color: 'var(--text-tertiary)' }}>
              Loading sharing data...
            </p>
          ) : (
            <div
              style={{
                display: 'flex',
                flexDirection: 'column',
                gap: '1.5rem',
              }}
            >
              {/* Add user section */}
              <div>
                <h4
                  style={{
                    fontSize: '0.85rem',
                    fontWeight: 600,
                    color: 'var(--text-primary)',
                    marginBottom: '0.75rem',
                    paddingBottom: '0.5rem',
                    borderBottom: '1px solid var(--border)',
                  }}
                >
                  Add User
                </h4>
                <div
                  style={{
                    display: 'flex',
                    gap: '0.5rem',
                    alignItems: 'flex-end',
                  }}
                >
                  <div style={{ flex: 1, ...fieldStyle }}>
                    <label style={labelStyle}>Username</label>
                    <input
                      style={inputStyle}
                      placeholder="Enter username"
                      value={newUserUsername}
                      onChange={(e) =>
                        setNewUserUsername(e.target.value)
                      }
                      onKeyDown={(e) => {
                        if (e.key === 'Enter') {
                          e.preventDefault()
                          handleAddSharedUser()
                        }
                      }}
                    />
                  </div>
                  <button
                    className="btn btn-primary"
                    onClick={handleAddSharedUser}
                    disabled={sharingSaving || !newUserUsername.trim()}
                    style={{
                      padding: '0.5rem 1rem',
                      fontSize: '0.85rem',
                      height: 'fit-content',
                    }}
                  >
                    {sharingSaving ? 'Adding...' : 'Add'}
                  </button>
                </div>
              </div>

              {/* Shared users list */}
              <div>
                <h4
                  style={{
                    fontSize: '0.85rem',
                    fontWeight: 600,
                    color: 'var(--text-primary)',
                    marginBottom: '0.75rem',
                    paddingBottom: '0.5rem',
                    borderBottom: '1px solid var(--border)',
                  }}
                >
                  Shared With ({sharedTo.length})
                </h4>
                {sharedTo.length === 0 ? (
                  <p
                    style={{
                      color: 'var(--text-tertiary)',
                      fontSize: '0.85rem',
                    }}
                  >
                    This agent is not shared with anyone yet.
                  </p>
                ) : (
                  <div
                    style={{
                      display: 'flex',
                      flexDirection: 'column',
                      gap: '0.5rem',
                    }}
                  >
                    {sharedTo.map((userId) => {
                      const user = allUsers.find(
                        (u) => u.user_id === userId
                      )
                      return (
                        <div
                          key={userId}
                          style={{
                            display: 'flex',
                            alignItems: 'center',
                            justifyContent: 'space-between',
                            padding: '0.5rem 0.75rem',
                            border: '1px solid var(--border)',
                            borderRadius: '0.375rem',
                            background: 'var(--surface)',
                          }}
                        >
                          <span
                            style={{
                              fontSize: '0.85rem',
                              color: 'var(--text-primary)',
                            }}
                          >
                            {user?.username || userId}
                          </span>
                          <button
                            className="btn btn-ghost"
                            onClick={() =>
                              handleRemoveSharedUser(userId)
                            }
                            disabled={sharingSaving}
                            style={{
                              fontSize: '0.8rem',
                              padding: '4px 8px',
                              color: '#ef4444',
                            }}
                          >
                            Remove
                          </button>
                        </div>
                      )
                    })}
                  </div>
                )}
              </div>
            </div>
          )}
        </div>
      </OnFirstShow>
    )
  }

  const dangerTab = () => (
      <div style={{ maxWidth: '600px' }}>
        <p
          style={{
            color: 'var(--text-secondary)',
            marginBottom: '1.5rem',
            fontSize: '14px',
          }}
        >
          Irreversible actions for this agent.
        </p>
        <div
          style={{
            border: '1px solid #ef4444',
            borderRadius: '8px',
            padding: '1.5rem',
          }}
        >
          <h3
            style={{
              margin: '0 0 0.5rem 0',
              fontSize: '1rem',
            }}
          >
            Delete Agent
          </h3>
          <p
            style={{
              color: 'var(--text-secondary)',
              fontSize: '13px',
              marginBottom: '1rem',
            }}
          >
            Permanently delete{' '}
            <strong>{agentId}</strong> and all its data.
            This cannot be undone.
          </p>
          <label
            style={{
              display: 'flex',
              alignItems: 'center',
              gap: '0.5rem',
              fontSize: '13px',
              marginBottom: '1rem',
              cursor: 'pointer',
              color: 'var(--text-secondary)',
            }}
          >
            <input
              type="checkbox"
              checked={deleteWorkspace}
              onChange={(e) =>
                setDeleteWorkspace(e.target.checked)
              }
            />
            Also delete the agent's stored data (skills, memories, etc.)
          </label>
          <div
            style={{
              display: 'flex',
              alignItems: 'center',
              gap: '0.75rem',
              marginBottom: '0.5rem',
            }}
          >
            <span
              style={{
                fontSize: '13px',
                color: 'var(--text-secondary)',
              }}
            >
              Type{' '}
              <code
                style={{
                  background: 'var(--surface)',
                  padding: '2px 6px',
                  borderRadius: '4px',
                }}
              >
                {agentId}
              </code>{' '}
              to confirm:
            </span>
          </div>
          <input
            style={{
              ...inputStyle,
              marginBottom: '1rem',
            }}
            placeholder={agentId}
            value={deleteConfirm}
            onChange={(e) =>
              setDeleteConfirm(e.target.value)
            }
          />
          <button
            onClick={handleDeleteAgent}
            disabled={
              deleting || deleteConfirm !== agentId
            }
            style={{
              padding: '0.5rem 1.5rem',
              borderRadius: '0.375rem',
              border: 'none',
              background:
                deleteConfirm === agentId
                  ? '#ef4444'
                  : 'var(--border)',
              color: '#fff',
              cursor:
                deleteConfirm === agentId &&
                  !deleting
                  ? 'pointer'
                  : 'not-allowed',
              fontSize: '0.85rem',
              fontWeight: 500,
            }}
          >
            {deleting ? 'Deleting...' : 'Delete Agent'}
          </button>
        </div>
      </div>
  )

  return (
    <>
      <div className="main-header">
        <h3 style={{ margin: 0 }}>Config</h3>
      </div>
      <div className="main-body" style={{ padding: 0 }}>
        {detail ? (
          <AgentForm
            mode="edit"
            agentId={agentId}
            initialData={detail}
            onSubmit={handleSave}
            onCancel={() => navigate(`/${agentId}/chat`)}
            extraTabs={[
              { key: 'sharing', label: 'Sharing', icon: FaUserGroup, render: sharingTab },
              { key: 'danger', label: 'Danger Zone', icon: FaTriangleExclamation, render: dangerTab },
            ]}
          />
        ) : (
          <p style={{ color: 'var(--text-tertiary)', padding: '24px' }}>
            Loading agent configuration...
          </p>
        )}
      </div>
    </>
  )
}
