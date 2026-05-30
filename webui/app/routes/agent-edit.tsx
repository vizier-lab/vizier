import { useState, useEffect } from 'react'
import { useNavigate, useParams } from 'react-router'
import { useToastStore } from '../hooks/toastStore'
import { getAgentDetail, updateAgent, listGlobalConfigs } from '../services/vizier'
import TooltipLabel from '../components/TooltipLabel'
import type { CreateAgentRequest, AgentDetail, GlobalConfigEntry } from '../interfaces/types'

const PROVIDERS = ['ollama', 'deepseek', 'openrouter', 'anthropic', 'openai', 'gemini', 'mimo']

export default function AgentEdit() {
  const { agentId } = useParams()
  const navigate = useNavigate()
  const addToast = useToastStore((s) => s.addToast)
  const [loading, setLoading] = useState(true)
  const [saving, setSaving] = useState(false)
  const [showAdvanced, setShowAdvanced] = useState(false)
  const [availableMcpServers, setAvailableMcpServers] = useState<string[]>([])
  const [form, setForm] = useState<CreateAgentRequest>({
    agent_id: '',
    name: '',
    description: '',
    provider: 'ollama',
    model: '',
    system_prompt: '',
    thinking_depth: 10,
    session_memory_capacity: 10,
    max_tokens: 100000,
    tools: {
      shell_access: false,
      brave_search: false,
      brave_search_settings: {},
      vector_memory: true,
      discord: false,
      telegram: false,
      fetch: false,
      http_client: false,
      timeout: '1m',
      mcp_servers: [],
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
          max_tokens: d.max_tokens,
          tools: {
            shell_access: d.shell_access,
            brave_search: d.brave_search,
            brave_search_settings: d.brave_search_settings || {},
            vector_memory: d.vector_memory,
            discord: d.discord,
            telegram: d.telegram,
            fetch: d.fetch,
            http_client: d.http_client,
            timeout: d.tools_timeout || '1m',
            mcp_servers: d.mcp_servers || [],
          },
          prompt_timeout: d.prompt_timeout,
          heartbeat_interval: d.heartbeat_interval,
          dream_interval: d.dream_interval,
          discord_token: d.discord_token || '',
          telegram_token: d.telegram_token || '',
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

  const updateToolField = (key: string, value: string | string[]) => {
    setForm((prev) => ({
      ...prev,
      tools: { ...prev.tools, [key]: value },
    }))
  }

  const toggleMcpServer = (name: string) => {
    setForm((prev) => {
      const current = prev.tools?.mcp_servers ?? []
      const next = current.includes(name)
        ? current.filter((s) => s !== name)
        : [...current, name]
      return { ...prev, tools: { ...prev.tools, mcp_servers: next } }
    })
  }

  useEffect(() => {
    const loadMcpServers = async () => {
      try {
        const response = await listGlobalConfigs()
        const entries: GlobalConfigEntry[] = response.data || []
        const mcpEntry = entries.find((e) => e.key === 'mcp_servers')
        if (mcpEntry && mcpEntry.value.type === 'McpServers') {
          setAvailableMcpServers(Object.keys(mcpEntry.value.data))
        }
      } catch {
        // silently ignore — MCP servers are optional
      }
    }
    loadMcpServers()
  }, [])

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
    background: 'var(--surface)',
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
            <label style={labelStyle}>
              <TooltipLabel label="Agent ID" tooltip="Unique identifier for the agent. Lowercase letters, numbers, hyphens, and underscores only. Cannot be changed after creation." />
            </label>
            <input
              style={{ ...inputStyle, opacity: 0.6 }}
              value={form.agent_id}
              disabled
            />
          </section>

          <section style={fieldStyle}>
            <label style={labelStyle}>
              <TooltipLabel label="Name" tooltip="Display name for this agent." />
              {' *'}
            </label>
            <input
              style={inputStyle}
              placeholder="My Agent"
              value={form.name}
              onChange={(e) => updateField('name', e.target.value)}
            />
          </section>

          <section style={fieldStyle}>
            <label style={labelStyle}>
              <TooltipLabel label="Description" tooltip="Optional description of what this agent does." />
            </label>
            <input
              style={inputStyle}
              placeholder="A helpful assistant"
              value={form.description || ''}
              onChange={(e) => updateField('description', e.target.value)}
            />
          </section>

          <div style={{ display: 'flex', gap: '0.75rem' }}>
            <section style={{ ...fieldStyle, flex: 1 }}>
              <label style={labelStyle}>
                <TooltipLabel label="Provider" tooltip="The AI provider to use for completions (e.g. ollama, openai, anthropic)." />
              </label>
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
              <label style={labelStyle}>
                <TooltipLabel label="Model" tooltip="The model identifier for the selected provider (e.g. gpt-4o-mini, claude-3-haiku)." />
              </label>
              <input
                style={inputStyle}
                value={form.model}
                onChange={(e) => updateField('model', e.target.value)}
              />
            </section>
          </div>

          <section style={fieldStyle}>
            <label style={labelStyle}>
              <TooltipLabel label="System Prompt" tooltip="Instructions that define the agent's behavior, personality, and capabilities." />
            </label>
            <textarea
              style={{ ...inputStyle, minHeight: '120px', resize: 'vertical' }}
              placeholder="You are a helpful assistant..."
              value={form.system_prompt || ''}
              onChange={(e) => updateField('system_prompt', e.target.value)}
            />
          </section>

          <div style={{ display: 'flex', gap: '0.75rem' }}>
            <section style={{ ...fieldStyle, flex: 1 }}>
              <label style={labelStyle}>
                <TooltipLabel label="Thinking Depth" tooltip="Maximum number of LLM reasoning turns per request. Each turn allows one completion call plus any tool executions. Higher values enable deeper reasoning but increase latency. Set to 0 for unlimited." />
              </label>
              <input
                style={inputStyle}
                type="number"
                min={1}
                value={form.thinking_depth || 10}
                onChange={(e) => updateField('thinking_depth', parseInt(e.target.value) || 10)}
              />
            </section>
            <section style={{ ...fieldStyle, flex: 1 }}>
              <label style={labelStyle}>
                <TooltipLabel label="Max Tokens" tooltip="Maximum output tokens per LLM completion request. Passed directly to the provider API. Leave empty to use the provider's default limit." />
              </label>
              <input
                style={inputStyle}
                type="number"
                min={1}
                placeholder="No limit"
                value={form.max_tokens ?? ''}
                onChange={(e) => updateField('max_tokens', e.target.value ? parseInt(e.target.value) : undefined)}
              />
            </section>
            <section style={{ ...fieldStyle, flex: 1 }}>
              <label style={labelStyle}>
                <TooltipLabel label="Memory Capacity" tooltip="Maximum number of recent conversation messages loaded as context for each request. Higher values give more conversational context but increase token usage." />
              </label>
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
                <label style={labelStyle}>
                  <TooltipLabel label="Tools" tooltip="Capabilities the agent can use during conversations and tasks." />
                </label>
                <div style={{ display: 'grid', gridTemplateColumns: '1fr 1fr', gap: '0.5rem' }}>
                  {([
                    ['shell_access', 'Shell Access'],
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

                {/* Brave Search with settings */}
                <div style={{ marginTop: '0.75rem', padding: '0.75rem', border: '1px solid var(--border)', borderRadius: '0.5rem' }}>
                  <label style={{ display: 'flex', alignItems: 'center', gap: '0.4rem', fontSize: '0.8rem', cursor: 'pointer', marginBottom: form.tools?.brave_search ? '0.75rem' : 0 }}>
                    <input
                      type="checkbox"
                      checked={form.tools?.brave_search ?? false}
                      onChange={(e) => updateTool('brave_search', e.target.checked)}
                    />
                    Brave Search
                  </label>
                  {form.tools?.brave_search && (
                    <div style={{ display: 'flex', flexDirection: 'column', gap: '0.5rem', paddingLeft: '1.5rem' }}>
                      <div>
                        <label style={{ display: 'block', marginBottom: '0.25rem', fontSize: '0.75rem', color: 'var(--text-secondary)' }}>API Key (optional, falls back to global)</label>
                        <input
                          style={inputStyle}
                          type="password"
                          placeholder="Leave empty to use global config"
                          value={form.tools?.brave_search_settings?.api_key || ''}
                          onChange={(e) => setForm((prev) => ({
                            ...prev,
                            tools: {
                              ...prev.tools!,
                              brave_search_settings: {
                                ...prev.tools?.brave_search_settings,
                                api_key: e.target.value || undefined,
                              },
                            },
                          }))}
                        />
                      </div>
                      <label style={{ display: 'flex', alignItems: 'center', gap: '0.4rem', fontSize: '0.8rem', cursor: 'pointer' }}>
                        <input
                          type="checkbox"
                          checked={form.tools?.brave_search_settings?.safesearch ?? true}
                          onChange={(e) => setForm((prev) => ({
                            ...prev,
                            tools: {
                              ...prev.tools!,
                              brave_search_settings: {
                                ...prev.tools?.brave_search_settings,
                                safesearch: e.target.checked,
                              },
                            },
                          }))}
                        />
                        Safe Search
                      </label>
                    </div>
                  )}
                </div>
              </section>

              {/* Tool Timeout + MCP Servers */}
              <div style={{ display: 'flex', gap: '0.75rem' }}>
                <section style={{ ...fieldStyle, flex: 1 }}>
                  <label style={labelStyle}>
                    <TooltipLabel label="Tool Timeout" tooltip="Maximum time allowed for a single tool execution (e.g. 1m, 30s, 2m)." />
                  </label>
                  <input
                    style={inputStyle}
                    placeholder="1m"
                    value={form.tools?.timeout || ''}
                    onChange={(e) => updateToolField('timeout', e.target.value)}
                  />
                </section>
                {availableMcpServers.length > 0 && (
                  <section style={{ ...fieldStyle, flex: 1 }}>
                    <label style={labelStyle}>
                      <TooltipLabel label="MCP Servers" tooltip="Select globally configured MCP servers to attach to this agent." />
                    </label>
                    <div style={{ display: 'flex', flexDirection: 'column', gap: '0.4rem', padding: '0.25rem 0' }}>
                      {availableMcpServers.map((name) => (
                        <label key={name} style={{ display: 'flex', alignItems: 'center', gap: '0.4rem', fontSize: '0.8rem', cursor: 'pointer' }}>
                          <input
                            type="checkbox"
                            checked={form.tools?.mcp_servers?.includes(name) ?? false}
                            onChange={() => toggleMcpServer(name)}
                          />
                          {name}
                        </label>
                      ))}
                    </div>
                  </section>
                )}
              </div>

              <div style={{ display: 'flex', gap: '0.75rem' }}>
                <section style={{ ...fieldStyle, flex: 1 }}>
                  <label style={labelStyle}>
                    <TooltipLabel label="Prompt Timeout" tooltip="Maximum wall-clock duration for a single request, including all reasoning turns and tool executions (e.g. 5m, 30s)." />
                  </label>
                  <input
                    style={inputStyle}
                    placeholder="5m"
                    value={form.prompt_timeout || ''}
                    onChange={(e) => updateField('prompt_timeout', e.target.value)}
                  />
                </section>
                <section style={{ ...fieldStyle, flex: 1 }}>
                  <label style={labelStyle}>
                    <TooltipLabel label="Heartbeat Interval" tooltip="How often the agent's background task loop runs. The agent executes tasks written to HEARTBEAT.md on each tick (e.g. 30m, 1h)." />
                  </label>
                  <input
                    style={inputStyle}
                    placeholder="30m"
                    value={form.heartbeat_interval || ''}
                    onChange={(e) => updateField('heartbeat_interval', e.target.value)}
                  />
                </section>
                <section style={{ ...fieldStyle, flex: 1 }}>
                  <label style={labelStyle}>
                    <TooltipLabel label="Dream Interval" tooltip="How often the agent runs a self-reflection process that analyzes past conversations and updates its documents and memories (e.g. 24h)." />
                  </label>
                  <input
                    style={inputStyle}
                    placeholder="24h"
                    value={form.dream_interval || ''}
                    onChange={(e) => updateField('dream_interval', e.target.value)}
                  />
                </section>
              </div>

              {/* Channel Tokens */}
              <div style={{ display: 'flex', gap: '0.75rem' }}>
                <section style={{ ...fieldStyle, flex: 1 }}>
                  <label style={labelStyle}>
                    <TooltipLabel label="Discord Bot Token" tooltip="Bot token from the Discord Developer Portal. Required for the agent to connect to Discord channels." />
                  </label>
                  <input
                    style={inputStyle}
                    type="password"
                    placeholder="Optional"
                    value={form.discord_token || ''}
                    onChange={(e) => updateField('discord_token', e.target.value || undefined)}
                  />
                </section>
                <section style={{ ...fieldStyle, flex: 1 }}>
                  <label style={labelStyle}>
                    <TooltipLabel label="Telegram Bot Token" tooltip="Bot token from @BotFather. Required for the agent to connect to Telegram chats." />
                  </label>
                  <input
                    style={inputStyle}
                    type="password"
                    placeholder="Optional"
                    value={form.telegram_token || ''}
                    onChange={(e) => updateField('telegram_token', e.target.value || undefined)}
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
