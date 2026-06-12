import { useState, useEffect, useRef } from 'react'
import { FaGear, FaCode, FaScrewdriverWrench, FaCircleCheck } from 'react-icons/fa6'
import TooltipLabel from './TooltipLabel'
import MarkdownEditor from './MarkdownEditor'
import Avatar from './avatar'
import AvatarCropModal from './AvatarCropModal'
import { uploadFile } from '../services/vizier'
import type {
  CreateAgentRequest,
  AgentDetail,
  McpServerConfig,
  ShellConfigData,
} from '../interfaces/types'
import defaultPrompt from '../../../templates/agent.template.md?raw'

type FormTab = 'config' | 'tools' | 'prompt' | 'review'

const TABS: { key: FormTab; label: string; icon: typeof FaGear }[] = [
  { key: 'config', label: 'Config', icon: FaGear },
  { key: 'tools', label: 'Tools', icon: FaScrewdriverWrench },
  { key: 'prompt', label: 'System Prompt', icon: FaCode },
  { key: 'review', label: 'Review', icon: FaCircleCheck },
]

const PROVIDERS = [
  'mistralrs',
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
  mistralrs: 'google/gemma-4-E4B-it',
  ollama: 'qwen3.5:4b',
  deepseek: 'deepseek-chat',
  openrouter: 'anthropic/claude-3-haiku',
  anthropic: 'claude-3-haiku-20240307',
  openai: 'gpt-4o-mini',
  gemini: 'gemini-2.0-flash',
  mimo: 'mimo-v2.5-pro',
  llama_cpp: 'google_gemma-4-E4B-it-Q4_K_M',
}

const QUANTIZATION_OPTIONS = [
  { value: 'auto_4', label: 'Auto 4-bit (Recommended)' },
  { value: 'auto_8', label: 'Auto 8-bit' },
  { value: 'q4_0', label: 'Q4_0' },
  { value: 'q4_1', label: 'Q4_1' },
  { value: 'q4k', label: 'Q4K' },
  { value: 'q5_0', label: 'Q5_0' },
  { value: 'q5_1', label: 'Q5_1' },
  { value: 'q5k', label: 'Q5K' },
  { value: 'q6k', label: 'Q6K' },
  { value: 'q8_0', label: 'Q8_0' },
  { value: 'q8_1', label: 'Q8_1' },
  { value: 'hqq4', label: 'HQQ4' },
  { value: 'hqq8', label: 'HQQ8' },
  { value: 'fp8', label: 'FP8' },
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
  provider: 'mistralrs',
  model: 'google/gemma-4-E4B-it',
  quantization: 'auto_4',
  system_prompt: defaultPrompt,
  thinking_depth: 10,
  session_memory_capacity: 10,
  max_tokens: 100000,
  show_thinking: false,
  show_tool_calls: false,
  silent_read_initiative_chance: 0.0,
  tools: {
    shell: null,
    brave_search: false,
    brave_search_settings: {},
    discord: false,
    telegram: false,
    fetch: false,
    http_client: false,
    programmatic_sandbox: false,
    timeout: '1m',
    mcp_servers: {},
    tts: false,
    tts_settings: {},
    stt: false,
    stt_settings: {},
    read_image: false,
    read_image_settings: {},
    image_gen: false,
    image_gen_settings: {},
  },
  prompt_timeout: '5m',
  heartbeat_interval: '30m',
  dream_enabled: false,
  dream_schedule: '',
  dream_provider: '',
  dream_model: '',
  avatar_url: undefined,
}

