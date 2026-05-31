import { useState, useEffect } from 'react'
import TooltipLabel from './TooltipLabel'
import MarkdownEditor from './MarkdownEditor'
import { listGlobalConfigs } from '../services/vizier'
import type { CreateAgentRequest, AgentDetail, GlobalConfigEntry } from '../interfaces/types'
import defaultPrompt from '../../../templates/agent.template.md?raw'

const PROVIDERS = ['ollama', 'deepseek', 'openrouter', 'anthropic', 'openai', 'gemini', 'mimo']

const DEFAULT_MODELS: Record<string, string> = {
  ollama: 'qwen3.5:4b',
  deepseek: 'deepseek-chat',
  openrouter: 'anthropic/claude-3-haiku',
  anthropic: 'claude-3-haiku-20240307',
  openai: 'gpt-4o-mini',
  gemini: 'gemini-2.0-flash',
  mimo: 'mimo-v2.5-pro',
}

const STEPS = [
  { num: 1, label: 'Essential' },
  { num: 2, label: 'Advanced' },
  { num: 3, label: 'System Prompt' },
]

interface AgentFormProps {
  mode: 'create' | 'edit'
  agentId?: string
  initialData?: AgentDetail
  onSubmit: (form: CreateAgentRequest) => Promise<void>
  onCancel: () => void
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

const DEFAULT_FORM: CreateAgentRequest = {
  agent_id: '',
  name: '',
  description: '',
  provider: 'ollama',
  model: 'qwen3.5:4b',
  system_prompt: defaultPrompt,
  thinking_depth: 10,
  session_memory_capacity: 10,
  max_tokens: 100000,
  show_thinking: false,
  show_tool_calls: false,
  silent_read_initiative_chance: 0.0,
  tools: {
    shell_access: false,
    brave_search: false,
    brave_search_settings: {},
    vector_memory: true,
    discord: false,
    telegram: false,
    fetch: false,
    http_client: false,
    programmatic_sandbox: false,
    timeout: '1m',
    mcp_servers: [],
  },
  prompt_timeout: '5m',
  heartbeat_interval: '30m',
  dream_interval: '24h',
}

export default function AgentForm({ mode, initialData, onSubmit, onCancel }: AgentFormProps) {
  const [step, setStep] = useState(1)
  const [submitting, setSubmitting] = useState(false)
  const [availableMcpServers, setAvailableMcpServers] = useState<string[]>([])
  const [form, setForm] = useState<CreateAgentRequest>(DEFAULT_FORM)

  useEffect(() => {
    if (mode === 'edit' && initialData) {
      const d = initialData
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
        show_thinking: d.show_thinking ?? false,
        show_tool_calls: d.show_tool_calls ?? false,
        silent_read_initiative_chance: d.silent_read_initiative_chance ?? 0.0,
        tools: {
          shell_access: d.shell_access,
          brave_search: d.brave_search,
          brave_search_settings: d.brave_search_settings || {},
          vector_memory: d.vector_memory,
          discord: d.discord,
          telegram: d.telegram,
          fetch: d.fetch,
          http_client: d.http_client,
          programmatic_sandbox: d.programmatic_sandbox ?? false,
          timeout: d.tools_timeout || '1m',
          mcp_servers: d.mcp_servers || [],
        },
        prompt_timeout: d.prompt_timeout,
        heartbeat_interval: d.heartbeat_interval,
        dream_interval: d.dream_interval,
        discord_token: d.discord_token || '',
        telegram_token: d.telegram_token || '',
      })
    }
  }, [mode, initialData])

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

  const handleSubmit = async () => {
    if (mode === 'create' && (!form.agent_id.trim() || !form.name.trim())) {
      return
    }
    if (mode === 'edit' && !form.name.trim()) {
      return
    }

    setSubmitting(true)
    try {
      await onSubmit(form)
    } finally {
      setSubmitting(false)
    }
  }

  const canNext = step === 1
    ? (mode === 'edit' || !!form.agent_id.trim()) && !!form.name.trim()
    : true

  return (
    <div style={{ display: 'flex', justifyContent: 'center' }}>
      <div style={{ width: '100%', maxWidth: '720px', display: 'flex', flexDirection: 'column', gap: '1.25rem' }}>
        {/* Step indicator */}
        <div style={{ display: 'flex', alignItems: 'center', justifyContent: 'center', gap: '0', padding: '0.5rem 0' }}>
          {STEPS.map((s, i) => {
            const isActive = step === s.num
            const isCompleted = step > s.num
            return (
              <div key={s.num} style={{ display: 'flex', alignItems: 'center' }}>
                {i > 0 && (
                  <div style={{
                    width: '48px',
                    height: '2px',
                    background: isCompleted ? 'var(--accent-primary)' : 'var(--border)',
                    margin: '0 0.25rem',
                  }} />
                )}
                <button
                  onClick={() => {
                    if (isCompleted || isActive) setStep(s.num)
                    else if (step + 1 === s.num && canNext) setStep(s.num)
                  }}
                  disabled={!isCompleted && !isActive && !(step + 1 === s.num && canNext)}
                  style={{
                    display: 'flex',
                    alignItems: 'center',
                    gap: '0.4rem',
                    background: 'none',
                    border: 'none',
                    cursor: isCompleted || isActive || (step + 1 === s.num && canNext) ? 'pointer' : 'default',
                    padding: '0.25rem 0.5rem',
                    borderRadius: '1rem',
                  }}
                >
                  <div style={{
                    width: '24px',
                    height: '24px',
                    borderRadius: '50%',
                    display: 'flex',
                    alignItems: 'center',
                    justifyContent: 'center',
                    fontSize: '0.7rem',
                    fontWeight: 600,
                    background: isActive ? 'var(--accent-primary)' : isCompleted ? 'var(--accent-primary)' : 'transparent',
                    color: isActive ? '#fff' : isCompleted ? '#fff' : 'var(--text-tertiary)',
                    border: isActive || isCompleted ? 'none' : '2px solid var(--border)',
                    transition: 'all 0.2s',
                  }}>
                    {isCompleted ? '\u2713' : s.num}
                  </div>
                  <span style={{
                    fontSize: '0.78rem',
                    fontWeight: isActive ? 600 : 400,
                    color: isActive ? 'var(--text-primary)' : isCompleted ? 'var(--accent-primary)' : 'var(--text-tertiary)',
                    transition: 'all 0.2s',
                  }}>
                    {s.label}
                  </span>
                </button>
              </div>
            )
          })}
        </div>

        {/* Step 1: Essential */}
        {step === 1 && (
          <>
            <section style={fieldStyle}>
              <label style={labelStyle}>
                <TooltipLabel label="Agent ID" tooltip="Unique identifier for the agent. Lowercase letters, numbers, hyphens, and underscores only. Cannot be changed after creation." />
                {mode === 'create' && ' *'}
              </label>
              <input
                style={{ ...inputStyle, opacity: mode === 'edit' ? 0.6 : 1 }}
                placeholder="my-agent"
                value={form.agent_id}
                disabled={mode === 'edit'}
                onChange={(e) => updateField('agent_id', e.target.value.toLowerCase().replace(/[^a-z0-9-_]/g, '-'))}
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
                    if (mode === 'create') updateField('model', DEFAULT_MODELS[p] || '')
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
          </>
        )}

        {/* Step 2: Advanced */}
        {step === 2 && (
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
                  <TooltipLabel label="Show Thinking" tooltip="Display the agent's reasoning/thinking process in chat responses." />
                </label>
                <label style={{ display: 'flex', alignItems: 'center', gap: '0.4rem', fontSize: '0.8rem', cursor: 'pointer' }}>
                  <input
                    type="checkbox"
                    checked={form.show_thinking ?? false}
                    onChange={(e) => updateField('show_thinking', e.target.checked)}
                  />
                  Show thinking output
                </label>
              </section>
              <section style={{ ...fieldStyle, flex: 1 }}>
                <label style={labelStyle}>
                  <TooltipLabel label="Show Tool Calls" tooltip="Display tool call details (name, arguments, results) in chat responses." />
                </label>
                <label style={{ display: 'flex', alignItems: 'center', gap: '0.4rem', fontSize: '0.8rem', cursor: 'pointer' }}>
                  <input
                    type="checkbox"
                    checked={form.show_tool_calls ?? false}
                    onChange={(e) => updateField('show_tool_calls', e.target.checked)}
                  />
                  Show tool call details
                </label>
              </section>
            </div>

            <div style={{ display: 'flex', gap: '0.75rem' }}>
              <section style={{ ...fieldStyle, flex: 1 }}>
                <label style={labelStyle}>
                  <TooltipLabel label="Silent Read Chance" tooltip="Probability (0.0 to 1.0) that the agent proactively reads silent/channel messages. 0.0 = never, 1.0 = always." />
                </label>
                <div style={{ display: 'flex', alignItems: 'center', gap: '0.75rem' }}>
                  <input
                    style={{ ...inputStyle, flex: 1 }}
                    type="range"
                    min={0}
                    max={1}
                    step={0.05}
                    value={form.silent_read_initiative_chance ?? 0.0}
                    onChange={(e) => updateField('silent_read_initiative_chance', parseFloat(e.target.value))}
                  />
                  <span style={{ fontSize: '0.8rem', color: 'var(--text-secondary)', minWidth: '2.5rem', textAlign: 'right' }}>
                    {(form.silent_read_initiative_chance ?? 0.0).toFixed(2)}
                  </span>
                </div>
              </section>
              <section style={{ ...fieldStyle, flex: 1 }}>
                <label style={labelStyle}>
                  <TooltipLabel label="Programmatic Sandbox" tooltip="Enable sandboxed execution for programmatic tools (e.g. code execution in a container)." />
                </label>
                <label style={{ display: 'flex', alignItems: 'center', gap: '0.4rem', fontSize: '0.8rem', cursor: 'pointer' }}>
                  <input
                    type="checkbox"
                    checked={form.tools?.programmatic_sandbox ?? false}
                    onChange={(e) => updateTool('programmatic_sandbox', e.target.checked)}
                  />
                  Enable sandboxed execution
                </label>
              </section>
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

        {/* Step 3: System Prompt */}
        {step === 3 && (
          <section style={fieldStyle}>
            <label style={labelStyle}>
              <TooltipLabel label="System Prompt" tooltip="Instructions that define the agent's behavior, personality, and capabilities. Supports full Markdown formatting." />
            </label>
            <div style={{ border: '1px solid var(--border)', borderRadius: '0.5rem', overflow: 'hidden' }}>
              <MarkdownEditor
                value={form.system_prompt || ''}
                onChange={(v) => updateField('system_prompt', v)}
                placeholder="You are a helpful assistant..."
              />
            </div>
          </section>
        )}

        {/* Navigation */}
        <div style={{ display: 'flex', gap: '0.75rem', paddingTop: '0.5rem' }}>
          <button
            onClick={onCancel}
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
          <div style={{ flex: 1 }} />
          {step > 1 && (
            <button
              onClick={() => setStep(step - 1)}
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
              Back
            </button>
          )}
          {step < 3 ? (
            <button
              onClick={() => setStep(step + 1)}
              disabled={!canNext}
              style={{
                padding: '0.6rem 1.5rem',
                borderRadius: '0.375rem',
                border: 'none',
                background: canNext ? 'var(--accent-primary)' : 'var(--border)',
                color: '#fff',
                cursor: canNext ? 'pointer' : 'not-allowed',
                fontSize: '0.85rem',
                fontWeight: 500,
              }}
            >
              Next
            </button>
          ) : (
            <button
              onClick={handleSubmit}
              disabled={submitting}
              style={{
                padding: '0.6rem 1.5rem',
                borderRadius: '0.375rem',
                border: 'none',
                background: submitting ? 'var(--border)' : 'var(--accent-primary)',
                color: '#fff',
                cursor: submitting ? 'not-allowed' : 'pointer',
                fontSize: '0.85rem',
                fontWeight: 500,
              }}
            >
              {submitting
                ? (mode === 'create' ? 'Creating...' : 'Saving...')
                : (mode === 'create' ? 'Create Agent' : 'Save Changes')
              }
            </button>
          )}
        </div>
      </div>
    </div>
  )
}
