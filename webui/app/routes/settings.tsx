import { useEffect, useState, FormEvent } from 'react'
import { useParams } from 'react-router'
import { listApiKeys, createApiKey, deleteApiKey, changePassword } from '../services/vizier'
import { FiTrash2 } from 'react-icons/fi'
import type { ApiKey } from '../interfaces/types'

type Section = 'password' | 'api-keys'

export default function Settings() {
  const { agentId } = useParams()
  const [activeSection, setActiveSection] = useState<Section>('password')

  // Password change state
  const [currentPassword, setCurrentPassword] = useState('')
  const [newPassword, setNewPassword] = useState('')
  const [confirmPassword, setConfirmPassword] = useState('')
  const [passwordChanging, setPasswordChanging] = useState(false)
  const [passwordMessage, setPasswordMessage] = useState<{ type: 'success' | 'error', text: string } | null>(null)

  // API Keys state
  const [apiKeys, setApiKeys] = useState<ApiKey[]>([])
  const [loadingKeys, setLoadingKeys] = useState(false)
  const [showCreateKeyForm, setShowCreateKeyForm] = useState(false)
  const [newKeyName, setNewKeyName] = useState('')
  const [newKeyExpiry, setNewKeyExpiry] = useState('90')
  const [createdKey, setCreatedKey] = useState<string | null>(null)
  const [creatingKey, setCreatingKey] = useState(false)

  useEffect(() => {
    if (activeSection === 'api-keys') {
      loadApiKeys()
    }
  }, [activeSection])

  const loadApiKeys = async () => {
    try {
      setLoadingKeys(true)
      const response = await listApiKeys()
      setApiKeys(response.data || [])
    } catch (error) {
      console.error('Failed to load API keys:', error)
    } finally {
      setLoadingKeys(false)
    }
  }

  const handlePasswordChange = async (e: FormEvent) => {
    e.preventDefault()
    setPasswordMessage(null)

    if (newPassword !== confirmPassword) {
      setPasswordMessage({ type: 'error', text: 'New passwords do not match' })
      return
    }

    if (newPassword.length < 8) {
      setPasswordMessage({ type: 'error', text: 'Password must be at least 8 characters' })
      return
    }

    setPasswordChanging(true)
    try {
      await changePassword(currentPassword, newPassword)
      setPasswordMessage({ type: 'success', text: 'Password changed successfully' })
      setCurrentPassword('')
      setNewPassword('')
      setConfirmPassword('')
    } catch (error: any) {
      setPasswordMessage({
        type: 'error',
        text: error.response?.data?.message || 'Failed to change password'
      })
    } finally {
      setPasswordChanging(false)
    }
  }

  const handleCreateApiKey = async () => {
    if (!newKeyName.trim()) return

    setCreatingKey(true)
    try {
      const response = await createApiKey(newKeyName, parseInt(newKeyExpiry))
      setCreatedKey(response.data.key)
      await loadApiKeys()
      setNewKeyName('')
      setNewKeyExpiry('90')
    } catch (error) {
      console.error('Failed to create API key:', error)
      alert('Failed to create API key')
    } finally {
      setCreatingKey(false)
    }
  }

  const handleDeleteApiKey = async (keyId: string) => {
    if (!confirm('Are you sure you want to delete this API key?')) return

    try {
      await deleteApiKey(keyId)
      await loadApiKeys()
    } catch (error) {
      console.error('Failed to delete API key:', error)
      alert('Failed to delete API key')
    }
  }

  const closeCreatedKeyModal = () => {
    setCreatedKey(null)
    setShowCreateKeyForm(false)
  }

  return (
    <>
      {/* Header */}
      <div className="main-header">
        <div>
          <h3 style={{ margin: 0 }}>Settings</h3>
        </div>
      </div>

      {/* Mobile section nav (horizontal tabs) */}
      <div className="flex md:hidden border-b border-[var(--border)] px-4 gap-2 py-2">
        <button
          onClick={() => setActiveSection('password')}
          className={`px-3 py-1.5 text-sm font-medium rounded-t transition-colors ${activeSection === 'password' ? 'bg-[var(--surface)] text-[var(--text-primary)] border-b-2 border-[var(--accent-primary)]' : 'text-[var(--text-tertiary)]'}`}
        >
          Password
        </button>
        <button
          onClick={() => setActiveSection('api-keys')}
          className={`px-3 py-1.5 text-sm font-medium rounded-t transition-colors ${activeSection === 'api-keys' ? 'bg-[var(--surface)] text-[var(--text-primary)] border-b-2 border-[var(--accent-primary)]' : 'text-[var(--text-tertiary)]'}`}
        >
          API Keys
        </button>
      </div>

      <div className="flex" style={{ height: 'calc(100vh - 60px)' }}>
        {/* Section Navigation (desktop only) */}
        <div className="hidden md:block" style={{
          width: '200px',
          borderRight: '1px solid var(--border)',
          padding: '24px 16px',
        }}>
          <div
            className={`nav-item ${activeSection === 'password' ? 'active' : ''}`}
            onClick={() => setActiveSection('password')}
          >
            Password
          </div>
          <div
            className={`nav-item ${activeSection === 'api-keys' ? 'active' : ''}`}
            onClick={() => setActiveSection('api-keys')}
          >
            API Keys
          </div>
        </div>

        {/* Content */}
        <div className="flex-1 overflow-auto p-4 md:p-6">
          {/* Password Section */}
          {activeSection === 'password' && (
            <div style={{ maxWidth: '600px' }}>
              <h2 style={{ marginBottom: '1rem' }}>Change Password</h2>
              <p style={{
                color: 'var(--text-secondary)',
                marginBottom: '2rem',
                fontSize: '14px',
              }}>
                Update your account password. Make sure to use a strong password.
              </p>

              {passwordMessage && (
                <div style={{
                  padding: '12px',
                  background: passwordMessage.type === 'success' ? '#e8f5e9' : '#ffebee',
                  border: `1px solid ${passwordMessage.type === 'success' ? '#c8e6c9' : '#ffcdd2'}`,
                  borderRadius: '4px',
                  color: passwordMessage.type === 'success' ? '#2e7d32' : '#c62828',
                  fontSize: '14px',
                  marginBottom: '1rem',
                }}>
                  {passwordMessage.text}
                </div>
              )}

              <form onSubmit={handlePasswordChange} style={{
                display: 'flex',
                flexDirection: 'column',
                gap: '1rem',
              }}>
                <div className="input-group">
                  <label htmlFor="current-password">Current Password</label>
                  <input
                    id="current-password"
                    type="password"
                    value={currentPassword}
                    onChange={(e) => setCurrentPassword(e.target.value)}
                    required
                    disabled={passwordChanging}
                  />
                </div>

                <div className="input-group">
                  <label htmlFor="new-password">New Password</label>
                  <input
                    id="new-password"
                    type="password"
                    value={newPassword}
                    onChange={(e) => setNewPassword(e.target.value)}
                    required
                    minLength={8}
                    disabled={passwordChanging}
                  />
                </div>

                <div className="input-group">
                  <label htmlFor="confirm-password">Confirm New Password</label>
                  <input
                    id="confirm-password"
                    type="password"
                    value={confirmPassword}
                    onChange={(e) => setConfirmPassword(e.target.value)}
                    required
                    minLength={8}
                    disabled={passwordChanging}
                  />
                </div>

                <button
                  type="submit"
                  className="btn btn-primary"
                  disabled={passwordChanging}
                  style={{ alignSelf: 'flex-start' }}
                >
                  {passwordChanging ? 'Changing...' : 'Change Password'}
                </button>
              </form>
            </div>
          )}

          {/* API Keys Section */}
          {activeSection === 'api-keys' && (
            <div>
              <div style={{
                display: 'flex',
                justifyContent: 'space-between',
                alignItems: 'center',
                marginBottom: '1.5rem',
              }}>
                <div>
                  <h2 style={{ marginBottom: '0.5rem' }}>API Keys</h2>
                  <p style={{
                    color: 'var(--text-secondary)',
                    fontSize: '14px',
                  }}>
                    Manage API keys for programmatic access
                  </p>
                </div>
                {!showCreateKeyForm && (
                  <button
                    className="btn btn-primary"
                    onClick={() => setShowCreateKeyForm(true)}
                  >
                    + Create API Key
                  </button>
                )}
              </div>

              {/* Create Key Form */}
              {showCreateKeyForm && !createdKey && (
                <div className="card" style={{ marginBottom: '1.5rem' }}>
                  <h3 style={{ marginBottom: '1rem' }}>Create New API Key</h3>
                  <div style={{
                    display: 'flex',
                    flexDirection: 'column',
                    gap: '1rem',
                  }}>
                    <div className="input-group">
                      <label htmlFor="key-name">Key Name</label>
                      <input
                        id="key-name"
                        type="text"
                        value={newKeyName}
                        onChange={(e) => setNewKeyName(e.target.value)}
                        placeholder="My API Key"
                        required
                      />
                    </div>

                    <div className="input-group">
                      <label htmlFor="key-expiry">Expires In (days)</label>
                      <input
                        id="key-expiry"
                        type="number"
                        value={newKeyExpiry}
                        onChange={(e) => setNewKeyExpiry(e.target.value)}
                        min="1"
                        max="365"
                      />
                    </div>

                    <div style={{ display: 'flex', gap: '8px' }}>
                      <button
                        className="btn btn-primary"
                        onClick={handleCreateApiKey}
                        disabled={!newKeyName.trim() || creatingKey}
                      >
                        {creatingKey ? 'Creating...' : 'Create'}
                      </button>
                      <button
                        className="btn btn-secondary"
                        onClick={() => {
                          setShowCreateKeyForm(false)
                          setNewKeyName('')
                          setNewKeyExpiry('90')
                        }}
                        disabled={creatingKey}
                      >
                        Cancel
                      </button>
                    </div>
                  </div>
                </div>
              )}

              {/* API Keys List */}
              {loadingKeys ? (
                <div style={{ textAlign: 'center', padding: '2rem', color: 'var(--text-tertiary)' }}>
                  Loading API keys...
                </div>
              ) : apiKeys.length === 0 ? (
                <div style={{ textAlign: 'center', padding: '2rem', color: 'var(--text-tertiary)' }}>
                  No API keys yet. Create one to get started.
                </div>
              ) : (
                <div style={{
                  display: 'flex',
                  flexDirection: 'column',
                  gap: '1rem',
                }}>
                  {apiKeys.map((key) => (
                    <div key={key.id} className="card" style={{
                      display: 'flex',
                      justifyContent: 'space-between',
                      alignItems: 'center',
                    }}>
                      <div>
                        <h4 style={{ marginBottom: '0.25rem' }}>{key.name}</h4>
                        <div style={{
                          fontSize: '12px',
                          color: 'var(--text-tertiary)',
                        }}>
                          <div>Created: {new Date(key.created_at).toLocaleString()}</div>
                          {key.expires_at && (
                            <div>Expires: {new Date(key.expires_at).toLocaleString()}</div>
                          )}
                          {key.last_used_at && (
                            <div>Last used: {new Date(key.last_used_at).toLocaleString()}</div>
                          )}
                        </div>
                      </div>
                      <button
                        className="btn btn-ghost"
                        onClick={() => handleDeleteApiKey(key.id)}
                        style={{ color: '#c00' }}
                      >
                        <FiTrash2 size={16} />
                        <span>Delete</span>
                      </button>
                    </div>
                  ))}
                </div>
              )}
            </div>
          )}
        </div>
      </div>

      {/* Created Key Modal */}
      {createdKey && (
        <>
          <div
            style={{
              position: 'fixed',
              top: 0,
              left: 0,
              right: 0,
              bottom: 0,
              background: 'rgba(0, 0, 0, 0.5)',
              zIndex: 1000,
            }}
            onClick={closeCreatedKeyModal}
          />
          <div
            style={{
              position: 'fixed',
              top: '50%',
              left: '50%',
              transform: 'translate(-50%, -50%)',
              background: 'var(--background)',
              borderRadius: '8px',
              padding: '2rem',
              maxWidth: '600px',
              width: '90%',
              zIndex: 1001,
              border: '1px solid var(--border)',
            }}
          >
            <h2 style={{ marginBottom: '1rem' }}>API Key Created</h2>
            <p style={{
              color: '#c62828',
              background: '#ffebee',
              padding: '12px',
              borderRadius: '4px',
              fontSize: '14px',
              marginBottom: '1rem',
            }}>
              <strong>Important:</strong> This key will only be shown once. Make sure to copy it now.
            </p>
            <div style={{
              background: 'var(--surface)',
              padding: '16px',
              borderRadius: '4px',
              fontFamily: 'var(--font-mono)',
              fontSize: '14px',
              wordBreak: 'break-all',
              marginBottom: '1.5rem',
              border: '1px solid var(--border)',
            }}>
              {createdKey}
            </div>
            <div style={{ display: 'flex', gap: '8px' }}>
              <button
                className="btn btn-primary"
                onClick={() => {
                  navigator.clipboard.writeText(createdKey)
                  alert('API key copied to clipboard')
                }}
              >
                Copy to Clipboard
              </button>
              <button
                className="btn btn-secondary"
                onClick={closeCreatedKeyModal}
              >
                Close
              </button>
            </div>
          </div>
        </>
      )}
    </>
  )
}
