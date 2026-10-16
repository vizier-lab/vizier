import { useState, useEffect, useRef, useMemo, type ReactNode } from 'react'
import { FaGear, FaCode, FaScrewdriverWrench, FaCircleCheck } from 'react-icons/fa6'
import TooltipLabel from './TooltipLabel'
import MarkdownEditor from './MarkdownEditor'
import Avatar from './avatar'
import AvatarCropModal from './AvatarCropModal'
import EmbeddingIndexerSection from './EmbeddingIndexerSection'
import { uploadFile } from '../services/vizier'
import ProviderModelPicker, { ModelCard } from './ProviderModelPicker'
import MediaKeyHint from './MediaKeyHint'
import { useCatalogueProviders, useModelInfo } from '../hooks/catalogueStore'
import type {
  AgentWarning,
  CreateAgentRequest,
  AgentDetail,
  EmbeddingToolSettings,
  IndexerConfig,
  ShellConfigData,
} from '../interfaces/types'
import {
  TTS_PROVIDER_MODELS,
  TTS_PROVIDER_VOICES,
  STT_PROVIDER_MODELS,
  IMAGE_GEN_PROVIDER_MODELS,
} from '../interfaces/types'
import ModelSelect from './ModelSelect'
import defaultPrompt from '../../../templates/agent.template.md?raw'

type FormTab = 'config' | 'tools' | 'prompt' | 'review'

/** A tab the edit page adds after the form's own (Sharing, Danger Zone). */
export interface ExtraTab {
  key: string
  label: string
  icon: typeof FaGear
  render: () => ReactNode
}

const TABS: { key: FormTab; label: string; icon: typeof FaGear }[] = [
  { key: 'config', label: 'Config', icon: FaGear },
  { key: 'tools', label: 'Tools', icon: FaScrewdriverWrench },
  { key: 'prompt', label: 'System Prompt', icon: FaCode },
  { key: 'review', label: 'Review', icon: FaCircleCheck },
]

interface AgentFormProps {
  mode: 'create' | 'edit'
  agentId?: string
  initialData?: AgentDetail
  onSubmit: (form: CreateAgentRequest) => Promise<void>
  onCancel: () => void
  /** Edit mode: tabs after Config, Tools and System Prompt. */
  extraTabs?: ExtraTab[]
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
  provider: '',
  model: '',
  system_prompt: defaultPrompt,
  thinking_depth: 100,
  checkpoint_threshold: 80,
  max_tokens: undefined,
  context_window: undefined,
  silent_read_initiative_chance: 0.0,
  tools: {
    shell: null,
    fetch: false,
    http_client: false,
    timeout: '30m',
    tts: false,
    tts_settings: {},
    stt: false,
    stt_settings: {},
    read_image: false,
    read_image_settings: {},
    image_gen: false,
    image_gen_settings: {},
  },
  prompt_timeout: '60m',
  dream_enabled: false,
  dream_schedule: '',
  dream_provider: '',
  dream_model: '',
  avatar_url: undefined,
  embedding: {
    provider: 'local',
    model: 'all_mini_lml6_v2',
  },
  indexer: { kind: 'sqlite' },
}

