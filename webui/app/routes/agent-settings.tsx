import { useState, useEffect, useRef } from 'react'
import { useParams, useNavigate } from 'react-router'
import {
  FaGear,
  FaFolder,
  FaTriangleExclamation,
  FaCode,
  FaUserGroup,
  FaScrewdriverWrench,
} from 'react-icons/fa6'
import {
  getAgentDetail,
  updateAgent,
  getAgentDocument,
  updateAgentDocument,
  getIdentityDocument,
  updateIdentityDocument,
  getHeartbeatDocument,
  updateHeartbeatDocument,
  deleteAgent,
  uploadFile,
  getAgentSharing,
  updateAgentSharing,
  listUsers,
} from '../services/vizier'
import TooltipLabel from '../components/TooltipLabel'
import MarkdownEditor from '../components/MarkdownEditor'
import Avatar from '../components/avatar'
import AvatarCropModal from '../components/AvatarCropModal'
import EmbeddingIndexerSection from '../components/EmbeddingIndexerSection'
import { useToastStore } from '../hooks/toastStore'
import { useAgentStore } from '../hooks/agentStore'
import type {
  CreateAgentRequest,
  AgentDetail,
  User,
  ChatProvider,
  EmbeddingToolSettings,
  IndexerConfig,
  McpServerConfig,
  ShellConfigData,
} from '../interfaces/types'
import {
  CHAT_PROVIDERS,
  CHAT_PROVIDER_MODELS,
  TTS_PROVIDER_MODELS,
  TTS_PROVIDER_VOICES,
  STT_PROVIDER_MODELS,
  IMAGE_GEN_PROVIDER_MODELS,
} from '../interfaces/types'
import ModelSelect from '../components/ModelSelect'

function getErrorMessage(err: unknown): string {
  if (err && typeof err === 'object' && 'response' in err) {
    const resp = (err as { response?: { data?: { message?: string } } })
      .response
    return resp?.data?.message || 'An error occurred'
  }
  return 'An error occurred'
}

type SettingsTab = 'config' | 'prompt' | 'documents' | 'tools' | 'sharing' | 'danger'
type DocumentType = 'agent' | 'identity' | 'heartbeat'

const TABS: { key: SettingsTab; label: string; icon: typeof FaGear }[] = [
  { key: 'config', label: 'Config', icon: FaGear },
  { key: 'prompt', label: 'System Prompt', icon: FaCode },
  { key: 'documents', label: 'Documents', icon: FaFolder },
  { key: 'tools', label: 'Tools', icon: FaScrewdriverWrench },
  { key: 'sharing', label: 'Sharing', icon: FaUserGroup },
  { key: 'danger', label: 'Danger Zone', icon: FaTriangleExclamation },
]

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

