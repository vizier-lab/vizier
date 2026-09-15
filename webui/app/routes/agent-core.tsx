import { useState, useEffect, useCallback } from 'react'
import { useParams } from 'react-router'
import { FaClockRotateLeft } from 'react-icons/fa6'
import {
  diffCoreRevisions,
  getAgentCore,
  getCoreHistory,
  getCoreRevision,
  rollbackCore,
  updateAgentCore,
} from '../services/vizier'
import { useToastStore } from '../hooks/toastStore'
import MarkdownEditor from '../components/MarkdownEditor'
import SlideOver from '../components/SlideOver'
import VersionHistory, { type VersionHistorySource } from '../components/VersionHistory'

function getErrorMessage(err: unknown): string {
  if (err && typeof err === 'object' && 'response' in err) {
    const resp = (err as { response?: { data?: { message?: string } } }).response
    return resp?.data?.message || 'An error occurred'
  }
  return 'An error occurred'
}

export default function AgentCore() {
  const { agentId } = useParams()
  const addToast = useToastStore((s) => s.addToast)

  const [content, setContent] = useState('')
  const [original, setOriginal] = useState('')
  const [loading, setLoading] = useState(true)
  const [saving, setSaving] = useState(false)
  const [historyOpen, setHistoryOpen] = useState(false)

  const load = useCallback(async () => {
    if (!agentId) return
    setLoading(true)
    try {
      const res = await getAgentCore(agentId)
      const data = res.data?.content || ''
      setContent(data)
      setOriginal(data)
    } catch (err: unknown) {
      console.error('Failed to load CORE:', err)
      addToast('error', 'Failed to load CORE', getErrorMessage(err))
      setContent('')
      setOriginal('')
    } finally {
      setLoading(false)
    }
  }, [agentId, addToast])

  useEffect(() => {
    void load()
  }, [load])

  // CORE rows have no deletion state; map the CORE API types into the shared panel's shape.
  const historySource: VersionHistorySource | null = agentId
    ? {
        list: (offset, limit) =>
          getCoreHistory(agentId, offset, limit).then((res) => ({
            rows: res.data.revisions.map((r) => ({ ...r, deleted: false })),
            total: res.data.total,
          })),
        get: (seq) => getCoreRevision(agentId, seq).then((res) => ({ ...res.data, deleted: false })),
        diff: (to, from) => diffCoreRevisions(agentId, to, from).then((res) => res.data),
        rollback: (seq) => rollbackCore(agentId, seq).then((res) => res.data),
      }
    : null

  const handleSave = async () => {
    if (!agentId) return
    setSaving(true)
    try {
      await updateAgentCore(agentId, content)
      setOriginal(content)
      addToast('success', 'CORE saved')
    } catch (err: unknown) {
      addToast('error', 'Failed to save CORE', getErrorMessage(err))
    } finally {
      setSaving(false)
    }
  }

  const handleReset = () => {
    setContent(original)
  }

  const hasChanges = content !== original

  return (
    <>
      <div className="main-header">
        <h3 style={{ margin: 0 }}>Core</h3>
        <div style={{ display: 'flex', gap: '0.5rem', alignItems: 'center' }}>
          {hasChanges && (
            <>
              <span style={{ fontSize: '12px', color: 'var(--text-tertiary)' }}>
                Unsaved changes
              </span>
              <button
                className="btn btn-ghost"
                onClick={handleReset}
                disabled={saving}
              >
                Reset
              </button>
              <button
                className="btn btn-primary"
                onClick={handleSave}
                disabled={saving}
              >
                {saving ? 'Saving...' : 'Save'}
              </button>
            </>
          )}
          <button
            className="btn btn-ghost"
            onClick={() => setHistoryOpen(true)}
            disabled={loading || !agentId}
            title="Browse, compare and restore earlier versions of CORE"
          >
            <FaClockRotateLeft size={14} />
            History
          </button>
        </div>
      </div>

      <SlideOver open={historyOpen} onClose={() => setHistoryOpen(false)} title="History: CORE">
        {historySource && (
          <VersionHistory
            label="CORE"
            source={historySource}
            extraWarning={hasChanges ? 'Your unsaved editor changes will be discarded.' : undefined}
            onRolledBack={() => {
              void load()
            }}
          />
        )}
      </SlideOver>

      <div className="main-body" style={{ padding: '1.5rem' }}>
        <p
          style={{
            color: 'var(--text-secondary)',
            fontSize: '14px',
            marginBottom: '1rem',
            maxWidth: '600px',
          }}
        >
          Persistent memory and identity for the agent. This document is included
          in the agent's system prompt and can be updated by the agent itself using
          the <code>WRITE_CORE</code> tool.
        </p>

        {loading ? (
          <p style={{ color: 'var(--text-tertiary)' }}>Loading CORE...</p>
        ) : (
          <div style={{ height: 'calc(100vh - 200px)' }}>
            <MarkdownEditor
              value={content}
              onChange={setContent}
              placeholder="Enter CORE content..."
              className="document-mdx-editor"
            />
          </div>
        )}
      </div>
    </>
  )
}
