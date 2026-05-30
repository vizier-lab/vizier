import { useState, useEffect } from 'react'
import { useNavigate, useParams } from 'react-router'
import { useToastStore } from '../hooks/toastStore'
import { getAgentDetail, updateAgent } from '../services/vizier'
import type { CreateAgentRequest, AgentDetail } from '../interfaces/types'

const PROVIDERS = ['ollama', 'deepseek', 'openrouter', 'anthropic', 'openai', 'gemini']

export default function AgentEdit() {
  const { agentId } = useParams()
  const navigate = useNavigate()
  const addToast = useToastStore((s) => s.addToast)
  const [loading, setLoading] = useState(true)
  const [saving, setSaving] = useState(false)
  const [showAdvanced, setShowAdvanced] = useState(false)
  const [form, setForm] = useState<CreateAgentRequest>({
    agent_id: '',
    name: '',
    description: '',
    provider: 'ollama',
    model: '',
    system_prompt: '',
    thinking_depth: 10,
    session_memory_capacity: 10,
    tools: {
      shell_access: false,
      brave_search: false,
      vector_memory: true,
      discord: false,
      telegram: false,
      fetch: false,
      http_client: false,
    },
    prompt_timeout: '5m',
    heartbeat_interval: '30m',
    dream_interval: '24h',
  })

  useEffect(() => {
    if (!agentId) return
    const load = async () => {
      try {
        const res = await getAgentDetail(agentId)
        const d: AgentDetail = res.data
        setForm({
          agent_id: d.agent_id,
          name: d.name,
          description: d.description || '',
          provider: d.provider,
          model: d.model,
          system_prompt: d.system_prompt || '',
          thinking_depth: d.thinking_depth,
          session_memory_capacity: d.session_memory_capacity,
          tools: {
            shell_access: d.shell_access,
            brave_search: d.brave_search,
            vector_memory: d.vector_memory,
            discord: d.discord,
            telegram: d.telegram,
            fetch: d.fetch,
            http_client: d.http_client,
          },
          prompt_timeout: d.prompt_timeout,
          heartbeat_interval: d.heartbeat_interval,
          dream_interval: d.dream_interval,
        })
      } catch (err: any) {
        addToast('error', 'Failed to load agent config')
        navigate('/')
      } finally {
        setLoading(false)
      }
    }
    load()
  }, [agentId])

  const updateField = <K extends keyof CreateAgentRequest>(key: K, value: CreateAgentRequest[K]) => {
    setForm((prev) => ({ ...prev, [key]: value }))
  }

  const updateTool = (key: keyof NonNullable<CreateAgentRequest['tools']>, value: boolean) => {
    setForm((prev) => ({
      ...prev,
      tools: { ...prev.tools, [key]: value },
    }))
  }

  const handleSave = async () => {
    if (!agentId || !form.name.trim()) {
      addToast('error', 'Name is required')
      return
    }

    setSaving(true)
    try {
      await updateAgent(agentId, form)
      addToast('success', `Agent "${form.name}" updated`)
      setTimeout(() => {
        window.location.href = `/${agentId}/chat`
      }, 500)
    } catch (err: any) {
      addToast('error', err?.response?.data?.message || 'Failed to update agent')
    } finally {
      setSaving(false)
    }
  }

  const inputStyle: React.CSSProperties = {
    width: '100%',
    padding: '0.5rem 0.75rem',
    borderRadius: '0.375rem',
    border: '1px solid var(--border)',
    background: 'var(--bg-primary)',
    color: 'var(--text-primary)',
    fontSize: '0.875rem',
    outline: 'none',
  }

  const labelStyle: React.CSSProperties = {
    display: 'block',
    marginBottom: '0.25rem',
    fontSize: '0.8rem',
    fontWeight: 500,
    color: 'var(--text-secondary)',
  }

  const fieldStyle: React.CSSProperties = {
    display: 'flex',
    flexDirection: 'column',
    gap: '0.25rem',
  }

  if (loading) {
    return (
      <>
        <div className="main-header">
          <h3 style={{ margin: 0 }}>Edit Agent</h3>
        </div>
        <div className="main-body">
          <p style={{ color: 'var(--text-tertiary)' }}>Loading agent configuration...</p>
        </div>
      </>
    )
  }

  return (
    <>
      <div className="main-header">
        <h3 style={{ margin: 0 }}>Edit Agent</h3>
      </div>

      <div className="main-body" style={{ display: 'flex', justifyContent: 'center' }}>
        <div style={{ width: '100%', maxWidth: '720px', display: 'flex', flexDirection: 'column', gap: '1.25rem' }}>
          <section style={fieldStyle}>
            <label style={labelStyle}>Agent ID</label>
            <input
              style={{ ...inputStyle, opacity: 0.6 }}
              value={form.agent_id}
              disabled
            />
          </section>

          <section style={fieldStyle}>
            <label style={labelStyle}>Name *</label>
            <input
              style={inputStyle}
              placeholder="My Agent"
              value={form.name}
              onChange={(e) => updateField('name', e.target.value)}
            />
          </section>

          <section style={fieldStyle}>
            <label style={labelStyle}>Description</label>
            <input
              style={inputStyle}
              placeholder="A helpful assistant"
              value={form.description || ''}
              onChange={(e) => updateField('description', e.target.value)}
            />
          </section>

          <div style={{ display: 'flex', gap: '0.75rem' }}>
            <section style={{ ...fieldStyle, flex: 1 }}>
              <label style={labelStyle}>Provider</label>
              <select
                style={inputStyle}
                value={form.provider}
                onChange={(e) => {
                  const p = e.target.value
                  updateField('provider', p)
                }}
              >
                {PROVIDERS.map((p) => (
                  <option key={p} value={p}>{p}</option>
                ))}
              </select>
            </section>
            <section style={{ ...fieldStyle, flex: 1 }}>
              <label style={labelStyle}>Model</label>
              <input
                style={inputStyle}
                value={form.model}
                onChange={(e) => updateField('model', e.target.value)}
              />
            </section>
          </div>

          <section style={fieldStyle}>
            <label style={labelStyle}>System Prompt</label>
            <textarea
              style={{ ...inputStyle, minHeight: '120px', resize: 'vertical' }}
              placeholder="You are a helpful assistant..."
              value={form.system_prompt || ''}
              onChange={(e) => updateField('system_prompt', e.target.value)}
            />
          </section>

          <div style={{ display: 'flex', gap: '0.75rem' }}>
            <section style={{ ...fieldStyle, flex: 1 }}>
              <label style={labelStyle}>Thinking Depth</label>
              <input
                style={inputStyle}
                type="number"
                min={1}
                value={form.thinking_depth || 10}
                onChange={(e) => updateField('thinking_depth', parseInt(e.target.value) || 10)}
              />
            </section>
            <section style={{ ...fieldStyle, flex: 1 }}>
              <label style={labelStyle}>Memory Capacity</label>
              <input
                style={inputStyle}
                type="number"
                min={1}
                value={form.session_memory_capacity || 10}
                onChange={(e) => updateField('session_memory_capacity', parseInt(e.target.value) || 10)}
              />
            </section>
          </div>

          <button
            onClick={() => setShowAdvanced(!showAdvanced)}
            style={{
              background: 'none',
              border: 'none',
              color: 'var(--text-secondary)',
              cursor: 'pointer',
              fontSize: '0.8rem',
              padding: '0',
              textAlign: 'left',
            }}
          >
            {showAdvanced ? '▾ Hide Advanced' : '▸ Advanced Options'}
          </button>

          {showAdvanced && (
            <>
              <section style={fieldStyle}>
                <label style={labelStyle}>Tools</label>
                <div style={{ display: 'grid', gridTemplateColumns: '1fr 1fr', gap: '0.5rem' }}>
                  {([
                    ['shell_access', 'Shell Access'],
                    ['brave_search', 'Brave Search'],
                    ['vector_memory', 'Vector Memory'],
                    ['discord', 'Discord'],
                    ['telegram', 'Telegram'],
                    ['fetch', 'Fetch Webpage'],
                    ['http_client', 'HTTP Client'],
                  ] as const).map(([key, label]) => (
                    <label key={key} style={{ display: 'flex', alignItems: 'center', gap: '0.4rem', fontSize: '0.8rem', cursor: 'pointer' }}>
                      <input
                        type="checkbox"
                        checked={form.tools?.[key] ?? false}
                        onChange={(e) => updateTool(key, e.target.checked)}
                      />
                      {label}
                    </label>
                  ))}
                </div>
              </section>

              <div style={{ display: 'flex', gap: '0.75rem' }}>
                <section style={{ ...fieldStyle, flex: 1 }}>
                  <label style={labelStyle}>Prompt Timeout</label>
                  <input
                    style={inputStyle}
                    placeholder="5m"
                    value={form.prompt_timeout || ''}
                    onChange={(e) => updateField('prompt_timeout', e.target.value)}
                  />
                </section>
                <section style={{ ...fieldStyle, flex: 1 }}>
                  <label style={labelStyle}>Heartbeat Interval</label>
                  <input
                    style={inputStyle}
                    placeholder="30m"
                    value={form.heartbeat_interval || ''}
                    onChange={(e) => updateField('heartbeat_interval', e.target.value)}
                  />
                </section>
                <section style={{ ...fieldStyle, flex: 1 }}>
                  <label style={labelStyle}>Dream Interval</label>
                  <input
                    style={inputStyle}
                    placeholder="24h"
                    value={form.dream_interval || ''}
                    onChange={(e) => updateField('dream_interval', e.target.value)}
                  />
                </section>
              </div>
            </>
          )}

          <div style={{ display: 'flex', gap: '0.75rem', paddingTop: '0.5rem' }}>
            <button
              onClick={() => navigate(`/${agentId}/chat`)}
              style={{
                padding: '0.6rem 1.5rem',
                borderRadius: '0.375rem',
                border: '1px solid var(--border)',
                background: 'transparent',
                color: 'var(--text-primary)',
                cursor: 'pointer',
                fontSize: '0.85rem',
              }}
            >
              Cancel
            </button>
            <button
              onClick={handleSave}
              disabled={saving}
              style={{
                padding: '0.6rem 1.5rem',
                borderRadius: '0.375rem',
                border: 'none',
                background: saving ? 'var(--border)' : 'var(--accent-primary)',
                color: '#fff',
                cursor: saving ? 'not-allowed' : 'pointer',
                fontSize: '0.85rem',
                fontWeight: 500,
              }}
            >
              {saving ? 'Saving...' : 'Save Changes'}
            </button>
          </div>
        </div>
      </div>
    </>
  )
}
