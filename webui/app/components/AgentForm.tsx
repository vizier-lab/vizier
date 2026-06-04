import { useState, useEffect, useRef } from 'react'
import { FaGear, FaCode } from 'react-icons/fa6'
import TooltipLabel from './TooltipLabel'
import MarkdownEditor from './MarkdownEditor'
import Avatar from './avatar'
import AvatarCropModal from './AvatarCropModal'
import { getMcpServers, uploadFile } from '../services/vizier'
import type {
  CreateAgentRequest,
  AgentDetail,
} from '../interfaces/types'
import defaultPrompt from '../../../templates/agent.template.md?raw'

type FormTab = 'config' | 'prompt'

const TABS: { key: FormTab; label: string; icon: typeof FaGear }[] = [
  { key: 'config', label: 'Config', icon: FaGear },
  { key: 'prompt', label: 'System Prompt', icon: FaCode },
]

const PROVIDERS = [
  'ollama',
  'deepseek',
  'openrouter',
  'anthropic',
  'openai',
  'gemini',
  'mimo',
  'llama_cpp',
]

const DEFAULT_MODELS: Record<string, string> = {
  ollama: 'qwen3.5:4b',
  deepseek: 'deepseek-chat',
  openrouter: 'anthropic/claude-3-haiku',
  anthropic: 'claude-3-haiku-20240307',
  openai: 'gpt-4o-mini',
  gemini: 'gemini-2.0-flash',
  mimo: 'mimo-v2.5-pro',
  llama_cpp: 'google_gemma-4-E4B-it-Q4_K_M',
}

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
  avatar_url: undefined,
}