export default function AgentForm({
  mode,
  initialData,
  onSubmit,
  onCancel,
}: AgentFormProps) {
  const [activeTab, setActiveTab] = useState<FormTab>('config')
  const [maxReachedTab, setMaxReachedTab] = useState<number>(0)
  const [submitting, setSubmitting] = useState(false)
  const [form, setForm] = useState<CreateAgentRequest>(DEFAULT_FORM)
  const [cropFile, setCropFile] = useState<File | null>(null)
  const [avatarBlob, setAvatarBlob] = useState<Blob | null>(null)
  const [avatarPreview, setAvatarPreview] = useState<string | null>(null)
  const avatarInputRef = useRef<HTMLInputElement>(null)

  // Dream model toggle (UI-only state)
  const [useSameModel, setUseSameModel] = useState(true)

  // MCP server form state
  const [mcpFormOpen, setMcpFormOpen] = useState(false)
  const [mcpFormKey, setMcpFormKey] = useState<string | null>(null)
  const [mcpForm, setMcpForm] = useState<{
    name: string
    config: McpServerConfig
  }>({
    name: '',
    config: { host: 'local', command: '', args: [], env: {}, uri: '' },
  })

  useEffect(() => {
    if (mode === 'edit' && initialData) {
      const d = initialData
      setForm({
        agent_id: d.agent_id,
        name: d.name,
        description: d.description || '',
        provider: d.provider,
        model: d.model,
        quantization: d.quantization || 'auto_4',
        system_prompt: d.system_prompt || '',
        thinking_depth: d.thinking_depth,
        session_memory_capacity: d.session_memory_capacity,
        max_tokens: d.max_tokens,
        show_thinking: d.show_thinking ?? false,
        show_tool_calls: d.show_tool_calls ?? false,
        silent_read_initiative_chance:
          d.silent_read_initiative_chance ?? 0.0,
        tools: {
          shell: d.shell || null,
          brave_search: d.brave_search,
          brave_search_settings: d.brave_search_settings || {},
          discord: d.discord,
          telegram: d.telegram,
          fetch: d.fetch,
          http_client: d.http_client,
          programmatic_sandbox: d.programmatic_sandbox ?? false,
          timeout: d.tools_timeout || '1m',
          mcp_servers: d.mcp_servers || {},
          tts: d.tts,
          tts_settings: d.tts_settings || {},
          stt: d.stt,
          stt_settings: d.stt_settings || {},
          read_image: d.read_image,
          read_image_settings: d.read_image_settings || {},
          image_gen: d.image_gen,
          image_gen_settings: d.image_gen_settings || {},
        },
        prompt_timeout: d.prompt_timeout,
        heartbeat_interval: d.heartbeat_interval,
        dream_enabled: d.dream_enabled,
        dream_schedule: d.dream_schedule || '',
        dream_provider: d.dream_provider || '',
        dream_model: d.dream_model || '',
        discord_token: d.discord_token || '',
        telegram_token: d.telegram_token || '',
        avatar_url: d.avatar_url,
      })
      setUseSameModel(!d.dream_provider && !d.dream_model)
    }
  }, [mode, initialData])

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

  const updateShell = (value: ShellConfigData | null) => {
    setForm((prev) => ({ ...prev, tools: { ...prev.tools, shell: value } }))
  }

  const updateMcpServers = (servers: Record<string, McpServerConfig>) => {
    setForm((prev) => ({ ...prev, tools: { ...prev.tools, mcp_servers: servers } }))
  }

  const goToNextTab = () => {
    const currentIndex = TABS.findIndex(t => t.key === activeTab)
    if (currentIndex < TABS.length - 1) {
      const nextIndex = currentIndex + 1
      setActiveTab(TABS[nextIndex].key)
      setMaxReachedTab(prev => Math.max(prev, nextIndex))
    }
  }

  const openAddMcpForm = () => {
    setMcpFormKey(null)
    setMcpForm({
      name: '',
      config: { host: 'local', command: '', args: [], env: {}, uri: '' },
    })
    setMcpFormOpen(true)
  }

  const openEditMcpForm = (key: string) => {
    const server = form.tools?.mcp_servers?.[key]
    if (!server) return
    setMcpFormKey(key)
    setMcpForm({
      name: key,
      config: { ...server, args: server.args || [], env: server.env || {} },
    })
    setMcpFormOpen(true)
  }

  const handleSaveMcpServer = () => {
    const name = mcpForm.name.trim()
    if (!name) return
    const config = { ...mcpForm.config }
    if (config.host === 'local') {
      delete config.uri
    } else {
      delete config.command
      delete config.args
    }
    if (Object.keys(config.env || {}).length === 0) delete config.env
    if (config.args && config.args.length === 0) delete config.args

    const servers = { ...(form.tools?.mcp_servers || {}) }
    if (mcpFormKey && mcpFormKey !== name) {
      delete servers[mcpFormKey]
    }
    servers[name] = config
    updateMcpServers(servers)
    setMcpFormOpen(false)
  }

  const handleDeleteMcpServer = (key: string) => {
    const servers = { ...(form.tools?.mcp_servers || {}) }
    delete servers[key]
    updateMcpServers(servers)
  }

  const handleAvatarSelect = (e: React.ChangeEvent<HTMLInputElement>) => {
    const file = e.target.files?.[0]
    if (file) setCropFile(file)
    if (avatarInputRef.current) avatarInputRef.current.value = ''
  }

  const handleAvatarCropped = (blob: Blob) => {
    setCropFile(null)
    setAvatarBlob(blob)
    if (avatarPreview) URL.revokeObjectURL(avatarPreview)
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

    if (form.dream_enabled && !useSameModel) {
      if (!form.dream_provider || !form.dream_model) {
        return
      }
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

      await onSubmit({
        ...form,
        avatar_url: avatarUrl,
        dream_schedule: form.dream_enabled ? (form.dream_schedule || '0 2 * * *') : '',
        dream_provider: (!form.dream_enabled || useSameModel) ? null : form.dream_provider || null,
        dream_model: (!form.dream_enabled || useSameModel) ? null : form.dream_model || null,
      })
    } finally {
      setSubmitting(false)
    }
  }

  const avatarDisplayUrl = avatarPreview || form.avatar_url

  return (
    <>
      {/* Mobile tab nav */}
      <div className="flex md:hidden border-b border-[var(--border)] px-4 gap-2 py-2 overflow-x-auto">
        {TABS.map(({ key, label }, index) => {
          const isDisabled = mode === 'create' && index > maxReachedTab
          return (
            <button
              key={key}
              onClick={() => !isDisabled && setActiveTab(key)}
              disabled={isDisabled}
              className={`px-3 py-1.5 text-sm font-medium rounded-t transition-colors whitespace-nowrap ${activeTab === key ? 'bg-[var(--surface)] text-[var(--text-primary)] border-b-2 border-[var(--accent-primary)]' : isDisabled ? 'text-[var(--text-tertiary)] opacity-40 cursor-not-allowed' : 'text-[var(--text-tertiary)]'}`}
            >
              {label}
            </button>
          )
        })}
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
          {TABS.map(({ key, label, icon: Icon }, index) => {
            const isDisabled = mode === 'create' && index > maxReachedTab
            return (
              <div
                key={key}
                className={`nav-item ${activeTab === key ? 'active' : ''} ${isDisabled ? 'opacity-40 cursor-not-allowed' : ''}`}
                onClick={() => !isDisabled && setActiveTab(key)}
              >
                <Icon size={16} />
                <span>{label}</span>
              </div>
            )
          })}
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
                          tooltip="HuggingFace model ID (e.g., google/gemma-4-E4B-it, Qwen/Qwen3-4B)"
                        />
                      </label>
                      <input
                        style={inputStyle}
                        placeholder="google/gemma-4-E4B-it"
                        value={form.model}
                        onChange={(e) =>
                          updateField(
                            'model',
                            e.target.value
                          )
                        }
                      />
                    </section>
                    {form.provider === 'mistralrs' && (
                      <section
                        style={{ ...fieldStyle, flex: 1 }}
                      >
                        <label style={labelStyle}>
                          <TooltipLabel
                            label="Quantization"
                            tooltip="Model quantization for faster inference and lower memory usage."
                          />
                        </label>
                        <select
                          style={inputStyle}
                          value={form.quantization || 'auto_4'}
                          onChange={(e) =>
                            updateField(
                              'quantization',
                              e.target.value
                            )
                          }
                        >
                          {QUANTIZATION_OPTIONS.map((q) => (
                            <option key={q.value} value={q.value}>
                              {q.label}
                            </option>
                          ))}
                        </select>
                      </section>
                    )}
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
                </div>
              </div>

              {/* Dream Config */}
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
                  Dreaming
                </h4>
                <div style={{ display: 'flex', flexDirection: 'column', gap: '1rem' }}>
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
                      checked={form.dream_enabled || false}
                      onChange={(e) =>
                        updateField('dream_enabled', e.target.checked)
                      }
                    />
                    Enable dreaming
                  </label>
                  {form.dream_enabled && (
                    <>
                      <div style={{ display: 'flex', gap: '1rem', alignItems: 'flex-end' }}>
                        <section style={{ ...fieldStyle, flex: 1 }}>
                          <label style={labelStyle}>
                            <TooltipLabel
                              label="Dream Schedule"
                              tooltip="Cron expression for when the agent dreams. Defaults to daily at 2 AM if left empty."
                            />
                          </label>
                          <input
                            style={inputStyle}
                            placeholder="0 2 * * *"
                            value={form.dream_schedule || ''}
                            onChange={(e) =>
                              updateField('dream_schedule', e.target.value)
                            }
                          />
                        </section>
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
                          checked={useSameModel}
                          onChange={(e) => {
                            setUseSameModel(e.target.checked)
                            if (e.target.checked) {
                              updateField('dream_provider', '')
                              updateField('dream_model', '')
                            }
                          }}
                        />
                        Use same model as main
                      </label>
                      {!useSameModel && (
                        <div style={{ display: 'flex', gap: '1rem', alignItems: 'flex-end' }}>
                          <section style={{ ...fieldStyle, flex: 1 }}>
                            <label style={labelStyle}>
                              <TooltipLabel
                                label="Dream Provider *"
                                tooltip="Provider to use for dreaming. Required when not using the main model."
                              />
                            </label>
                            <select
                              style={{
                                ...inputStyle,
                                borderColor: !form.dream_provider ? 'var(--error, #ef4444)' : undefined,
                              }}
                              value={form.dream_provider || ''}
                              onChange={(e) =>
                                updateField('dream_provider', e.target.value)
                              }
                            >
                              <option value="" disabled>Select provider</option>
                              {PROVIDERS.map((p) => (
                                <option key={p} value={p}>{p}</option>
                              ))}
                            </select>
                          </section>
                          <section style={{ ...fieldStyle, flex: 1 }}>
                            <label style={labelStyle}>
                              <TooltipLabel
                                label="Dream Model *"
                                tooltip="Model to use for dreaming. Required when not using the main model."
                              />
                            </label>
                            <input
                              style={{
                                ...inputStyle,
                                borderColor: !form.dream_model ? 'var(--error, #ef4444)' : undefined,
                              }}
                              value={form.dream_model || ''}
                              onChange={(e) =>
                                updateField('dream_model', e.target.value)
                              }
                            />
                          </section>
                        </div>
                      )}
                    </>
                  )}
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
                {mode === 'create' ? (
                  <button
                    onClick={goToNextTab}
                    disabled={
                      !form.agent_id.trim() || !form.name.trim()
                    }
                    style={{
                      padding: '0.6rem 1.5rem',
                      borderRadius: '0.375rem',
                      border: 'none',
                      background:
                        !form.agent_id.trim() || !form.name.trim()
                          ? 'var(--border)'
                          : 'var(--accent-primary)',
                      color: '#fff',
                      cursor:
                        !form.agent_id.trim() || !form.name.trim()
                          ? 'not-allowed'
                          : 'pointer',
                      fontSize: '0.85rem',
                      fontWeight: 500,
                    }}
                  >
                    Next
                  </button>
                ) : (
                  <button
                    onClick={handleSubmit}
                    disabled={submitting || !form.name.trim()}
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
                    {submitting ? 'Saving...' : 'Save Changes'}
                  </button>
                )}
              </div>
            </div>
          )}

          {/* ─── Tools Tab ─── */}
          {activeTab === 'tools' && (
            <div
              style={{
                maxWidth: '720px',
                display: 'flex',
                flexDirection: 'column',
                gap: '1.5rem',
              }}
            >
              {/* Tool Toggles */}
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
                  Enabled Tools
                </h4>
                <div
                  style={{
                    display: 'grid',
                    gridTemplateColumns: '1fr 1fr',
                    gap: '0.5rem',
                  }}
                >
                  {(
                    [
                      ['discord', 'Discord'],
                      ['telegram', 'Telegram'],
                      ['fetch', 'Fetch Webpage'],
                      ['http_client', 'HTTP Client'],
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
                        checked={form.tools?.[key] ?? false}
                        onChange={(e) =>
                          updateTool(key, e.target.checked)
                        }
                      />
                      {label}
                    </label>
                  ))}
                </div>
              </div>

              {/* Tool Settings */}
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
                  Tool Settings
                </h4>
                <section style={fieldStyle}>
                  <label style={labelStyle}>
                    <TooltipLabel
                      label="Tool Timeout"
                      tooltip="Maximum time for a single tool execution (e.g. 1m, 30s)."
                    />
                  </label>
                  <input
                    style={{ ...inputStyle, maxWidth: '200px' }}
                    placeholder="1m"
                    value={form.tools?.timeout || ''}
                    onChange={(e) =>
                      updateToolField('timeout', e.target.value)
                    }
                  />
                </section>
              </div>

              {/* Programmatic Sandbox */}
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
                  Programmatic Sandbox
                </h4>
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
                      form.tools?.programmatic_sandbox ?? false
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
              </div>

              {/* Brave Search */}
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
                  Brave Search
                </h4>
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
                      marginBottom: form.tools?.brave_search
                        ? '0.75rem'
                        : 0,
                    }}
                  >
                    <input
                      type="checkbox"
                      checked={
                        form.tools?.brave_search ?? false
                      }
                      onChange={(e) =>
                        updateTool(
                          'brave_search',
                          e.target.checked
                        )
                      }
                    />
                    Enable Brave Search
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
              </div>

              {/* TTS (Text-to-Speech) */}
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
                  Text-to-Speech
                </h4>
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
                      marginBottom: form.tools?.tts
                        ? '0.75rem'
                        : 0,
                    }}
                  >
                    <input
                      type="checkbox"
                      checked={form.tools?.tts ?? false}
                      onChange={(e) =>
                        updateTool('tts', e.target.checked)
                      }
                    />
                    Enable TTS
                  </label>
                  {form.tools?.tts && (
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
                            marginBottom: '0.25rem',
                            fontSize: '0.75rem',
                            color: 'var(--text-secondary)',
                          }}
                        >
                          Provider
                        </label>
                        <select
                          style={inputStyle}
                          value={
                            form.tools?.tts_settings?.provider ||
                            'piper'
                          }
                          onChange={(e) =>
                            setForm((prev) => ({
                              ...prev,
                              tools: {
                                ...prev.tools!,
                                tts_settings: {
                                  ...prev.tools?.tts_settings,
                                  provider: e.target
                                    .value as import('../interfaces/types').TtsProvider,
                                },
                              },
                            }))
                          }
                        >
                          <option value="piper">Piper (Local)</option>
                          <option value="kitten">Kitten (Local)</option>
                          <option value="openai">OpenAI</option>
                          <option value="openrouter">
                            OpenRouter
                          </option>
                          <option value="elevenlabs">
                            ElevenLabs
                          </option>
                        </select>
                      </div>
                      <div>
                        <label
                          style={{
                            display: 'block',
                            marginBottom: '0.25rem',
                            fontSize: '0.75rem',
                            color: 'var(--text-secondary)',
                          }}
                        >
                          Model (optional)
                        </label>
                        <input
                          style={inputStyle}
                          type="text"
                          placeholder={
                            (form.tools?.tts_settings?.provider || 'piper') === 'piper'
                              ? 'Model name in .vizier/models/tts/'
                              : (form.tools?.tts_settings?.provider || 'piper') === 'kitten'
                                ? 'e.g. kitten-nano-en-v0_1-fp16'
                                : 'Provider default'
                          }
                          value={
                            form.tools?.tts_settings?.model || ''
                          }
                          onChange={(e) =>
                            setForm((prev) => ({
                              ...prev,
                              tools: {
                                ...prev.tools!,
                                tts_settings: {
                                  ...prev.tools?.tts_settings,
                                  model:
                                    e.target.value || undefined,
                                },
                              },
                            }))
                          }
                        />
                      </div>
                      <div>
                        <label
                          style={{
                            display: 'block',
                            marginBottom: '0.25rem',
                            fontSize: '0.75rem',
                            color: 'var(--text-secondary)',
                          }}
                        >
                          Voice (optional)
                        </label>
                        <input
                          style={inputStyle}
                          type="text"
                          placeholder={
                            (form.tools?.tts_settings?.provider || 'piper') === 'piper'
                              ? 'Default: "0" (speaker ID)'
                              : (form.tools?.tts_settings?.provider || 'piper') === 'kitten'
                                ? 'Default: "0" (0-7, 4M/4F)'
                                : 'Default: "alloy"'
                          }
                          value={
                            form.tools?.tts_settings?.voice || ''
                          }
                          onChange={(e) =>
                            setForm((prev) => ({
                              ...prev,
                              tools: {
                                ...prev.tools!,
                                tts_settings: {
                                  ...prev.tools?.tts_settings,
                                  voice:
                                    e.target.value || undefined,
                                },
                              },
                            }))
                          }
                        />
                      </div>
                      <div>
                        <label
                          style={{
                            display: 'block',
                            marginBottom: '0.25rem',
                            fontSize: '0.75rem',
                            color: 'var(--text-secondary)',
                          }}
                        >
                          Speed
                        </label>
                        <input
                          style={inputStyle}
                          type="number"
                          min="0.25"
                          max="4.0"
                          step="0.25"
                          placeholder="1.0"
                          value={
                            form.tools?.tts_settings?.speed ?? ''
                          }
                          onChange={(e) =>
                            setForm((prev) => ({
                              ...prev,
                              tools: {
                                ...prev.tools!,
                                tts_settings: {
                                  ...prev.tools?.tts_settings,
                                  speed: e.target.value
                                    ? parseFloat(e.target.value)
                                    : undefined,
                                },
                              },
                            }))
                          }
                        />
                      </div>
                    </div>
                  )}
                </div>
              </div>

              {/* STT (Speech-to-Text) */}
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
                  Speech-to-Text
                </h4>
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
                      marginBottom: form.tools?.stt
                        ? '0.75rem'
                        : 0,
                    }}
                  >
                    <input
                      type="checkbox"
                      checked={form.tools?.stt ?? false}
                      onChange={(e) =>
                        updateTool('stt', e.target.checked)
                      }
                    />
                    Enable STT
                  </label>
                  {form.tools?.stt && (
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
                            marginBottom: '0.25rem',
                            fontSize: '0.75rem',
                            color: 'var(--text-secondary)',
                          }}
                        >
                          Provider
                        </label>
                        <select
                          style={inputStyle}
                          value={
                            form.tools?.stt_settings?.provider ||
                            'sense_voice'
                          }
                          onChange={(e) =>
                            setForm((prev) => ({
                              ...prev,
                              tools: {
                                ...prev.tools!,
                                stt_settings: {
                                  ...prev.tools?.stt_settings,
                                  provider: e.target
                                    .value as import('../interfaces/types').SttProvider,
                                },
                              },
                            }))
                          }
                        >
                          <option value="sense_voice">
                            SenseVoice (Local)
                          </option>
                          <option value="openai">OpenAI Whisper</option>
                          <option value="elevenlabs">
                            ElevenLabs
                          </option>
                        </select>
                      </div>
                      <div>
                        <label
                          style={{
                            display: 'block',
                            marginBottom: '0.25rem',
                            fontSize: '0.75rem',
                            color: 'var(--text-secondary)',
                          }}
                        >
                          Model (optional)
                        </label>
                        <input
                          style={inputStyle}
                          type="text"
                          placeholder={
                            (form.tools?.stt_settings?.provider || 'sense_voice') === 'sense_voice'
                              ? 'e.g. sherpa-onnx-sense-voice-zh-en-ja-ko-yue-int8-2024-07-17'
                              : (form.tools?.stt_settings?.provider || 'sense_voice') === 'openai'
                                ? 'whisper-1'
                                : 'scribe_v1'
                          }
                          value={
                            form.tools?.stt_settings?.model || ''
                          }
                          onChange={(e) =>
                            setForm((prev) => ({
                              ...prev,
                              tools: {
                                ...prev.tools!,
                                stt_settings: {
                                  ...prev.tools?.stt_settings,
                                  model:
                                    e.target.value || undefined,
                                },
                              },
                            }))
                          }
                        />
                      </div>
                      <div>
                        <label
                          style={{
                            display: 'block',
                            marginBottom: '0.25rem',
                            fontSize: '0.75rem',
                            color: 'var(--text-secondary)',
                          }}
                        >
                          Language (optional)
                        </label>
                        <input
                          style={inputStyle}
                          type="text"
                          placeholder="e.g. en, zh, auto (default: auto)"
                          value={
                            form.tools?.stt_settings?.language || ''
                          }
                          onChange={(e) =>
                            setForm((prev) => ({
                              ...prev,
                              tools: {
                                ...prev.tools!,
                                stt_settings: {
                                  ...prev.tools?.stt_settings,
                                  language:
                                    e.target.value || undefined,
                                },
                              },
                            }))
                          }
                        />
                      </div>
                    </div>
                  )}
                </div>
              </div>

              {/* Read Image (vision model) */}
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
                  Read Image (vision model)
                </h4>
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
                      marginBottom: form.tools?.read_image
                        ? '0.75rem'
                        : 0,
                    }}
                  >
                    <input
                      type="checkbox"
                      checked={form.tools?.read_image ?? false}
                      onChange={(e) =>
                        updateTool('read_image', e.target.checked)
                      }
                    />
                    Use vision model for image reading
                  </label>
                  {form.tools?.read_image && (
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
                            marginBottom: '0.25rem',
                            fontSize: '0.75rem',
                            color: 'var(--text-secondary)',
                          }}
                        >
                          Provider
                        </label>
                        <select
                          style={inputStyle}
                          value={
                            form.tools?.read_image_settings?.provider ||
                            ''
                          }
                          onChange={(e) =>
                            setForm((prev) => ({
                              ...prev,
                              tools: {
                                ...prev.tools!,
                                read_image_settings: {
                                  ...prev.tools?.read_image_settings,
                                  provider:
                                    e.target.value || undefined,
                                },
                              },
                            }))
                          }
                        >
                          <option value="">Select provider</option>
                          <option value="ollama">Ollama</option>
                          <option value="openai">OpenAI</option>
                          <option value="anthropic">Anthropic</option>
                          <option value="openrouter">OpenRouter</option>
                          <option value="gemini">Gemini</option>
                          <option value="deepseek">DeepSeek</option>
                          <option value="mimo">Xiaomi MiMo</option>
                          <option value="llama_cpp">Llama.cpp</option>
                        </select>
                      </div>
                      <div>
                        <label
                          style={{
                            display: 'block',
                            marginBottom: '0.25rem',
                            fontSize: '0.75rem',
                            color: 'var(--text-secondary)',
                          }}
                        >
                          Model
                        </label>
                        <input
                          style={inputStyle}
                          type="text"
                          placeholder="e.g. gpt-4o, claude-sonnet-4-5, llama3.2-vision"
                          value={
                            form.tools?.read_image_settings?.model || ''
                          }
                          onChange={(e) =>
                            setForm((prev) => ({
                              ...prev,
                              tools: {
                                ...prev.tools!,
                                read_image_settings: {
                                  ...prev.tools?.read_image_settings,
                                  model:
                                    e.target.value || undefined,
                                },
                              },
                            }))
                          }
                        />
                      </div>
                    </div>
                  )}
                </div>
              </div>

              {/* Image Generation */}
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
                  Image Generation
                </h4>
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
                      marginBottom: form.tools?.image_gen
                        ? '0.75rem'
                        : 0,
                    }}
                  >
                    <input
                      type="checkbox"
                      checked={form.tools?.image_gen ?? false}
                      onChange={(e) =>
                        updateTool('image_gen', e.target.checked)
                      }
                    />
                    Enable Image Generation
                  </label>
                  {form.tools?.image_gen && (
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
                            marginBottom: '0.25rem',
                            fontSize: '0.75rem',
                            color: 'var(--text-secondary)',
                          }}
                        >
                          Provider
                        </label>
                        <select
                          style={inputStyle}
                          value={
                            form.tools?.image_gen_settings?.provider ||
                            'openai'
                          }
                          onChange={(e) =>
                            setForm((prev) => ({
                              ...prev,
                              tools: {
                                ...prev.tools!,
                                image_gen_settings: {
                                  ...prev.tools?.image_gen_settings,
                                  provider: e.target
                                    .value as import('../interfaces/types').ImageGenProvider,
                                },
                              },
                            }))
                          }
                        >
                          <option value="openai">OpenAI</option>
                        </select>
                      </div>
                      <div>
                        <label
                          style={{
                            display: 'block',
                            marginBottom: '0.25rem',
                            fontSize: '0.75rem',
                            color: 'var(--text-secondary)',
                          }}
                        >
                          Model (optional)
                        </label>
                        <input
                          style={inputStyle}
                          type="text"
                          placeholder="e.g. dall-e-3, gpt-image-1"
                          value={
                            form.tools?.image_gen_settings?.model || ''
                          }
                          onChange={(e) =>
                            setForm((prev) => ({
                              ...prev,
                              tools: {
                                ...prev.tools!,
                                image_gen_settings: {
                                  ...prev.tools?.image_gen_settings,
                                  model:
                                    e.target.value || undefined,
                                },
                              },
                            }))
                          }
                        />
                      </div>
                      <div>
                        <label
                          style={{
                            display: 'block',
                            marginBottom: '0.25rem',
                            fontSize: '0.75rem',
                            color: 'var(--text-secondary)',
                          }}
                        >
                          Size
                        </label>
                        <select
                          style={inputStyle}
                          value={
                            form.tools?.image_gen_settings?.size ||
                            '1024x1024'
                          }
                          onChange={(e) =>
                            setForm((prev) => ({
                              ...prev,
                              tools: {
                                ...prev.tools!,
                                image_gen_settings: {
                                  ...prev.tools?.image_gen_settings,
                                  size: e.target.value || undefined,
                                },
                              },
                            }))
                          }
                        >
                          <option value="1024x1024">1024x1024 (square)</option>
                          <option value="1024x1792">1024x1792 (portrait)</option>
                          <option value="1792x1024">1792x1024 (landscape)</option>
                        </select>
                      </div>
                    </div>
                  )}
                </div>
              </div>

              {/* Shell Config */}
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
                  Shell Configuration
                </h4>
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
                      marginBottom: form.tools?.shell
                        ? '0.75rem'
                        : 0,
                    }}
                  >
                    <input
                      type="checkbox"
                      checked={form.tools?.shell !== null}
                      onChange={(e) =>
                        updateShell(
                          e.target.checked
                            ? {
                                environment: 'local',
                                path: '.',
                              }
                            : null
                        )
                      }
                    />
                    Enable Shell
                  </label>
                  {form.tools?.shell && (() => {
                    const shell = form.tools!.shell!
                    return (
                    <div
                      style={{
                        display: 'flex',
                        flexDirection: 'column',
                        gap: '1rem',
                        paddingLeft: '1.5rem',
                      }}
                    >
                      {/* Environment radio */}
                      <div
                        style={{
                          display: 'flex',
                          gap: '1.5rem',
                          fontSize: '0.8rem',
                        }}
                      >
                        <label
                          style={{
                            display: 'flex',
                            alignItems: 'center',
                            gap: '0.4rem',
                            cursor: 'pointer',
                          }}
                        >
                          <input
                            type="radio"
                            name="shell-env"
                            checked={
                              shell.environment === 'local'
                            }
                            onChange={() =>
                              updateShell({
                                ...shell,
                                environment: 'local',
                              })
                            }
                          />
                          Local
                        </label>
                        <label
                          style={{
                            display: 'flex',
                            alignItems: 'center',
                            gap: '0.4rem',
                            cursor: 'pointer',
                          }}
                        >
                          <input
                            type="radio"
                            name="shell-env"
                            checked={
                              shell.environment === 'docker'
                            }
                            onChange={() =>
                              updateShell({
                                ...shell,
                                environment: 'docker',
                              })
                            }
                          />
                          Docker
                        </label>
                      </div>

                      {/* Local config */}
                      {shell.environment ===
                        'local' && (
                        <div
                          style={{
                            display: 'flex',
                            flexDirection: 'column',
                            gap: '0.75rem',
                          }}
                        >
                          <section style={fieldStyle}>
                            <label style={labelStyle}>
                              Working Directory
                            </label>
                            <input
                              style={inputStyle}
                              placeholder="."
                              value={
                                shell.path || ''
                              }
                              onChange={(e) =>
                                updateShell({
                                  ...shell,
                                  path:
                                    e.target.value ||
                                    undefined,
                                })
                              }
                            />
                          </section>
                          <section style={fieldStyle}>
                            <label style={labelStyle}>
                              Environment Variables
                            </label>
                            <textarea
                              style={{
                                ...inputStyle,
                                minHeight: '60px',
                                fontFamily:
                                  'monospace',
                                fontSize: '0.8rem',
                                resize: 'vertical',
                              }}
                              placeholder="KEY=value (one per line)"
                              value={Object.entries(
                                shell.env ||
                                  {}
                              )
                                .map(
                                  ([k, v]) =>
                                    `${k}=${v}`
                                )
                                .join('\n')}
                              onChange={(e) => {
                                const env: Record<
                                  string,
                                  string
                                > = {}
                                e.target.value
                                  .split('\n')
                                  .forEach((line) => {
                                    const trimmed =
                                      line.trim()
                                    if (
                                      trimmed.includes(
                                        '='
                                      )
                                    ) {
                                      const idx =
                                        trimmed.indexOf(
                                          '='
                                        )
                                      env[
                                        trimmed.substring(
                                          0,
                                          idx
                                        )
                                      ] =
                                        trimmed.substring(
                                          idx + 1
                                        )
                                    }
                                  })
                                updateShell({
                                  ...shell,
                                  env,
                                })
                              }}
                            />
                          </section>
                        </div>
                      )}

                      {/* Docker config */}
                      {shell.environment ===
                        'docker' && (
                        <div
                          style={{
                            display: 'flex',
                            flexDirection: 'column',
                            gap: '0.75rem',
                          }}
                        >
                          {/* Image Source radio */}
                          <div
                            style={{
                              display: 'flex',
                              gap: '1.5rem',
                              fontSize: '0.8rem',
                            }}
                          >
                            <label
                              style={{
                                display: 'flex',
                                alignItems: 'center',
                                gap: '0.4rem',
                                cursor: 'pointer',
                              }}
                            >
                              <input
                                type="radio"
                                name="shell-docker-source"
                                checked={
                                  shell.image
                                    ?.source === 'pull'
                                }
                                onChange={() =>
                                  updateShell({
                                    ...shell,
                                    image: {
                                      source: 'pull',
                                      name: '',
                                    },
                                  })
                                }
                              />
                              Pull
                            </label>
                            <label
                              style={{
                                display: 'flex',
                                alignItems: 'center',
                                gap: '0.4rem',
                                cursor: 'pointer',
                              }}
                            >
                              <input
                                type="radio"
                                name="shell-docker-source"
                                checked={
                                  shell.image
                                    ?.source ===
                                  'dockerfile'
                                }
                                onChange={() =>
                                  updateShell({
                                    ...shell,
                                    image: {
                                      source:
                                        'dockerfile',
                                      path: '',
                                      name: '',
                                    },
                                  })
                                }
                              />
                              Dockerfile
                            </label>
                          </div>

                          {shell.image
                            ?.source === 'pull' && (
                            <section style={fieldStyle}>
                              <label style={labelStyle}>
                                Image Name
                              </label>
                              <input
                                style={inputStyle}
                                placeholder="e.g. ubuntu:latest"
                                value={
                                  shell.image
                                    ?.name || ''
                                }
                                onChange={(e) =>
                                  updateShell({
                                    ...shell,
                                    image: {
                                      source: 'pull',
                                      name: e.target
                                        .value,
                                    },
                                  })
                                }
                              />
                            </section>
                          )}

                          {shell.image
                            ?.source === 'dockerfile' && (
                            <>
                              <section style={fieldStyle}>
                                <label style={labelStyle}>
                                  Dockerfile Path
                                </label>
                                <input
                                  style={inputStyle}
                                  placeholder="./Dockerfile"
                                  value={
                                    shell.image
                                      ?.path || ''
                                  }
                                  onChange={(e) =>
                                    updateShell({
                                      ...shell,
                                      image: {
                                        source:
                                          'dockerfile',
                                        path: e.target
                                          .value,
                                        name: shell
                                          .image
                                          ?.source ===
                                          'dockerfile'
                                          ? shell
                                              .image
                                              .name
                                          : '',
                                      },
                                    })
                                  }
                                />
                              </section>
                              <section style={fieldStyle}>
                                <label style={labelStyle}>
                                  Image Name
                                </label>
                                <input
                                  style={inputStyle}
                                  placeholder="e.g. my-agent"
                                  value={
                                    shell.image
                                      ?.name || ''
                                  }
                                  onChange={(e) =>
                                    updateShell({
                                      ...shell,
                                      image: {
                                        source:
                                          'dockerfile',
                                        path: shell
                                          .image
                                          ?.source ===
                                          'dockerfile'
                                          ? shell
                                              .image
                                              .path
                                          : '',
                                        name: e.target
                                          .value,
                                      },
                                    })
                                  }
                                />
                              </section>
                            </>
                          )}

                          <section style={fieldStyle}>
                            <label style={labelStyle}>
                              Container Name
                            </label>
                            <input
                              style={inputStyle}
                              placeholder="Optional"
                              value={
                                shell
                                  .container_name || ''
                              }
                              onChange={(e) =>
                                updateShell({
                                  ...shell,
                                  container_name:
                                    e.target.value ||
                                    undefined,
                                })
                              }
                            />
                          </section>

                          <section style={fieldStyle}>
                            <label style={labelStyle}>
                              Environment Variables
                            </label>
                            <textarea
                              style={{
                                ...inputStyle,
                                minHeight: '60px',
                                fontFamily: 'monospace',
                                fontSize: '0.8rem',
                                resize: 'vertical',
                              }}
                              placeholder="KEY=value (one per line)"
                              value={Object.entries(
                                shell.env || {}
                              )
                                .map(
                                  ([k, v]) =>
                                    `${k}=${v}`
                                )
                                .join('\n')}
                              onChange={(e) => {
                                const env: Record<
                                  string,
                                  string
                                > = {}
                                e.target.value
                                  .split('\n')
                                  .forEach((line) => {
                                    const trimmed =
                                      line.trim()
                                    if (
                                      trimmed.includes(
                                        '='
                                      )
                                    ) {
                                      const idx =
                                        trimmed.indexOf(
                                          '='
                                        )
                                      env[
                                        trimmed.substring(
                                          0,
                                          idx
                                        )
                                      ] =
                                        trimmed.substring(
                                          idx + 1
                                        )
                                    }
                                  })
                                updateShell({
                                  ...shell,
                                  env,
                                })
                              }}
                            />
                          </section>
                        </div>
                      )}
                    </div>
                    )
                  })()}
                </div>
              </div>

              {/* MCP Servers */}
              <div>
                <div
                  style={{
                    display: 'flex',
                    alignItems: 'center',
                    justifyContent: 'space-between',
                    marginBottom: '0.75rem',
                    paddingBottom: '0.5rem',
                    borderBottom: '1px solid var(--border)',
                  }}
                >
                  <h4
                    style={{
                      fontSize: '0.85rem',
                      fontWeight: 600,
                      color: 'var(--text-primary)',
                      margin: 0,
                    }}
                  >
                    MCP Servers
                  </h4>
                  <button
                    className="btn btn-primary"
                    onClick={openAddMcpForm}
                    style={{
                      fontSize: '0.8rem',
                      padding: '4px 12px',
                    }}
                  >
                    + Add Server
                  </button>
                </div>

                {/* MCP server list */}
                {Object.keys(form.tools?.mcp_servers || {})
                  .length > 0 ? (
                  <div
                    style={{
                      display: 'flex',
                      flexDirection: 'column',
                      gap: '0.5rem',
                    }}
                  >
                    {Object.entries(
                      form.tools?.mcp_servers || {}
                    ).map(([name, config]) => (
                      <div
                        key={name}
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
                        <div
                          style={{
                            display: 'flex',
                            alignItems: 'center',
                            gap: '1rem',
                          }}
                        >
                          <span
                            style={{
                              fontSize: '0.85rem',
                              fontWeight: 500,
                              color: 'var(--text-primary)',
                            }}
                          >
                            {name}
                          </span>
                          <span
                            style={{
                              fontSize: '0.75rem',
                              color: 'var(--text-tertiary)',
                              background:
                                'var(--background)',
                              padding: '2px 8px',
                              borderRadius: '4px',
                            }}
                          >
                            {config.host}
                          </span>
                          <span
                            style={{
                              fontSize: '0.75rem',
                              color: 'var(--text-tertiary)',
                              fontFamily: 'monospace',
                            }}
                          >
                            {config.host === 'local'
                              ? config.command
                              : config.uri}
                          </span>
                        </div>
                        <div
                          style={{
                            display: 'flex',
                            gap: '0.25rem',
                          }}
                        >
                          <button
                            className="btn btn-ghost"
                            onClick={() =>
                              openEditMcpForm(name)
                            }
                            style={{
                              fontSize: '0.8rem',
                              padding: '4px 8px',
                            }}
                          >
                            Edit
                          </button>
                          <button
                            className="btn btn-ghost"
                            onClick={() =>
                              handleDeleteMcpServer(name)
                            }
                            style={{
                              fontSize: '0.8rem',
                              padding: '4px 8px',
                              color: '#ef4444',
                            }}
                          >
                            Delete
                          </button>
                        </div>
                      </div>
                    ))}
                  </div>
                ) : (
                  <p
                    style={{
                      color: 'var(--text-tertiary)',
                      fontSize: '0.85rem',
                    }}
                  >
                    No MCP servers configured.
                  </p>
                )}

                {/* Inline MCP form */}
                {mcpFormOpen && (
                  <div
                    style={{
                      marginTop: '1rem',
                      padding: '1rem',
                      border: '1px solid var(--border)',
                      borderRadius: '0.5rem',
                      background: 'var(--surface)',
                      display: 'flex',
                      flexDirection: 'column',
                      gap: '0.75rem',
                    }}
                  >
                    <section style={fieldStyle}>
                      <label style={labelStyle}>Name</label>
                      <input
                        style={inputStyle}
                        placeholder="server-name"
                        value={mcpForm.name}
                        onChange={(e) =>
                          setMcpForm((prev) => ({
                            ...prev,
                            name: e.target.value,
                          }))
                        }
                      />
                    </section>

                    <div
                      style={{
                        display: 'flex',
                        gap: '1.5rem',
                        fontSize: '0.8rem',
                      }}
                    >
                      <label
                        style={{
                          display: 'flex',
                          alignItems: 'center',
                          gap: '0.4rem',
                          cursor: 'pointer',
                        }}
                      >
                        <input
                          type="radio"
                          name="mcp-type"
                          checked={
                            mcpForm.config.host === 'local'
                          }
                          onChange={() =>
                            setMcpForm((prev) => ({
                              ...prev,
                              config: {
                                ...prev.config,
                                host: 'local',
                                command: '',
                                args: [],
                                uri: undefined,
                              },
                            }))
                          }
                        />
                        Local stdio
                      </label>
                      <label
                        style={{
                          display: 'flex',
                          alignItems: 'center',
                          gap: '0.4rem',
                          cursor: 'pointer',
                        }}
                      >
                        <input
                          type="radio"
                          name="mcp-type"
                          checked={
                            mcpForm.config.host === 'http'
                          }
                          onChange={() =>
                            setMcpForm((prev) => ({
                              ...prev,
                              config: {
                                ...prev.config,
                                host: 'http',
                                uri: '',
                                command: undefined,
                                args: undefined,
                              },
                            }))
                          }
                        />
                        HTTP SSE
                      </label>
                    </div>

                    {mcpForm.config.host === 'local' ? (
                      <>
                        <section style={fieldStyle}>
                          <label style={labelStyle}>
                            Command
                          </label>
                          <input
                            style={inputStyle}
                            placeholder="npx"
                            value={
                              mcpForm.config.command || ''
                            }
                            onChange={(e) =>
                              setMcpForm((prev) => ({
                                ...prev,
                                config: {
                                  ...prev.config,
                                  command: e.target.value,
                                },
                              }))
                            }
                          />
                        </section>
                        <section style={fieldStyle}>
                          <label style={labelStyle}>
                            Args (space-separated)
                          </label>
                          <input
                            style={inputStyle}
                            placeholder="-y @modelcontextprotocol/server-everything"
                            value={
                              mcpForm.config.args?.join(
                                ' '
                              ) || ''
                            }
                            onChange={(e) =>
                              setMcpForm((prev) => ({
                                ...prev,
                                config: {
                                  ...prev.config,
                                  args: e.target.value
                                    .split(' ')
                                    .filter(Boolean),
                                },
                              }))
                            }
                          />
                        </section>
                      </>
                    ) : (
                      <section style={fieldStyle}>
                        <label style={labelStyle}>
                          URI
                        </label>
                        <input
                          style={inputStyle}
                          placeholder="http://localhost:3000/sse"
                          value={
                            mcpForm.config.uri || ''
                          }
                          onChange={(e) =>
                            setMcpForm((prev) => ({
                              ...prev,
                              config: {
                                ...prev.config,
                                uri: e.target.value,
                              },
                            }))
                          }
                        />
                      </section>
                    )}

                    <section style={fieldStyle}>
                      <label style={labelStyle}>
                        Environment Variables
                      </label>
                      <textarea
                        style={{
                          ...inputStyle,
                          minHeight: '60px',
                          fontFamily: 'monospace',
                          fontSize: '0.8rem',
                          resize: 'vertical',
                        }}
                        placeholder="KEY=value (one per line)"
                        value={Object.entries(
                          mcpForm.config.env || {}
                        )
                          .map(([k, v]) => `${k}=${v}`)
                          .join('\n')}
                        onChange={(e) => {
                          const env: Record<
                            string,
                            string
                          > = {}
                          e.target.value
                            .split('\n')
                            .forEach((line) => {
                              const trimmed = line.trim()
                              if (
                                trimmed.includes('=')
                              ) {
                                const idx =
                                  trimmed.indexOf('=')
                                env[
                                  trimmed.substring(
                                    0,
                                    idx
                                  )
                                ] = trimmed.substring(
                                  idx + 1
                                )
                              }
                            })
                          setMcpForm((prev) => ({
                            ...prev,
                            config: {
                              ...prev.config,
                              env,
                            },
                          }))
                        }}
                      />
                    </section>

                    <div
                      style={{
                        display: 'flex',
                        gap: '0.5rem',
                        justifyContent: 'flex-end',
                      }}
                    >
                      <button
                        className="btn btn-secondary"
                        onClick={() =>
                          setMcpFormOpen(false)
                        }
                        style={{
                          fontSize: '0.8rem',
                          padding: '6px 16px',
                        }}
                      >
                        Cancel
                      </button>
                      <button
                        className="btn btn-primary"
                        onClick={handleSaveMcpServer}
                        style={{
                          fontSize: '0.8rem',
                          padding: '6px 16px',
                        }}
                      >
                        {mcpFormKey
                          ? 'Update'
                          : 'Add'} Server
                      </button>
                    </div>
                  </div>
                )}
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
                {mode === 'create' ? (
                  <button
                    onClick={goToNextTab}
                    style={{
                      padding: '0.6rem 1.5rem',
                      borderRadius: '0.375rem',
                      border: 'none',
                      background: 'var(--accent-primary)',
                      color: '#fff',
                      cursor: 'pointer',
                      fontSize: '0.85rem',
                      fontWeight: 500,
                    }}
                  >
                    Next
                  </button>
                ) : (
                  <button
                    onClick={handleSubmit}
                    disabled={submitting || !form.name.trim()}
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
                    {submitting ? 'Saving...' : 'Save Changes'}
                  </button>
                )}
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
                {mode === 'create' ? (
                  <button
                    onClick={goToNextTab}
                    style={{
                      padding: '0.6rem 1.5rem',
                      borderRadius: '0.375rem',
                      border: 'none',
                      background: 'var(--accent-primary)',
                      color: '#fff',
                      cursor: 'pointer',
                      fontSize: '0.85rem',
                      fontWeight: 500,
                    }}
                  >
                    Next
                  </button>
                ) : (
                  <button
                    onClick={handleSubmit}
                    disabled={submitting || !form.name.trim()}
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
                    {submitting ? 'Saving...' : 'Save Changes'}
                  </button>
                )}
              </div>
            </div>
          )}

          {/* ─── Review Tab ─── */}
          {activeTab === 'review' && mode === 'create' && (
            <div
              style={{
                maxWidth: '720px',
                display: 'flex',
                flexDirection: 'column',
                gap: '1.5rem',
              }}
            >
              <p
                style={{
                  color: 'var(--text-secondary)',
                  fontSize: '14px',
                  marginBottom: '0.5rem',
                }}
              >
                Review your agent configuration before creating.
              </p>

              {/* Agent Info */}
              <div
                style={{
                  padding: '1rem',
                  border: '1px solid var(--border)',
                  borderRadius: '0.5rem',
                  background: 'var(--surface)',
                }}
              >
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
                  Agent Details
                </h4>
                <div style={{ display: 'flex', flexDirection: 'column', gap: '0.5rem' }}>
                  <div style={{ display: 'flex', justifyContent: 'space-between' }}>
                    <span style={{ fontSize: '0.8rem', color: 'var(--text-secondary)' }}>Name</span>
                    <span style={{ fontSize: '0.8rem', color: 'var(--text-primary)', fontWeight: 500 }}>
                      {form.name || '(not set)'}
                    </span>
                  </div>
                  <div style={{ display: 'flex', justifyContent: 'space-between' }}>
                    <span style={{ fontSize: '0.8rem', color: 'var(--text-secondary)' }}>ID</span>
                    <span style={{ fontSize: '0.8rem', color: 'var(--text-primary)', fontFamily: 'monospace' }}>
                      {form.agent_id || '(not set)'}
                    </span>
                  </div>
                  <div style={{ display: 'flex', justifyContent: 'space-between' }}>
                    <span style={{ fontSize: '0.8rem', color: 'var(--text-secondary)' }}>Description</span>
                    <span style={{ fontSize: '0.8rem', color: 'var(--text-primary)', maxWidth: '60%', textAlign: 'right' }}>
                      {form.description || '(none)'}
                    </span>
                  </div>
                </div>
              </div>

              {/* Model Config */}
              <div
                style={{
                  padding: '1rem',
                  border: '1px solid var(--border)',
                  borderRadius: '0.5rem',
                  background: 'var(--surface)',
                }}
              >
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
                  Model Configuration
                </h4>
                <div style={{ display: 'flex', flexDirection: 'column', gap: '0.5rem' }}>
                  <div style={{ display: 'flex', justifyContent: 'space-between' }}>
                    <span style={{ fontSize: '0.8rem', color: 'var(--text-secondary)' }}>Provider</span>
                    <span style={{ fontSize: '0.8rem', color: 'var(--text-primary)', fontWeight: 500 }}>
                      {form.provider}
                    </span>
                  </div>
                  <div style={{ display: 'flex', justifyContent: 'space-between' }}>
                    <span style={{ fontSize: '0.8rem', color: 'var(--text-secondary)' }}>Model</span>
                    <span style={{ fontSize: '0.8rem', color: 'var(--text-primary)', fontFamily: 'monospace' }}>
                      {form.model}
                    </span>
                  </div>
                  <div style={{ display: 'flex', justifyContent: 'space-between' }}>
                    <span style={{ fontSize: '0.8rem', color: 'var(--text-secondary)' }}>Max Tokens</span>
                    <span style={{ fontSize: '0.8rem', color: 'var(--text-primary)' }}>
                      {form.max_tokens}
                    </span>
                  </div>
                  <div style={{ display: 'flex', justifyContent: 'space-between' }}>
                    <span style={{ fontSize: '0.8rem', color: 'var(--text-secondary)' }}>Thinking Depth</span>
                    <span style={{ fontSize: '0.8rem', color: 'var(--text-primary)' }}>
                      {form.thinking_depth}
                    </span>
                  </div>
                </div>
              </div>

              {/* Tools Summary */}
              <div
                style={{
                  padding: '1rem',
                  border: '1px solid var(--border)',
                  borderRadius: '0.5rem',
                  background: 'var(--surface)',
                }}
              >
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
                  Enabled Tools
                </h4>
                <div style={{ display: 'flex', flexWrap: 'wrap', gap: '0.5rem' }}>
                  {form.tools?.brave_search && (
                    <span style={{ padding: '0.25rem 0.5rem', borderRadius: '0.25rem', background: 'var(--accent-primary)', color: '#fff', fontSize: '0.75rem' }}>
                      Brave Search
                    </span>
                  )}
                  {form.tools?.discord && (
                    <span style={{ padding: '0.25rem 0.5rem', borderRadius: '0.25rem', background: 'var(--accent-primary)', color: '#fff', fontSize: '0.75rem' }}>
                      Discord
                    </span>
                  )}
                  {form.tools?.telegram && (
                    <span style={{ padding: '0.25rem 0.5rem', borderRadius: '0.25rem', background: 'var(--accent-primary)', color: '#fff', fontSize: '0.75rem' }}>
                      Telegram
                    </span>
                  )}
                  {form.tools?.fetch && (
                    <span style={{ padding: '0.25rem 0.5rem', borderRadius: '0.25rem', background: 'var(--accent-primary)', color: '#fff', fontSize: '0.75rem' }}>
                      Fetch
                    </span>
                  )}
                  {form.tools?.http_client && (
                    <span style={{ padding: '0.25rem 0.5rem', borderRadius: '0.25rem', background: 'var(--accent-primary)', color: '#fff', fontSize: '0.75rem' }}>
                      HTTP Client
                    </span>
                  )}
                  {form.tools?.programmatic_sandbox && (
                    <span style={{ padding: '0.25rem 0.5rem', borderRadius: '0.25rem', background: 'var(--accent-primary)', color: '#fff', fontSize: '0.75rem' }}>
                      Sandbox
                    </span>
                  )}
                  {form.tools?.tts && (
                    <span style={{ padding: '0.25rem 0.5rem', borderRadius: '0.25rem', background: 'var(--accent-primary)', color: '#fff', fontSize: '0.75rem' }}>
                      TTS
                    </span>
                  )}
                  {form.tools?.stt && (
                    <span style={{ padding: '0.25rem 0.5rem', borderRadius: '0.25rem', background: 'var(--accent-primary)', color: '#fff', fontSize: '0.75rem' }}>
                      STT
                    </span>
                  )}
                  {form.tools?.read_image && (
                    <span style={{ padding: '0.25rem 0.5rem', borderRadius: '0.25rem', background: 'var(--accent-primary)', color: '#fff', fontSize: '0.75rem' }}>
                      Read Image (vision)
                    </span>
                  )}
                  {form.tools?.image_gen && (
                    <span style={{ padding: '0.25rem 0.5rem', borderRadius: '0.25rem', background: 'var(--accent-primary)', color: '#fff', fontSize: '0.75rem' }}>
                      Image Gen
                    </span>
                  )}
                  {form.tools?.mcp_servers && Object.keys(form.tools.mcp_servers).length > 0 && (
                    <span style={{ padding: '0.25rem 0.5rem', borderRadius: '0.25rem', background: 'var(--accent-primary)', color: '#fff', fontSize: '0.75rem' }}>
                      MCP ({Object.keys(form.tools.mcp_servers).length})
                    </span>
                  )}
                  {!form.tools?.brave_search && !form.tools?.discord &&
                    !form.tools?.telegram && !form.tools?.fetch && !form.tools?.http_client &&
                    !form.tools?.programmatic_sandbox && !form.tools?.tts && !form.tools?.stt &&
                    !form.tools?.read_image && !form.tools?.image_gen && (
                    <span style={{ fontSize: '0.8rem', color: 'var(--text-tertiary)' }}>
                      No tools enabled
                    </span>
                  )}
                </div>
              </div>

              {/* System Prompt Preview */}
              <div
                style={{
                  padding: '1rem',
                  border: '1px solid var(--border)',
                  borderRadius: '0.5rem',
                  background: 'var(--surface)',
                }}
              >
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
                  System Prompt
                </h4>
                <div
                  style={{
                    fontSize: '0.8rem',
                    color: 'var(--text-secondary)',
                    maxHeight: '150px',
                    overflow: 'auto',
                    whiteSpace: 'pre-wrap',
                    lineHeight: 1.5,
                  }}
                >
                  {form.system_prompt
                    ? form.system_prompt.slice(0, 500) + (form.system_prompt.length > 500 ? '...' : '')
                    : '(no prompt)'}
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
                  disabled={submitting}
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
                    display: 'flex',
                    alignItems: 'center',
                    gap: '0.5rem',
                  }}
                >
                  {submitting ? (
                    <>
                      Creating
                      <span className="thinking-dots">
                        <span>.</span>
                        <span>.</span>
                        <span>.</span>
                      </span>
                    </>
                  ) : (
                    'Create Agent'
                  )}
                </button>
              </div>
            </div>
          )}
        </div>
      </div>
    </>
  )
}