export default function AgentSettings() {
  const { agentId } = useParams()
  const navigate = useNavigate()
  const addToast = useToastStore((s) => s.addToast)
  const loadAgents = useAgentStore((s) => s.loadAgents)

  const [activeTab, setActiveTab] = useState<SettingsTab>('config')

  // ── Config state ──
  const [loading, setLoading] = useState(true)
  const [saving, setSaving] = useState(false)
  const [form, setForm] = useState<CreateAgentRequest>({
    agent_id: '',
    name: '',
    description: '',
    provider: 'ollama',
    model: '',
    system_prompt: '',
    thinking_depth: 100,
    checkpoint_threshold: 80,
    max_tokens: 100000,
    context_window: undefined,
    silent_read_initiative_chance: 0.0,
    tools: {
      shell: null,
      brave_search: false,
      brave_search_settings: {},
      discord: false,
      telegram: false,
      fetch: false,
      http_client: false,
      timeout: '30m',
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
    prompt_timeout: '60m',
    heartbeat_interval: '30m',
    dream_enabled: false,
    dream_schedule: '',
    dream_provider: '',
    dream_model: '',
    embedding: {
      provider: 'local',
      model: 'all_mini_lml6_v2',
    },
    indexer: { kind: 'sqlite' },
  })

  // ── Dream model toggle (UI-only state) ──
  const [useSameModel, setUseSameModel] = useState(true)

  // ── Documents state ──
  const [activeDoc, setActiveDoc] = useState<DocumentType>('agent')
  const [docContent, setDocContent] = useState('')
  const [docOriginal, setDocOriginal] = useState('')
  const [docLoading, setDocLoading] = useState(true)
  const [docSaving, setDocSaving] = useState(false)

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

  // ── MCP server form state ──
  const [mcpFormOpen, setMcpFormOpen] = useState(false)
  const [mcpFormKey, setMcpFormKey] = useState<string | null>(null)
  const [mcpForm, setMcpForm] = useState<{
    name: string
    config: McpServerConfig
  }>({
    name: '',
    config: { host: 'local', command: '', args: [], env: {}, uri: '' },
  })

  // ── Avatar state ──
  const [cropFile, setCropFile] = useState<File | null>(null)
  const [avatarBlob, setAvatarBlob] = useState<Blob | null>(null)
  const [avatarPreview, setAvatarPreview] = useState<string | null>(null)
  const avatarInputRef = useRef<HTMLInputElement>(null)

  // ── Load agent config ──
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
          checkpoint_threshold: (d.checkpoint_threshold ?? 0.8) * 100,
          max_tokens: d.max_tokens,
          context_window: d.context_window,
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
            timeout: d.tools_timeout || '30m',
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
          embedding: d.embedding || {
            provider: 'local',
            model: 'all_mini_lml6_v2',
          },
          indexer: d.indexer || { kind: 'sqlite' },
        })
        setUseSameModel(!d.dream_provider && !d.dream_model)
      } catch {
        addToast('error', 'Failed to load agent config')
        navigate('/')
      } finally {
        setLoading(false)
      }
    }
    load()
  }, [agentId])

  // ── Load documents ──
  useEffect(() => {
    if (activeTab !== 'documents' || !agentId) return
    const loadDoc = async () => {
      setDocLoading(true)
      try {
        let res: { data: { content: string } }
        switch (activeDoc) {
          case 'agent':
            res = await getAgentDocument(agentId)
            break
          case 'identity':
            res = await getIdentityDocument(agentId)
            break
          case 'heartbeat':
            res = await getHeartbeatDocument(agentId)
            break
        }
        const content = res.data?.content || ''
        setDocContent(content)
        setDocOriginal(content)
      } catch {
        addToast(
          'error',
          `Failed to load ${activeDoc.toUpperCase()}.md`
        )
        setDocContent('')
        setDocOriginal('')
      } finally {
        setDocLoading(false)
      }
    }
    loadDoc()
  }, [agentId, activeTab, activeDoc])

  // ── Load sharing data ──
  useEffect(() => {
    if (activeTab !== 'sharing' || !agentId) return
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
  }, [agentId, activeTab])

  // ── Config helpers ──
  const updateField = <K extends keyof CreateAgentRequest>(
    key: K,
    value: CreateAgentRequest[K]
  ) => {
    setForm((prev) => ({ ...prev, [key]: value }))
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

  const handleSaveConfig = async () => {
    if (!agentId || !form.name.trim()) {
      addToast('error', 'Name is required')
      return
    }
    setSaving(true)
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

      if (form.dream_enabled && !useSameModel) {
        if (!form.dream_provider || !form.dream_model) {
          addToast('error', 'Dream provider and model are required when not using the main model')
          return
        }
      }

      await updateAgent(agentId, {
        ...form,
        avatar_url: avatarUrl,
        checkpoint_threshold: (form.checkpoint_threshold ?? 80) / 100,
        dream_schedule: form.dream_enabled ? (form.dream_schedule || '0 2 * * *') : '',
        dream_provider: (!form.dream_enabled || useSameModel) ? null : form.dream_provider || null,
        dream_model: (!form.dream_enabled || useSameModel) ? null : form.dream_model || null,
      })
      setForm((prev) => ({ ...prev, avatar_url: avatarUrl }))
      setAvatarBlob(null)
      if (avatarPreview) URL.revokeObjectURL(avatarPreview)
      setAvatarPreview(null)
      addToast('success', `Agent "${form.name}" updated`)
      loadAgents()
    } catch (err: unknown) {
      addToast('error', getErrorMessage(err) || 'Failed to update agent')
    } finally {
      setSaving(false)
    }
  }

  // ── Document helpers ──
  const handleSaveDoc = async () => {
    if (!agentId) return
    setDocSaving(true)
    try {
      switch (activeDoc) {
        case 'agent':
          await updateAgentDocument(agentId, docContent)
          break
        case 'identity':
          await updateIdentityDocument(agentId, docContent)
          break
        case 'heartbeat':
          await updateHeartbeatDocument(agentId, docContent)
          break
      }
      setDocOriginal(docContent)
      addToast('success', `${activeDoc.toUpperCase()}.md saved`)
    } catch (err: unknown) {
      addToast(
        'error',
        `Failed to save ${activeDoc.toUpperCase()}.md`,
        getErrorMessage(err)
      )
    } finally {
      setDocSaving(false)
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

  const docTabs: { key: DocumentType; label: string }[] = [
    { key: 'agent', label: 'SOUL' },
    { key: 'identity', label: 'IDENTITY' },
    { key: 'heartbeat', label: 'HEARTBEAT' },
  ]

  if (loading) {
    return (
      <>
        <div className="main-header">
          <h3 style={{ margin: 0 }}>Agent Config</h3>
        </div>
        <div className="main-body">
          <p style={{ color: 'var(--text-tertiary)' }}>
            Loading agent configuration...
          </p>
        </div>
      </>
    )
  }

  return (
    <>
      <div className="main-header">
        <h3 style={{ margin: 0 }}>Agent Config</h3>
      </div>

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

      <div className="main-body" style={{ display: 'flex', padding: 0 }}>
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
                    name={form.agent_id || 'agent'}
                    size="lg"
                    avatarUrl={
                      avatarPreview || form.avatar_url
                    }
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
                      {(avatarPreview ||
                        form.avatar_url) && (
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
                    </label>
                    <input
                      style={{
                        ...inputStyle,
                        opacity: 0.6,
                      }}
                      value={form.agent_id}
                      disabled
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
                        onChange={(e) =>
                          updateField(
                            'provider',
                            e.target.value
                          )
                        }
                      >
                        {CHAT_PROVIDERS.map((p) => (
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
                      <ModelSelect
                        options={CHAT_PROVIDER_MODELS[form.provider as ChatProvider] || []}
                        value={form.model}
                        onChange={(value) => updateField('model', value)}
                        placeholder="Select or enter model"
                        style={inputStyle}
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
                          label="Checkpoint Threshold"
                          tooltip="Context window usage percentage to trigger a checkpoint (default: 80%)."
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
                          label="Context Window"
                          tooltip="Maximum context window size in tokens. Leave empty to auto-detect from provider."
                        />
                      </label>
                      <input
                        style={inputStyle}
                        type="number"
                        min={1}
                        placeholder="Auto-detect"
                        value={form.context_window ?? ''}
                        onChange={(e) =>
                          updateField(
                            'context_window',
                            e.target.value
                              ? parseInt(
                                e.target.value
                              )
                              : undefined
                          )
                        }
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
                      placeholder="60m"
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
                              {CHAT_PROVIDERS.map((p) => (
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
                            <ModelSelect
                              options={CHAT_PROVIDER_MODELS[form.dream_provider as ChatProvider] || []}
                              value={form.dream_model || ''}
                              onChange={(value) => updateField('dream_model', value)}
                              placeholder="Select or enter model"
                              style={{
                                ...inputStyle,
                                borderColor: !form.dream_model ? 'var(--error, #ef4444)' : undefined,
                              }}
                            />
                          </section>
                        </div>
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
                        tooltip="Bot token from @BotFather. Note: Bot must be an admin in chats to receive emoji reactions."
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

              {/* Save */}
              <div style={{ paddingTop: '0.5rem' }}>
                <button
                  onClick={handleSaveConfig}
                  disabled={saving}
                  style={{
                    padding: '0.6rem 1.5rem',
                    borderRadius: '0.375rem',
                    border: 'none',
                    background: saving
                      ? 'var(--border)'
                      : 'var(--accent-primary)',
                    color: '#fff',
                    cursor: saving
                      ? 'not-allowed'
                      : 'pointer',
                    fontSize: '0.85rem',
                    fontWeight: 500,
                  }}
                >
                  {saving ? 'Saving...' : 'Save Changes'}
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
                height: '100%'
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
                  height: '100%'
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
              <div>
                <button
                  onClick={handleSaveConfig}
                  disabled={saving}
                  style={{
                    padding: '0.6rem 1.5rem',
                    borderRadius: '0.375rem',
                    border: 'none',
                    background: saving
                      ? 'var(--border)'
                      : 'var(--accent-primary)',
                    color: '#fff',
                    cursor: saving
                      ? 'not-allowed'
                      : 'pointer',
                    fontSize: '0.85rem',
                    fontWeight: 500,
                  }}
                >
                  {saving ? 'Saving...' : 'Save Changes'}
                </button>
              </div>
            </div>
          )}

          {/* ─── Documents Tab ─── */}
          {activeTab === 'documents' && (
            <div>
              {/* Doc sub-tabs */}
              <div
                style={{
                  display: 'flex',
                  gap: '4px',
                  marginBottom: '1rem',
                  borderBottom: '1px solid var(--border)',
                  paddingBottom: '4px',
                }}
              >
                {docTabs.map((tab) => (
                  <button
                    key={tab.key}
                    onClick={() => setActiveDoc(tab.key)}
                    style={{
                      padding: '8px 16px',
                      borderRadius: '4px 4px 0 0',
                      border: 'none',
                      background:
                        activeDoc === tab.key
                          ? 'var(--surface)'
                          : 'transparent',
                      color:
                        activeDoc === tab.key
                          ? 'var(--text-primary)'
                          : 'var(--text-tertiary)',
                      cursor: 'pointer',
                      fontWeight:
                        activeDoc === tab.key
                          ? '600'
                          : '400',
                      transition: 'all 0.15s ease',
                    }}
                  >
                    {tab.label}
                  </button>
                ))}
                {docContent !== docOriginal && (
                  <div
                    style={{
                      marginLeft: 'auto',
                      display: 'flex',
                      gap: '8px',
                      alignItems: 'center',
                    }}
                  >
                    <span
                      style={{
                        fontSize: '12px',
                        color: 'var(--text-tertiary)',
                      }}
                    >
                      Unsaved changes
                    </span>
                    <button
                      className="btn btn-ghost"
                      onClick={() =>
                        setDocContent(docOriginal)
                      }
                    >
                      Reset
                    </button>
                    <button
                      className="btn btn-primary"
                      onClick={handleSaveDoc}
                      disabled={docSaving}
                    >
                      {docSaving ? 'Saving...' : 'Save'}
                    </button>
                  </div>
                )}
              </div>

              {/* Editor */}
              {docLoading ? (
                <p style={{ color: 'var(--text-tertiary)' }}>
                  Loading document...
                </p>
              ) : (
                <div style={{ height: 'calc(100vh - 220px)' }}>
                  <MarkdownEditor
                    key={activeDoc}
                    value={docContent}
                    onChange={setDocContent}
                    placeholder={`Enter ${activeDoc.toUpperCase()}.md content...`}
                    className="document-mdx-editor"
                  />
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

              {/* Tool Timeout */}
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
                            marginBottom: '0.25rem',
                            fontSize: '0.75rem',
                            color: 'var(--text-secondary)',
                          }}
                        >
                          API Key (optional, falls back to
                          global)
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
                                brave_search_settings: {
                                  ...prev.tools
                                    ?.brave_search_settings,
                                  api_key:
                                    e.target.value ||
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
                              ?.safesearch ?? true
                          }
                          onChange={(e) =>
                            setForm((prev) => ({
                              ...prev,
                              tools: {
                                ...prev.tools!,
                                brave_search_settings: {
                                  ...prev.tools
                                    ?.brave_search_settings,
                                  safesearch:
                                    e.target.checked,
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
                      <section style={fieldStyle}>
                        <label style={labelStyle}>
                          <TooltipLabel
                            label="Provider"
                            tooltip="Provider to use for vision-model image reading. Ignored in default attachment-injection mode."
                          />
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
                          {CHAT_PROVIDERS.map((p) => (
                            <option key={p} value={p}>{p}</option>
                          ))}
                        </select>
                      </section>
                      <section style={fieldStyle}>
                        <label style={labelStyle}>
                          <TooltipLabel
                            label="Model"
                            tooltip="Model identifier for the selected provider (e.g. gpt-4o, claude-sonnet-4-5, llama3.2-vision)."
                          />
                        </label>
                        <ModelSelect
                          options={CHAT_PROVIDER_MODELS[(form.tools?.read_image_settings?.provider || '') as ChatProvider] || []}
                          value={form.tools?.read_image_settings?.model || ''}
                          onChange={(value) =>
                            setForm((prev) => ({
                              ...prev,
                              tools: {
                                ...prev.tools!,
                                read_image_settings: {
                                  ...prev.tools?.read_image_settings,
                                  model: value || undefined,
                                },
                              },
                            }))
                          }
                          placeholder="Select or enter model"
                          style={inputStyle}
                        />
                      </section>
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
                      <section style={fieldStyle}>
                        <label style={labelStyle}>
                          <TooltipLabel
                            label="Provider"
                            tooltip="Image generation provider."
                          />
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
                      </section>
                      <section style={fieldStyle}>
                        <label style={labelStyle}>
                          <TooltipLabel
                            label="Model (optional)"
                            tooltip="Provider default is used if blank (dall-e-3)."
                          />
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
                      </section>
                      <section style={fieldStyle}>
                        <label style={labelStyle}>
                          <TooltipLabel
                            label="Size"
                            tooltip="Output image dimensions."
                          />
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
                      </section>
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

              {/* Save */}
              <div style={{ paddingTop: '0.5rem' }}>
                <button
                  onClick={handleSaveConfig}
                  disabled={saving}
                  style={{
                    padding: '0.6rem 1.5rem',
                    borderRadius: '0.375rem',
                    border: 'none',
                    background: saving
                      ? 'var(--border)'
                      : 'var(--accent-primary)',
                    color: '#fff',
                    cursor: saving
                      ? 'not-allowed'
                      : 'pointer',
                    fontSize: '0.85rem',
                    fontWeight: 500,
                  }}
                >
                  {saving ? 'Saving...' : 'Save Changes'}
                </button>
              </div>
            </div>
          )}

          {/* ─── Sharing Tab ─── */}
          {activeTab === 'sharing' && (
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
          )}

          {/* ─── Danger Zone Tab ─── */}
          {activeTab === 'danger' && (
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
                  Also delete workspace files (SOUL,
                  IDENTITY, etc.)
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
          )}
        </div>
      </div>
    </>
  )
}