export default function AgentForm({
  mode,
  initialData,
  onSubmit,
  onCancel,
}: AgentFormProps) {
  const [activeTab, setActiveTab] = useState<FormTab>('config')
  const [submitting, setSubmitting] = useState(false)
  const [availableMcpServers, setAvailableMcpServers] = useState<string[]>([])
  const [form, setForm] = useState<CreateAgentRequest>(DEFAULT_FORM)
  const [cropFile, setCropFile] = useState<File | null>(null)
  const [avatarBlob, setAvatarBlob] = useState<Blob | null>(null)
  const [avatarPreview, setAvatarPreview] = useState<string | null>(null)
  const avatarInputRef = useRef<HTMLInputElement>(null)

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
        silent_read_initiative_chance:
          d.silent_read_initiative_chance ?? 0.0,
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
        avatar_url: d.avatar_url,
      })
    }
  }, [mode, initialData])

  useEffect(() => {
    const loadMcpServers = async () => {
      try {
        const response = await getMcpServers()
        if (response.data && response.data.value.type === 'McpServers') {
          setAvailableMcpServers(Object.keys(response.data.value.data))
        }
      } catch {
        // silently ignore
      }
    }
    loadMcpServers()
  }, [])

  const updateField = <K extends keyof CreateAgentRequest>(
    key: K,
    value: CreateAgentRequest[K]
  ) => {
    setForm((prev) => ({ ...prev, [key]: value }))
  }

  const updateTool = (
    key: keyof NonNullable<CreateAgentRequest['tools']>,
    value: boolean
  ) => {
    setForm((prev) => ({ ...prev, tools: { ...prev.tools, [key]: value } }))
  }

  const updateToolField = (key: string, value: string | string[]) => {
    setForm((prev) => ({ ...prev, tools: { ...prev.tools, [key]: value } }))
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

  const handleAvatarSelect = (e: React.ChangeEvent<HTMLInputElement>) => {
    const file = e.target.files?.[0]
    if (file) setCropFile(file)
    if (avatarInputRef.current) avatarInputRef.current.value = ''
  }

  const handleAvatarCropped = (blob: Blob) => {
    setCropFile(null)
    setAvatarBlob(blob)
    setAvatarPreview(URL.createObjectURL(blob))
  }

  const handleRemoveAvatar = () => {
    setAvatarBlob(null)
    if (avatarPreview) URL.revokeObjectURL(avatarPreview)
    setAvatarPreview(null)
    setForm((prev) => ({ ...prev, avatar_url: undefined }))
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
      // Upload avatar if a new one was selected
      let avatarUrl = form.avatar_url
      if (avatarBlob) {
        const file = new File([avatarBlob], 'avatar.png', {
          type: 'image/png',
        })
        const res = await uploadFile(file)
        avatarUrl = res.url
      }

      await onSubmit({ ...form, avatar_url: avatarUrl })
    } finally {
      setSubmitting(false)
    }
  }

  const avatarDisplayUrl = avatarPreview || form.avatar_url

  return (
    <>
      {/* Mobile tab nav */}
      <div className="flex md:hidden border-b border-[var(--border)] px-4 gap-2 py-2 overflow-x-auto">
        {TABS.map(({ key, label }) => (
          <button
            key={key}
            onClick={() => setActiveTab(key)}
            className={`px-3 py-1.5 text-sm font-medium rounded-t transition-colors whitespace-nowrap ${activeTab === key ? 'bg-[var(--surface)] text-[var(--text-primary)] border-b-2 border-[var(--accent-primary)]' : 'text-[var(--text-tertiary)]'}`}
          >
            {label}
          </button>
        ))}
      </div>

      <div className="flex" style={{
        flex: 1,
        height: '100%',
      }}>
        {/* Desktop sidebar nav */}
        <div
          className="hidden md:block"
          style={{
            width: '200px',
            borderRight: '1px solid var(--border)',
            padding: '24px 16px',
            flexShrink: 0,
          }}
        >
          {TABS.map(({ key, label, icon: Icon }) => (
            <div
              key={key}
              className={`nav-item ${activeTab === key ? 'active' : ''}`}
              onClick={() => setActiveTab(key)}
            >
              <Icon size={16} />
              <span>{label}</span>
            </div>
          ))}
        </div>

        {/* Content */}
        <div
          className="flex-1 overflow-auto"
          style={{ padding: '24px', height: '100%' }}
        >
          {/* ─── Config Tab ─── */}
          {activeTab === 'config' && (
            <div
              style={{
                maxWidth: '720px',
                display: 'flex',
                flexDirection: 'column',
                gap: '1.5rem',
              }}
            >
              {/* Avatar */}
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
                  Avatar
                </h4>
                <div
                  style={{
                    display: 'flex',
                    alignItems: 'center',
                    gap: '1rem',
                  }}
                >
                  <Avatar
                    name={form.agent_id || 'new'}
                    size="lg"
                    avatarUrl={avatarDisplayUrl}
                  />
                  <div
                    style={{
                      display: 'flex',
                      flexDirection: 'column',
                      gap: '0.5rem',
                    }}
                  >
                    <div
                      style={{
                        display: 'flex',
                        gap: '0.5rem',
                      }}
                    >
                      <button
                        type="button"
                        className="btn btn-secondary"
                        style={{
                          fontSize: '0.8rem',
                          padding: '6px 12px',
                        }}
                        onClick={() =>
                          avatarInputRef.current?.click()
                        }
                      >
                        Choose Image
                      </button>
                      {avatarDisplayUrl && (
                        <button
                          type="button"
                          className="btn btn-ghost"
                          style={{
                            fontSize: '0.8rem',
                            padding: '6px 12px',
                            color: '#ef4444',
                          }}
                          onClick={handleRemoveAvatar}
                        >
                          Remove
                        </button>
                      )}
                    </div>
                    <span
                      style={{
                        fontSize: '0.75rem',
                        color: 'var(--text-tertiary)',
                      }}
                    >
                      Leave empty for generated avatar
                    </span>
                  </div>
                </div>
                <input
                  ref={avatarInputRef}
                  type="file"
                  accept="image/*"
                  onChange={handleAvatarSelect}
                  style={{ display: 'none' }}
                />
                <AvatarCropModal
                  file={cropFile}
                  onClose={() => setCropFile(null)}
                  onCropped={handleAvatarCropped}
                />
              </div>

              {/* Basic Info */}
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
                  Basic Info
                </h4>
                <div
                  style={{
                    display: 'flex',
                    flexDirection: 'column',
                    gap: '1rem',
                  }}
                >
                  <section style={fieldStyle}>
                    <label style={labelStyle}>
                      <TooltipLabel
                        label="Agent ID"
                        tooltip="Unique identifier. Lowercase letters, numbers, hyphens, and underscores only. Cannot be changed after creation."
                      />
                      {mode === 'create' && ' *'}
                    </label>
                    <input
                      style={{
                        ...inputStyle,
                        opacity:
                          mode === 'edit' ? 0.6 : 1,
                      }}
                      placeholder="my-agent"
                      value={form.agent_id}
                      disabled={mode === 'edit'}
                      onChange={(e) =>
                        updateField(
                          'agent_id',
                          e.target.value
                            .toLowerCase()
                            .replace(
                              /[^a-z0-9-_]/g,
                              '-'
                            )
                        )
                      }
                    />
                  </section>
                  <section style={fieldStyle}>
                    <label style={labelStyle}>
                      <TooltipLabel
                        label="Name"
                        tooltip="Display name for this agent."
                      />
                      {' *'}
                    </label>
                    <input
                      style={inputStyle}
                      placeholder="My Agent"
                      value={form.name}
                      onChange={(e) =>
                        updateField(
                          'name',
                          e.target.value
                        )
                      }
                    />
                  </section>
                  <section style={fieldStyle}>
                    <label style={labelStyle}>
                      <TooltipLabel
                        label="Description"
                        tooltip="Optional description of what this agent does."
                      />
                    </label>
                    <input
                      style={inputStyle}
                      placeholder="A helpful assistant"
                      value={form.description || ''}
                      onChange={(e) =>
                        updateField(
                          'description',
                          e.target.value
                        )
                      }
                    />
                  </section>
                  <div
                    style={{
                      display: 'flex',
                      gap: '0.75rem',
                    }}
                  >
                    <section
                      style={{ ...fieldStyle, flex: 1 }}
                    >
                      <label style={labelStyle}>
                        <TooltipLabel
                          label="Provider"
                          tooltip="The AI provider to use for completions."
                        />
                      </label>
                      <select
                        style={inputStyle}
                        value={form.provider}
                        onChange={(e) => {
                          updateField(
                            'provider',
                            e.target.value
                          )
                          if (mode === 'create')
                            updateField(
                              'model',
                              DEFAULT_MODELS[
                              e.target.value
                              ] || ''
                            )
                        }}
                      >
                        {PROVIDERS.map((p) => (
                          <option key={p} value={p}>
                            {p}
                          </option>
                        ))}
                      </select>
                    </section>
                    <section
                      style={{ ...fieldStyle, flex: 1 }}
                    >
                      <label style={labelStyle}>
                        <TooltipLabel
                          label="Model"
                          tooltip="The model identifier for the selected provider."
                        />
                      </label>
                      <input
                        style={inputStyle}
                        value={form.model}
                        onChange={(e) =>
                          updateField(
                            'model',
                            e.target.value
                          )
                        }
                      />
                    </section>
                  </div>
                </div>
              </div>

              {/* Model Parameters */}
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
                  Model Parameters
                </h4>
                <div
                  style={{
                    display: 'flex',
                    flexDirection: 'column',
                    gap: '1rem',
                  }}
                >
                  <div
                    style={{
                      display: 'flex',
                      gap: '0.75rem',
                    }}
                  >
                    <section
                      style={{ ...fieldStyle, flex: 1 }}
                    >
                      <label style={labelStyle}>
                        <TooltipLabel
                          label="Thinking Depth"
                          tooltip="Maximum LLM reasoning turns per request. Set to 0 for unlimited."
                        />
                      </label>
                      <input
                        style={inputStyle}
                        type="number"
                        min={1}
                        value={
                          form.thinking_depth || 10
                        }
                        onChange={(e) =>
                          updateField(
                            'thinking_depth',
                            parseInt(
                              e.target.value
                            ) || 10
                          )
                        }
                      />
                    </section>
                    <section
                      style={{ ...fieldStyle, flex: 1 }}
                    >
                      <label style={labelStyle}>
                        <TooltipLabel
                          label="Max Tokens"
                          tooltip="Maximum output tokens per LLM completion request."
                        />
                      </label>
                      <input
                        style={inputStyle}
                        type="number"
                        min={1}
                        placeholder="No limit"
                        value={form.max_tokens ?? ''}
                        onChange={(e) =>
                          updateField(
                            'max_tokens',
                            e.target.value
                              ? parseInt(
                                e.target.value
                              )
                              : undefined
                          )
                        }
                      />
                    </section>
                    <section
                      style={{ ...fieldStyle, flex: 1 }}
                    >
                      <label style={labelStyle}>
                        <TooltipLabel
                          label="Memory Capacity"
                          tooltip="Maximum recent conversation messages loaded as context."
                        />
                      </label>
                      <input
                        style={inputStyle}
                        type="number"
                        min={1}
                        value={
                          form.session_memory_capacity ||
                          10
                        }
                        onChange={(e) =>
                          updateField(
                            'session_memory_capacity',
                            parseInt(
                              e.target.value
                            ) || 10
                          )
                        }
                      />
                    </section>
                  </div>
                  <div
                    style={{
                      display: 'flex',
                      gap: '0.75rem',
                    }}
                  >
                    <section
                      style={{ ...fieldStyle, flex: 1 }}
                    >
                      <label style={labelStyle}>
                        <TooltipLabel
                          label="Show Thinking"
                          tooltip="Display the agent's reasoning/thinking process in chat."
                        />
                      </label>
                      <label
                        style={{
                          display: 'flex',
                          alignItems: 'center',
                          gap: '0.4rem',
                          fontSize: '0.8rem',
                          cursor: 'pointer',
                        }}
                      >
                        <input
                          type="checkbox"
                          checked={
                            form.show_thinking ??
                            false
                          }
                          onChange={(e) =>
                            updateField(
                              'show_thinking',
                              e.target.checked
                            )
                          }
                        />
                        Show thinking output
                      </label>
                    </section>
                    <section
                      style={{ ...fieldStyle, flex: 1 }}
                    >
                      <label style={labelStyle}>
                        <TooltipLabel
                          label="Show Tool Calls"
                          tooltip="Display tool call details in chat responses."
                        />
                      </label>
                      <label
                        style={{
                          display: 'flex',
                          alignItems: 'center',
                          gap: '0.4rem',
                          fontSize: '0.8rem',
                          cursor: 'pointer',
                        }}
                      >
                        <input
                          type="checkbox"
                          checked={
                            form.show_tool_calls ??
                            false
                          }
                          onChange={(e) =>
                            updateField(
                              'show_tool_calls',
                              e.target.checked
                            )
                          }
                        />
                        Show tool call details
                      </label>
                    </section>
                  </div>
                  <section style={fieldStyle}>
                    <label style={labelStyle}>
                      <TooltipLabel
                        label="Silent Read Chance"
                        tooltip="Probability (0.0-1.0) that the agent proactively reads silent/channel messages."
                      />
                    </label>
                    <div
                      style={{
                        display: 'flex',
                        alignItems: 'center',
                        gap: '0.75rem',
                      }}
                    >
                      <input
                        style={{
                          ...inputStyle,
                          flex: 1,
                        }}
                        type="range"
                        min={0}
                        max={1}
                        step={0.05}
                        value={
                          form.silent_read_initiative_chance ??
                          0.0
                        }
                        onChange={(e) =>
                          updateField(
                            'silent_read_initiative_chance',
                            parseFloat(
                              e.target.value
                            )
                          )
                        }
                      />
                      <span
                        style={{
                          fontSize: '0.8rem',
                          color: 'var(--text-secondary)',
                          minWidth: '2.5rem',
                          textAlign: 'right',
                        }}
                      >
                        {(
                          form.silent_read_initiative_chance ??
                          0.0
                        ).toFixed(2)}
                      </span>
                    </div>
                  </section>
                </div>
              </div>

              {/* Tools */}
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
                  Tools
                </h4>
                <div
                  style={{
                    display: 'flex',
                    flexDirection: 'column',
                    gap: '1rem',
                  }}
                >
                  <section style={fieldStyle}>
                    <div
                      style={{
                        display: 'grid',
                        gridTemplateColumns: '1fr 1fr',
                        gap: '0.5rem',
                      }}
                    >
                      {(
                        [
                          [
                            'shell_access',
                            'Shell Access',
                          ],
                          [
                            'vector_memory',
                            'Vector Memory',
                          ],
                          ['discord', 'Discord'],
                          ['telegram', 'Telegram'],
                          ['fetch', 'Fetch Webpage'],
                          [
                            'http_client',
                            'HTTP Client',
                          ],
                        ] as const
                      ).map(([key, label]) => (
                        <label
                          key={key}
                          style={{
                            display: 'flex',
                            alignItems: 'center',
                            gap: '0.4rem',
                            fontSize: '0.8rem',
                            cursor: 'pointer',
                          }}
                        >
                          <input
                            type="checkbox"
                            checked={
                              form.tools?.[key] ??
                              false
                            }
                            onChange={(e) =>
                              updateTool(
                                key,
                                e.target.checked
                              )
                            }
                          />
                          {label}
                        </label>
                      ))}
                    </div>
                  </section>
                  <div
                    style={{
                      padding: '0.75rem',
                      border: '1px solid var(--border)',
                      borderRadius: '0.5rem',
                    }}
                  >
                    <label
                      style={{
                        display: 'flex',
                        alignItems: 'center',
                        gap: '0.4rem',
                        fontSize: '0.8rem',
                        cursor: 'pointer',
                        marginBottom: form.tools
                          ?.brave_search
                          ? '0.75rem'
                          : 0,
                      }}
                    >
                      <input
                        type="checkbox"
                        checked={
                          form.tools?.brave_search ??
                          false
                        }
                        onChange={(e) =>
                          updateTool(
                            'brave_search',
                            e.target.checked
                          )
                        }
                      />
                      Brave Search
                    </label>
                    {form.tools?.brave_search && (
                      <div
                        style={{
                          display: 'flex',
                          flexDirection: 'column',
                          gap: '0.5rem',
                          paddingLeft: '1.5rem',
                        }}
                      >
                        <div>
                          <label
                            style={{
                              display: 'block',
                              marginBottom:
                                '0.25rem',
                              fontSize: '0.75rem',
                              color: 'var(--text-secondary)',
                            }}
                          >
                            API Key (optional, falls
                            back to global)
                          </label>
                          <input
                            style={inputStyle}
                            type="password"
                            placeholder="Leave empty to use global config"
                            value={
                              form.tools
                                ?.brave_search_settings
                                ?.api_key || ''
                            }
                            onChange={(e) =>
                              setForm((prev) => ({
                                ...prev,
                                tools: {
                                  ...prev.tools!,
                                  brave_search_settings:
                                  {
                                    ...prev
                                      .tools
                                      ?.brave_search_settings,
                                    api_key:
                                      e
                                        .target
                                        .value ||
                                      undefined,
                                  },
                                },
                              }))
                            }
                          />
                        </div>
                        <label
                          style={{
                            display: 'flex',
                            alignItems: 'center',
                            gap: '0.4rem',
                            fontSize: '0.8rem',
                            cursor: 'pointer',
                          }}
                        >
                          <input
                            type="checkbox"
                            checked={
                              form.tools
                                ?.brave_search_settings
                                ?.safesearch ??
                              true
                            }
                            onChange={(e) =>
                              setForm((prev) => ({
                                ...prev,
                                tools: {
                                  ...prev.tools!,
                                  brave_search_settings:
                                  {
                                    ...prev
                                      .tools
                                      ?.brave_search_settings,
                                    safesearch:
                                      e
                                        .target
                                        .checked,
                                  },
                                },
                              }))
                            }
                          />
                          Safe Search
                        </label>
                      </div>
                    )}
                  </div>
                  <section style={fieldStyle}>
                    <label style={labelStyle}>
                      <TooltipLabel
                        label="Programmatic Sandbox"
                        tooltip="Enable sandboxed execution for programmatic tools."
                      />
                    </label>
                    <label
                      style={{
                        display: 'flex',
                        alignItems: 'center',
                        gap: '0.4rem',
                        fontSize: '0.8rem',
                        cursor: 'pointer',
                      }}
                    >
                      <input
                        type="checkbox"
                        checked={
                          form.tools
                            ?.programmatic_sandbox ??
                          false
                        }
                        onChange={(e) =>
                          updateTool(
                            'programmatic_sandbox',
                            e.target.checked
                          )
                        }
                      />
                      Enable sandboxed execution
                    </label>
                  </section>
                  <div
                    style={{
                      display: 'flex',
                      gap: '0.75rem',
                    }}
                  >
                    <section
                      style={{ ...fieldStyle, flex: 1 }}
                    >
                      <label style={labelStyle}>
                        <TooltipLabel
                          label="Tool Timeout"
                          tooltip="Maximum time for a single tool execution (e.g. 1m, 30s)."
                        />
                      </label>
                      <input
                        style={inputStyle}
                        placeholder="1m"
                        value={
                          form.tools?.timeout || ''
                        }
                        onChange={(e) =>
                          updateToolField(
                            'timeout',
                            e.target.value
                          )
                        }
                      />
                    </section>
                    {availableMcpServers.length > 0 && (
                      <section
                        style={{
                          ...fieldStyle,
                          flex: 1,
                        }}
                      >
                        <label style={labelStyle}>
                          <TooltipLabel
                            label="MCP Servers"
                            tooltip="Select globally configured MCP servers to attach."
                          />
                        </label>
                        <div
                          style={{
                            display: 'flex',
                            flexDirection: 'column',
                            gap: '0.4rem',
                            padding: '0.25rem 0',
                          }}
                        >
                          {availableMcpServers.map(
                            (name) => (
                              <label
                                key={name}
                                style={{
                                  display:
                                    'flex',
                                  alignItems:
                                    'center',
                                  gap: '0.4rem',
                                  fontSize:
                                    '0.8rem',
                                  cursor: 'pointer',
                                }}
                              >
                                <input
                                  type="checkbox"
                                  checked={
                                    form.tools?.mcp_servers?.includes(
                                      name
                                    ) ??
                                    false
                                  }
                                  onChange={() =>
                                    toggleMcpServer(
                                      name
                                    )
                                  }
                                />
                                {name}
                              </label>
                            )
                          )}
                        </div>
                      </section>
                    )}
                  </div>
                </div>
              </div>

              {/* Timing */}
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
                  Timing
                </h4>
                <div
                  style={{ display: 'flex', gap: '0.75rem' }}
                >
                  <section style={{ ...fieldStyle, flex: 1 }}>
                    <label style={labelStyle}>
                      <TooltipLabel
                        label="Prompt Timeout"
                        tooltip="Maximum wall-clock duration for a single request."
                      />
                    </label>
                    <input
                      style={inputStyle}
                      placeholder="5m"
                      value={form.prompt_timeout || ''}
                      onChange={(e) =>
                        updateField(
                          'prompt_timeout',
                          e.target.value
                        )
                      }
                    />
                  </section>
                  <section style={{ ...fieldStyle, flex: 1 }}>
                    <label style={labelStyle}>
                      <TooltipLabel
                        label="Heartbeat Interval"
                        tooltip="How often the agent's background task loop runs."
                      />
                    </label>
                    <input
                      style={inputStyle}
                      placeholder="30m"
                      value={
                        form.heartbeat_interval || ''
                      }
                      onChange={(e) =>
                        updateField(
                          'heartbeat_interval',
                          e.target.value
                        )
                      }
                    />
                  </section>
                  <section style={{ ...fieldStyle, flex: 1 }}>
                    <label style={labelStyle}>
                      <TooltipLabel
                        label="Dream Interval"
                        tooltip="How often the agent runs self-reflection."
                      />
                    </label>
                    <input
                      style={inputStyle}
                      placeholder="24h"
                      value={form.dream_interval || ''}
                      onChange={(e) =>
                        updateField(
                          'dream_interval',
                          e.target.value
                        )
                      }
                    />
                  </section>
                </div>
              </div>

              {/* Channel Tokens */}
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
                  Channel Tokens
                </h4>
                <div
                  style={{ display: 'flex', gap: '0.75rem' }}
                >
                  <section style={{ ...fieldStyle, flex: 1 }}>
                    <label style={labelStyle}>
                      <TooltipLabel
                        label="Discord Bot Token"
                        tooltip="Bot token from the Discord Developer Portal."
                      />
                    </label>
                    <input
                      style={inputStyle}
                      type="password"
                      placeholder="Optional"
                      value={form.discord_token || ''}
                      onChange={(e) =>
                        updateField(
                          'discord_token',
                          e.target.value || undefined
                        )
                      }
                    />
                  </section>
                  <section style={{ ...fieldStyle, flex: 1 }}>
                    <label style={labelStyle}>
                      <TooltipLabel
                        label="Telegram Bot Token"
                        tooltip="Bot token from @BotFather."
                      />
                    </label>
                    <input
                      style={inputStyle}
                      type="password"
                      placeholder="Optional"
                      value={form.telegram_token || ''}
                      onChange={(e) =>
                        updateField(
                          'telegram_token',
                          e.target.value || undefined
                        )
                      }
                    />
                  </section>
                </div>
              </div>

              {/* Actions */}
              <div
                style={{
                  display: 'flex',
                  gap: '0.75rem',
                  paddingTop: '0.5rem',
                }}
              >
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
                <button
                  onClick={handleSubmit}
                  disabled={
                    submitting ||
                    (mode === 'create' &&
                      (!form.agent_id.trim() ||
                        !form.name.trim())) ||
                    (mode === 'edit' && !form.name.trim())
                  }
                  style={{
                    padding: '0.6rem 1.5rem',
                    borderRadius: '0.375rem',
                    border: 'none',
                    background: submitting
                      ? 'var(--border)'
                      : 'var(--accent-primary)',
                    color: '#fff',
                    cursor: submitting
                      ? 'not-allowed'
                      : 'pointer',
                    fontSize: '0.85rem',
                    fontWeight: 500,
                  }}
                >
                  {submitting
                    ? mode === 'create'
                      ? 'Creating...'
                      : 'Saving...'
                    : mode === 'create'
                      ? 'Create Agent'
                      : 'Save Changes'}
                </button>
              </div>
            </div>
          )}

          {/* ─── System Prompt Tab ─── */}
          {activeTab === 'prompt' && (
            <div
              style={{
                maxWidth: '900px',
                display: 'flex',
                flexDirection: 'column',
                gap: '1rem',
                height: '100%',
              }}
            >
              <p
                style={{
                  color: 'var(--text-secondary)',
                  fontSize: '14px',
                  marginBottom: '0.5rem',
                }}
              >
                Instructions that define the agent's behavior,
                personality, and capabilities. Supports full
                Markdown formatting.
              </p>
              <div
                style={{
                  border: '1px solid var(--border)',
                  borderRadius: '0.5rem',
                  overflow: 'hidden',
                  height: '100%',
                }}
              >
                <MarkdownEditor
                  value={form.system_prompt || ''}
                  onChange={(v) =>
                    updateField('system_prompt', v)
                  }
                  placeholder="You are a helpful assistant..."
                />
              </div>
              <div style={{ display: 'flex', gap: '0.75rem' }}>
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
                <button
                  onClick={handleSubmit}
                  disabled={
                    submitting ||
                    (mode === 'create' &&
                      (!form.agent_id.trim() ||
                        !form.name.trim())) ||
                    (mode === 'edit' && !form.name.trim())
                  }
                  style={{
                    padding: '0.6rem 1.5rem',
                    borderRadius: '0.375rem',
                    border: 'none',
                    background: submitting
                      ? 'var(--border)'
                      : 'var(--accent-primary)',
                    color: '#fff',
                    cursor: submitting
                      ? 'not-allowed'
                      : 'pointer',
                    fontSize: '0.85rem',
                    fontWeight: 500,
                  }}
                >
                  {submitting
                    ? mode === 'create'
                      ? 'Creating...'
                      : 'Saving...'
                    : mode === 'create'
                      ? 'Create Agent'
                      : 'Save Changes'}
                </button>
              </div>
            </div>
          )}
        </div>
      </div>
    </>
  )
}
