import { useEffect, useState } from 'react'
import { useParams } from 'react-router'
import {
  getAgentDocument,
  updateAgentDocument,
  getIdentityDocument,
  updateIdentityDocument,
  getHeartbeatDocument,
  updateHeartbeatDocument,
} from '../services/vizier'
import { useToastStore } from '../hooks/toastStore'
import MarkdownEditor from '../components/MarkdownEditor'

type DocumentType = 'agent' | 'identity' | 'heartbeat'

export default function DocumentManagement() {
  const { agentId } = useParams()
  const [activeTab, setActiveTab] = useState<DocumentType>('agent')
  const [content, setContent] = useState('')
  const [originalContent, setOriginalContent] = useState('')
  const [loading, setLoading] = useState(true)
  const [saving, setSaving] = useState(false)
  const [hasChanges, setHasChanges] = useState(false)

  const { addToast } = useToastStore()

  const tabs: { key: DocumentType; label: string; description: string }[] = [
    { key: 'agent', label: 'AGENT.md', description: 'Core conduct and self-evolution guidelines' },
    { key: 'identity', label: 'IDENTITY.md', description: 'Personality, name, role, and values' },
    { key: 'heartbeat', label: 'HEARTBEAT.md', description: 'Automated tasks for realtime-like behaviors' },
  ]

  useEffect(() => {
    loadDocument(activeTab)
  }, [agentId, activeTab])

  useEffect(() => {
    setHasChanges(content !== originalContent)
  }, [content, originalContent])

  const loadDocument = async (docType: DocumentType) => {
    if (!agentId) return

    setLoading(true)
    try {
      let response: { content: string }
      switch (docType) {
        case 'agent': {
          const res = await getAgentDocument(agentId)
          response = res.data
          break
        }
        case 'identity': {
          const res = await getIdentityDocument(agentId)
          response = res.data
          break
        }
        case 'heartbeat': {
          const res = await getHeartbeatDocument(agentId)
          response = res.data
          break
        }
      }
      const docContent = response?.content || ''
      setContent(docContent)
      setOriginalContent(docContent)
    } catch (error) {
      console.error(`Failed to load ${activeTab} document:`, error)
      addToast('error', `Failed to load ${activeTab.toUpperCase()}.md`, 'Document may not exist yet')
      setContent('')
      setOriginalContent('')
    } finally {
      setLoading(false)
    }
  }

  const handleSave = async () => {
    if (!agentId) return

    setSaving(true)
    try {
      switch (activeTab) {
        case 'agent':
          await updateAgentDocument(agentId, content)
          break
        case 'identity':
          await updateIdentityDocument(agentId, content)
          break
        case 'heartbeat':
          await updateHeartbeatDocument(agentId, content)
          break
      }
      setOriginalContent(content)
      addToast('success', `${activeTab.toUpperCase()}.md saved successfully`)
    } catch (error: any) {
      console.error(`Failed to save ${activeTab} document:`, error)
      addToast('error', `Failed to save ${activeTab.toUpperCase()}.md`, error.response?.data?.message || 'Please try again')
    } finally {
      setSaving(false)
    }
  }

  const handleReset = () => {
    setContent(originalContent)
  }

  if (loading) {
    return (
      <div className="main-body" style={{
        display: 'flex',
        alignItems: 'center',
        justifyContent: 'center',
      }}>
        Loading document...
      </div>
    )
  }

  return (
    <>
      {/* Header */}
      <div className="main-header">
        <div style={{ flex: 1 }}>
          <h3 style={{ margin: 0 }}>Agent Documents</h3>
        </div>
        {hasChanges && (
          <div style={{ display: 'flex', gap: '8px', alignItems: 'center' }}>
            <span style={{ fontSize: '12px', color: 'var(--text-tertiary)' }}>
              Unsaved changes
            </span>
            <button className="btn btn-ghost" onClick={handleReset}>
              Reset
            </button>
            <button className="btn btn-primary" onClick={handleSave} disabled={saving}>
              {saving ? 'Saving...' : 'Save'}
            </button>
          </div>
        )}
      </div>

      {/* Tabs */}
      <div className="main-body" style={{ paddingTop: '16px' }}>
        <div style={{
          display: 'flex',
          gap: '4px',
          marginBottom: '1rem',
          borderBottom: '1px solid var(--border)',
          paddingBottom: '4px',
        }}>
          {tabs.map((tab) => (
            <button
              key={tab.key}
              onClick={() => setActiveTab(tab.key)}
              style={{
                padding: '8px 16px',
                borderRadius: '4px 4px 0 0',
                border: 'none',
                background: activeTab === tab.key ? 'var(--surface)' : 'transparent',
                color: activeTab === tab.key ? 'var(--text-primary)' : 'var(--text-tertiary)',
                cursor: 'pointer',
                fontWeight: activeTab === tab.key ? '600' : '400',
                transition: 'all 0.15s ease',
              }}
            >
              {tab.label}
            </button>
          ))}
        </div>

        {/* Tab description */}
        <div className="flex flex-wrap items-center justify-between gap-2 mb-4">
          <div style={{
            fontSize: '14px',
            color: 'var(--text-secondary)',
          }}>
            {tabs.find(t => t.key === activeTab)?.description}
          </div>
        </div>

        {/* Editor */}
        <div style={{ height: 'calc(100vh - 250px)' }}>
          <MarkdownEditor
            key={activeTab}
            value={content}
            onChange={setContent}
            placeholder={`Enter ${activeTab.toUpperCase()}.md content...`}
            className="document-mdx-editor"
          />
        </div>
      </div>
    </>
  )
}