export default function AgentForm({
  mode,
  initialData,
  onSubmit,
  onCancel,
  extraTabs = [],
}: AgentFormProps) {
  const [activeTab, setActiveTab] = useState<string>('config')
  const tabs: { key: string; label: string; icon: typeof FaGear }[] =
    mode === 'create'
      ? TABS
      : [...TABS.filter((t) => t.key !== 'review'), ...extraTabs]
  const activeExtra = extraTabs.find((t) => t.key === activeTab)
  // Edit mode: what was last loaded or saved, to tell whether there are unsaved edits.
  const [baseline, setBaseline] = useState<string | null>(null)
  const [reloadKey, setReloadKey] = useState(0)
  const [maxReachedTab, setMaxReachedTab] = useState<number>(0)
  const [submitting, setSubmitting] = useState(false)
  const [form, setForm] = useState<CreateAgentRequest>(DEFAULT_FORM)
  const [cropFile, setCropFile] = useState<File | null>(null)
  const [avatarBlob, setAvatarBlob] = useState<Blob | null>(null)
  const [avatarPreview, setAvatarPreview] = useState<string | null>(null)
  const avatarInputRef = useRef<HTMLInputElement>(null)

  // Dream model toggle (UI-only state)
  const [useSameModel, setUseSameModel] = useState(true)
  const [advancedOpen, setAdvancedOpen] = useState(false)
  // Read-image uses a separate vision model (an existing agent's, or one the operator chose).
  const [visionOverride, setVisionOverride] = useState(false)

  const mainModel = useModelInfo(form.provider, form.model)
  const catalogueContext = mainModel ? (mainModel.limit.input ?? mainModel.limit.context) : null
  const effectiveContext = form.context_window || catalogueContext

  const providers = useCatalogueProviders()
  const providerOrigin = providers?.find((p) => p.id === form.provider)?.origin

  // The same rules as the server's save warnings (`catalogue::agent_warnings`), for the
  // Review step before anything is saved.
  const clientWarnings: AgentWarning[] = []
  if (form.model && mainModel !== undefined) {
    if (mainModel?.caps.tool_call === false)
      clientWarnings.push({
        code: 'no_tool_calling',
        message: `${form.model} can't call tools; the agent will only be able to answer in text`,
      })
    const explicitVision = visionOverride && !!form.tools?.read_image_settings?.provider && !!form.tools?.read_image_settings?.model
    if (form.tools?.read_image && !explicitVision && mainModel?.caps.image_input === false)
      clientWarnings.push({
        code: 'no_image_input',
        message: `${form.model} can't view images; configure a vision model for read-image`,
      })
    if (mainModel === null && providerOrigin === 'catalogue')
      clientWarnings.push({
        code: 'unknown_model',
        message: `${form.model} isn't in the catalogue for ${form.provider}; it will be used as typed`,
      })
  }

  const reviewRow = (label: string, value: ReactNode) => (
    <div style={{ display: 'flex', justifyContent: 'space-between', gap: '1rem' }}>
      <span style={{ fontSize: '0.8rem', color: 'var(--text-secondary)' }}>{label}</span>
      <span style={{ fontSize: '0.8rem', color: 'var(--text-primary)', textAlign: 'right' }}>{value}</span>
    </div>
  )

  const setReadImage = (next: { provider?: string; model?: string }) =>
    setForm((prev) => ({
      ...prev,
      tools: { ...prev.tools, read_image_settings: next },
    }))

  useEffect(() => {
    if (mode === 'edit' && initialData) {
      const d = initialData
      const loaded: CreateAgentRequest = {
        agent_id: d.agent_id,
        name: d.name,
        description: d.description || '',
        provider: d.provider,
        model: d.model,
        system_prompt: d.system_prompt || '',
        thinking_depth: d.thinking_depth,
        checkpoint_threshold: (d.checkpoint_threshold ?? 0.8) * 100,
        max_tokens: d.max_tokens,
        context_window: d.context_window,
        silent_read_initiative_chance:
          d.silent_read_initiative_chance ?? 0.0,
        tools: {
          shell: d.shell || null,
          fetch: d.fetch,
          http_client: d.http_client,
          timeout: d.tools_timeout || '30m',
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
        dream_enabled: d.dream_enabled,
        dream_schedule: d.dream_schedule || '',
        dream_provider: d.dream_provider || '',
        dream_model: d.dream_model || '',
        avatar_url: d.avatar_url,
        embedding: d.embedding || {
          provider: 'local',
          model: 'all_mini_lml6_v2',
        },
        indexer: d.indexer || { kind: 'sqlite' },
      }
      const sameModel = !d.dream_provider && !d.dream_model
      setForm(loaded)
      setUseSameModel(sameModel)
      setVisionOverride(!!d.read_image_settings?.provider)
      setAdvancedOpen(d.context_window != null || d.max_tokens != null)
      setBaseline(JSON.stringify({ form: loaded, useSameModel: sameModel }))
    }
  }, [mode, initialData, reloadKey])

  const snapshot = useMemo(
    () => JSON.stringify({ form, useSameModel }),
    [form, useSameModel]
  )
  const dirty =
    mode === 'edit' && baseline !== null && (snapshot !== baseline || avatarBlob !== null)

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

  const goToNextTab = () => {
    const currentIndex = TABS.findIndex(t => t.key === activeTab)
    if (currentIndex < TABS.length - 1) {
      const nextIndex = currentIndex + 1
      setActiveTab(TABS[nextIndex].key)
      setMaxReachedTab(prev => Math.max(prev, nextIndex))
    }
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
    if (mode === 'create' && (!form.agent_id.trim() || !form.name.trim() || !form.provider || !form.model)) {
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
        checkpoint_threshold: (form.checkpoint_threshold ?? 80) / 100,
        dream_schedule: form.dream_enabled ? (form.dream_schedule || '0 2 * * *') : '',
        dream_provider: (!form.dream_enabled || useSameModel) ? null : form.dream_provider || null,
        dream_model: (!form.dream_enabled || useSameModel) ? null : form.dream_model || null,
      })
      if (mode === 'edit') {
        const saved = { ...form, avatar_url: avatarUrl }
        setForm(saved)
        setBaseline(JSON.stringify({ form: saved, useSameModel }))
        setAvatarBlob(null)
        if (avatarPreview) URL.revokeObjectURL(avatarPreview)
        setAvatarPreview(null)
      }
    } catch {
      // The caller reports the error; the edits stay so they can be saved again.
    } finally {
      setSubmitting(false)
    }
  }

  const avatarDisplayUrl = avatarPreview || form.avatar_url

  return (
    <>
      {/* Mobile tab nav */}
      <div className="flex md:hidden border-b border-[var(--border)] px-4 gap-2 py-2 overflow-x-auto">
        {tabs.map(({ key, label }, index) => {
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
          {tabs.map(({ key, label, icon: Icon }, index) => {
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
          style={{ padding: '24px', height: '100%', display: 'flex', flexDirection: 'column' }}
        >
          <div style={{ flex: 1 }}>
          {activeExtra?.render()}
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
                  <section style={fieldStyle}>
                    <label style={labelStyle}>
                      <TooltipLabel
                        label="Model *"
                        tooltip="The provider and model that answer for this agent. Models that can't call tools are hidden unless you show all."
                      />
                    </label>
                    <ProviderModelPicker
                      value={{ provider: form.provider, model: form.model }}
                      onChange={(v) => setForm((prev) => ({ ...prev, provider: v.provider, model: v.model }))}
                      preselectDefault={mode === 'create'}
                    />
                  </section>
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
                <section style={{ ...fieldStyle, maxWidth: '20rem' }}>
                  <label style={labelStyle}>
                    <TooltipLabel
                      label="Checkpoint Threshold (%)"
                      tooltip="Context window usage that triggers a checkpoint (default: 80%)."
                    />
                  </label>
                  <input
                    style={inputStyle}
                    type="number"
                    min={10}
                    max={100}
                    value={form.checkpoint_threshold || 80}
                    onChange={(e) =>
                      updateField(
                        'checkpoint_threshold',
                        Math.min(100, Math.max(10, parseInt(e.target.value) || 80))
                      )
                    }
                  />
                  <span style={{ fontSize: '0.75rem', color: 'var(--text-tertiary)' }}>
                    {effectiveContext
                      ? `≈ ${Math.round((effectiveContext * (form.checkpoint_threshold || 80)) / 100).toLocaleString()} tokens of ${effectiveContext.toLocaleString()}`
                      : 'Context window unknown for this model'}
                  </span>
                </section>
              </div>

              {/* Advanced: optional overrides and rarely changed settings (FR-032) */}
              <div>
                <button
                  type="button"
                  onClick={() => setAdvancedOpen((o) => !o)}
                  style={{
                    display: 'flex',
                    alignItems: 'center',
                    gap: '0.4rem',
                    width: '100%',
                    fontSize: '0.85rem',
                    fontWeight: 600,
                    color: 'var(--text-primary)',
                    paddingBottom: '0.5rem',
                    marginBottom: advancedOpen ? '0.75rem' : 0,
                    borderBottom: '1px solid var(--border)',
                    background: 'none',
                    borderTop: 'none',
                    borderLeft: 'none',
                    borderRight: 'none',
                    cursor: 'pointer',
                    textAlign: 'left',
                  }}
                >
                  <span style={{ display: 'inline-block', transform: advancedOpen ? 'rotate(90deg)' : undefined, transition: 'transform 0.15s' }}>▸</span>
                  Advanced
                </button>
                {advancedOpen && (
                  <div style={{ display: 'flex', flexDirection: 'column', gap: '1rem' }}>
                    <div style={{ display: 'flex', gap: '0.75rem', flexWrap: 'wrap' }}>
                      <section style={{ ...fieldStyle, flex: '1 1 10rem' }}>
                        <label style={labelStyle}>
                          <TooltipLabel
                            label="Context Window"
                            tooltip="Override the model's context window in tokens. Leave empty to use the catalogue's value."
                          />
                        </label>
                        <input
                          style={inputStyle}
                          type="number"
                          min={1}
                          placeholder={catalogueContext ? `${catalogueContext.toLocaleString()} · from catalogue` : 'Auto-detect'}
                          value={form.context_window ?? ''}
                          onChange={(e) =>
                            updateField('context_window', e.target.value ? parseInt(e.target.value) : undefined)
                          }
                        />
                      </section>
                      <section style={{ ...fieldStyle, flex: '1 1 10rem' }}>
                        <label style={labelStyle}>
                          <TooltipLabel
                            label="Max Output"
                            tooltip="Cap on output tokens per completion. Leave empty to use the model's own limit."
                          />
                        </label>
                        <input
                          style={inputStyle}
                          type="number"
                          min={1}
                          placeholder={mainModel?.limit.output ? `${mainModel.limit.output.toLocaleString()} · from catalogue` : 'Model default'}
                          value={form.max_tokens ?? ''}
                          onChange={(e) =>
                            updateField('max_tokens', e.target.value ? parseInt(e.target.value) : undefined)
                          }
                        />
                      </section>
                    </div>
                    <div style={{ display: 'flex', gap: '0.75rem', flexWrap: 'wrap' }}>
                      <section style={{ ...fieldStyle, flex: '1 1 10rem' }}>
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
                          value={form.thinking_depth || 10}
                          onChange={(e) => updateField('thinking_depth', parseInt(e.target.value) || 10)}
                        />
                      </section>
                      <section style={{ ...fieldStyle, flex: '1 1 10rem' }}>
                        <label style={labelStyle}>
                          <TooltipLabel
                            label="Prompt Timeout"
                            tooltip="Maximum wall-clock duration for a single request."
                          />
                        </label>
                        <input
                          style={inputStyle}
                          placeholder="60m"
                          value={form.prompt_timeout || ''}
                          onChange={(e) => updateField('prompt_timeout', e.target.value)}
                        />
                      </section>
                    </div>
                    <section style={fieldStyle}>
                      <label style={labelStyle}>
                        <TooltipLabel
                          label="Silent Read Chance"
                          tooltip="Probability (0.0-1.0) that the agent proactively reads silent/channel messages."
                        />
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
                  </div>
                )}
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
                        <section style={fieldStyle}>
                          <label style={labelStyle}>
                            <TooltipLabel
                              label="Dream model *"
                              tooltip="Model to use for dreaming. It needs tool calling, so models without it are hidden."
                            />
                          </label>
                          <ProviderModelPicker
                            value={{ provider: form.dream_provider || '', model: form.dream_model || '' }}
                            onChange={(v) =>
                              setForm((prev) => ({ ...prev, dream_provider: v.provider, dream_model: v.model }))
                            }
                            filter={{ toolCallOnly: true }}
                            preselectDefault
                            showCard={false}
                            invalid
                          />
                        </section>
                      )}
                    </>
                  )}
                </div>
              </div>

              <EmbeddingIndexerSection
                embedding={form.embedding || { provider: 'local', model: 'all_mini_lml6_v2' }}
                indexer={form.indexer || { kind: 'sqlite' }}
                onEmbeddingChange={(next) => updateField('embedding', next)}
                onIndexerChange={(next) => updateField('indexer', next)}
                inputStyle={inputStyle}
                labelStyle={labelStyle}
                fieldStyle={fieldStyle}
              />

              {/* Actions */}
              {mode === 'create' && (
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
                    onClick={goToNextTab}
                    disabled={
                      !form.agent_id.trim() || !form.name.trim() || !form.provider || !form.model
                    }
                    style={{
                      padding: '0.6rem 1.5rem',
                      borderRadius: '0.375rem',
                      border: 'none',
                      background:
                        !form.agent_id.trim() || !form.name.trim() || !form.provider || !form.model
                          ? 'var(--border)'
                          : 'var(--accent-primary)',
                      color: '#fff',
                      cursor:
                        !form.agent_id.trim() || !form.name.trim() || !form.provider || !form.model
                          ? 'not-allowed'
                          : 'pointer',
                      fontSize: '0.85rem',
                      fontWeight: 500,
                    }}
                  >
                    Next
                  </button>
              </div>
              )}
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
                    placeholder="30m"
                      value={form.tools?.timeout || ''}
                    onChange={(e) =>
                      updateToolField('timeout', e.target.value)
                    }
                  />
                </section>
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
                    <div style={{ paddingLeft: '1.5rem', marginBottom: '0.5rem' }}>
                      <MediaKeyHint provider={form.tools?.tts_settings?.provider || 'openai'} />
                    </div>
                  )}
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
                            'openai'
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
                          <option value="openai">OpenAI</option>
                          <option value="openrouter">
                            OpenRouter
                          </option>
                          <option value="elevenlabs">
                            ElevenLabs
                          </option>
                          <option value="xai">xAI</option>
                          <option value="hyperbolic">Hyperbolic</option>
                          <option value="kokoro">Kokoro (Offline)</option>
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
                        <ModelSelect
                          options={TTS_PROVIDER_MODELS[(form.tools?.tts_settings?.provider || 'openai') as import('../interfaces/types').TtsProvider] || []}
                          value={form.tools?.tts_settings?.model || ''}
                          onChange={(value) =>
                            setForm((prev) => ({
                              ...prev,
                              tools: {
                                ...prev.tools!,
                                tts_settings: {
                                  ...prev.tools?.tts_settings,
                                  model: value || undefined,
                                },
                              },
                            }))
                          }
                          placeholder="Model name"
                          style={inputStyle}
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
                        <ModelSelect
                          options={TTS_PROVIDER_VOICES[(form.tools?.tts_settings?.provider || 'openai') as import('../interfaces/types').TtsProvider] || []}
                          value={form.tools?.tts_settings?.voice || ''}
                          onChange={(value) =>
                            setForm((prev) => ({
                              ...prev,
                              tools: {
                                ...prev.tools!,
                                tts_settings: {
                                  ...prev.tools?.tts_settings,
                                  voice: value || undefined,
                                },
                              },
                            }))
                          }
                          placeholder="Select or enter voice"
                          style={inputStyle}
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
                    <div style={{ paddingLeft: '1.5rem', marginBottom: '0.5rem' }}>
                      <MediaKeyHint provider={form.tools?.stt_settings?.provider || 'openai'} />
                    </div>
                  )}
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
                            'openai'
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
                          <option value="openai">OpenAI Whisper</option>
                          <option value="elevenlabs">
                            ElevenLabs
                          </option>
                          <option value="whisper">Local Whisper</option>
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
                        <ModelSelect
                          options={STT_PROVIDER_MODELS[(form.tools?.stt_settings?.provider || 'openai') as import('../interfaces/types').SttProvider] || []}
                          value={form.tools?.stt_settings?.model || ''}
                          onChange={(value) =>
                            setForm((prev) => ({
                              ...prev,
                              tools: {
                                ...prev.tools!,
                                stt_settings: {
                                  ...prev.tools?.stt_settings,
                                  model: value || undefined,
                                },
                              },
                            }))
                          }
                          placeholder="Select or enter model"
                          style={inputStyle}
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

              {/* Read Image: from the main model's capabilities (FR-033) */}
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
                  Read Image
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
                      marginBottom: form.tools?.read_image ? '0.75rem' : 0,
                    }}
                  >
                    <input
                      type="checkbox"
                      checked={form.tools?.read_image ?? false}
                      onChange={(e) => updateTool('read_image', e.target.checked)}
                    />
                    Let the agent read images
                  </label>
                  {form.tools?.read_image && (
                    <div style={{ display: 'flex', flexDirection: 'column', gap: '0.6rem', paddingLeft: '1.5rem' }}>
                      {visionOverride ? (
                        <>
                          <div style={{ display: 'flex', alignItems: 'center', gap: '0.5rem', fontSize: '0.8rem', color: 'var(--text-secondary)' }}>
                            <span style={{ flex: 1 }}>
                              Images are described by a separate vision model
                              {mainModel?.caps.image_input !== false && ' (optional: the main model can see images itself)'}.
                            </span>
                            <button
                              type="button"
                              className="btn btn-ghost"
                              style={{ fontSize: '0.75rem' }}
                              onClick={() => {
                                setVisionOverride(false)
                                setReadImage({ provider: undefined, model: undefined })
                              }}
                            >
                              Remove
                            </button>
                          </div>
                          <ProviderModelPicker
                            value={{
                              provider: form.tools?.read_image_settings?.provider || '',
                              model: form.tools?.read_image_settings?.model || '',
                            }}
                            onChange={(v) => setReadImage({ provider: v.provider || undefined, model: v.model || undefined })}
                            filter={{ imageInputOnly: true }}
                            preselectDefault
                            showCard={false}
                          />
                        </>
                      ) : mainModel?.caps.image_input === false ? (
                        <>
                          <div style={{ fontSize: '0.8rem', color: 'var(--warning, #d97706)' }}>
                            ⚠ {mainModel.name} can't see images. Choose a vision model to describe them, or images reach the
                            agent only as a note.
                          </div>
                          <button
                            type="button"
                            className="btn btn-ghost"
                            style={{ alignSelf: 'flex-start', fontSize: '0.8rem' }}
                            onClick={() => setVisionOverride(true)}
                          >
                            Choose a vision model
                          </button>
                        </>
                      ) : (
                        <div style={{ fontSize: '0.8rem', color: 'var(--text-secondary)' }}>
                          ✓ {mainModel?.name ?? (form.model || 'The main model')}{' '}
                          {mainModel ? 'can see images.' : 'is not in the catalogue; it is assumed to see images.'}{' '}
                          They are sent to it directly.
                          <button
                            type="button"
                            className="btn btn-ghost"
                            style={{ marginLeft: '0.5rem', fontSize: '0.75rem' }}
                            onClick={() => setVisionOverride(true)}
                          >
                            Use a separate vision model
                          </button>
                        </div>
                      )}
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
                    <div style={{ paddingLeft: '1.5rem', marginBottom: '0.5rem' }}>
                      <MediaKeyHint provider={form.tools?.image_gen_settings?.provider || 'openai'} />
                    </div>
                  )}
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
                        <ModelSelect
                          options={IMAGE_GEN_PROVIDER_MODELS[(form.tools?.image_gen_settings?.provider || 'openai') as import('../interfaces/types').ImageGenProvider] || []}
                          value={form.tools?.image_gen_settings?.model || ''}
                          onChange={(value) =>
                            setForm((prev) => ({
                              ...prev,
                              tools: {
                                ...prev.tools!,
                                image_gen_settings: {
                                  ...prev.tools?.image_gen_settings,
                                  model: value || undefined,
                                },
                              },
                            }))
                          }
                          placeholder="Select or enter model"
                          style={inputStyle}
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

              <p style={{ fontSize: '0.8rem', color: 'var(--text-tertiary)', margin: 0 }}>
                Bots, addons such as Brave Search, and MCP servers are managed under
                Integrations in the sidebar.
              </p>

              {/* Actions */}
              {mode === 'create' && (
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
              </div>
              )}
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
              {mode === 'create' && (
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
              </div>
              )}
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
                  {reviewRow('Model', <span style={{ fontFamily: 'monospace' }}>{form.provider} / {form.model}</span>)}
                  {form.context_window != null && reviewRow('Context window (override)', form.context_window.toLocaleString())}
                  {form.max_tokens != null && reviewRow('Max output (override)', form.max_tokens.toLocaleString())}
                  {reviewRow('Thinking depth', form.thinking_depth)}
                  {reviewRow('Checkpoint threshold', `${form.checkpoint_threshold}%`)}
                  {form.dream_enabled &&
                    reviewRow(
                      'Dreaming',
                      useSameModel ? 'main model' : `${form.dream_provider} / ${form.dream_model}`
                    )}
                  {form.tools?.read_image &&
                    reviewRow(
                      'Read image',
                      visionOverride && form.tools?.read_image_settings?.provider
                        ? `vision model ${form.tools.read_image_settings.provider} / ${form.tools.read_image_settings.model || '?'}`
                        : 'sent to the main model'
                    )}
                  {mainModel ? (
                    <ModelCard model={mainModel} checkpointThreshold={form.checkpoint_threshold || 80} />
                  ) : (
                    <div style={{ fontSize: '0.8rem', color: 'var(--text-tertiary)' }}>
                      No catalogue card for this model.
                    </div>
                  )}
                  {clientWarnings.map((w) => (
                    <div key={w.code} style={{ fontSize: '0.8rem', color: 'var(--warning, #d97706)' }}>
                      ⚠ {w.message}
                    </div>
                  ))}
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
                  {!form.tools?.fetch && !form.tools?.http_client &&
                    !form.tools?.tts && !form.tools?.stt &&
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

          {/* Edit mode: one save for every tab, shown while there are unsaved edits. */}
          {dirty && !activeExtra && (
            <div
              style={{
                position: 'sticky',
                bottom: 0,
                marginTop: '1.5rem',
                padding: '0.75rem 1rem',
                display: 'flex',
                alignItems: 'center',
                gap: '0.75rem',
                border: '1px solid var(--border)',
                borderRadius: '0.5rem',
                background: 'var(--surface)',
                boxShadow: '0 -4px 12px rgba(0,0,0,0.08)',
                maxWidth: '720px',
              }}
            >
              <span style={{ fontSize: '0.85rem', color: 'var(--text-secondary)', flex: 1 }}>
                You have unsaved changes
              </span>
              <button
                className="btn btn-ghost"
                onClick={() => {
                  setAvatarBlob(null)
                  if (avatarPreview) URL.revokeObjectURL(avatarPreview)
                  setAvatarPreview(null)
                  setReloadKey((k) => k + 1)
                }}
                style={{ fontSize: '0.85rem' }}
              >
                Discard
              </button>
              <button
                onClick={handleSubmit}
                disabled={submitting || !form.name.trim()}
                style={{
                  padding: '0.5rem 1.25rem',
                  borderRadius: '0.375rem',
                  border: 'none',
                  background: submitting ? 'var(--border)' : 'var(--accent-primary)',
                  color: '#fff',
                  cursor: submitting ? 'not-allowed' : 'pointer',
                  fontSize: '0.85rem',
                  fontWeight: 500,
                }}
              >
                {submitting ? 'Saving...' : 'Save changes'}
              </button>
            </div>
          )}
        </div>
      </div>
    </>
  )
}
