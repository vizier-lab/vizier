import { useState, useEffect, useRef } from 'react'
import { useParams, useNavigate } from 'react-router'
import {
    FaGear,
    FaFolder,
    FaTriangleExclamation,
    FaCode,
} from 'react-icons/fa6'
import {
    getAgentDetail,
    updateAgent,
    getMcpServers,
    getAgentDocument,
    updateAgentDocument,
    getIdentityDocument,
    updateIdentityDocument,
    getHeartbeatDocument,
    updateHeartbeatDocument,
    deleteAgent,
    uploadFile,
} from '../services/vizier'
import TooltipLabel from '../components/TooltipLabel'
import MarkdownEditor from '../components/MarkdownEditor'
import Avatar from '../components/avatar'
import AvatarCropModal from '../components/AvatarCropModal'
import { useToastStore } from '../hooks/toastStore'
import { useAgentStore } from '../hooks/agentStore'
import type {
    CreateAgentRequest,
    AgentDetail,
    GlobalConfigEntry,
} from '../interfaces/types'

function getErrorMessage(err: unknown): string {
    if (err && typeof err === 'object' && 'response' in err) {
        const resp = (err as { response?: { data?: { message?: string } } })
            .response
        return resp?.data?.message || 'An error occurred'
    }
    return 'An error occurred'
}

type SettingsTab = 'config' | 'prompt' | 'documents' | 'danger'
type DocumentType = 'agent' | 'identity' | 'heartbeat'

const TABS: { key: SettingsTab; label: string; icon: typeof FaGear }[] = [
    { key: 'config', label: 'Config', icon: FaGear },
    { key: 'prompt', label: 'System Prompt', icon: FaCode },
    { key: 'documents', label: 'Documents', icon: FaFolder },
    { key: 'danger', label: 'Danger Zone', icon: FaTriangleExclamation },
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
    })

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
            } catch {
                addToast('error', 'Failed to load agent config')
                navigate('/')
            } finally {
                setLoading(false)
            }
        }
        load()
    }, [agentId])

    // ── Load MCP servers ──
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

    const toggleMcpServer = (name: string) => {
        setForm((prev) => {
            const current = prev.tools?.mcp_servers ?? []
            const next = current.includes(name)
                ? current.filter((s) => s !== name)
                : [...current, name]
            return { ...prev, tools: { ...prev.tools, mcp_servers: next } }
        })
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

            await updateAgent(agentId, { ...form, avatar_url: avatarUrl })
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
        { key: 'agent', label: 'AGENT.md' },
        { key: 'identity', label: 'IDENTITY.md' },
        { key: 'heartbeat', label: 'HEARTBEAT.md' },
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
                    style={{ padding: '24px' }}
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

                                    {/* Brave Search */}
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

                                    {/* Programmatic Sandbox */}
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

                                    {/* Tool Timeout + MCP */}
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
                                    height: 'calc(100vh - 220px)',
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
                                    Also delete workspace files (AGENT.md,
                                    IDENTITY.md, etc.)
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
