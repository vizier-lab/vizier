import { useEffect, useState } from 'react'
import type { FormEvent } from 'react'
import {
    listApiKeys,
    createApiKey,
    deleteApiKey,
    changePassword,
    getMcpServers,
    upsertMcpServers,
    getShellConfig,
    upsertShellConfig,
    listProviders,
    upsertProvider,
    deleteProvider,
} from '../services/vizier'
import { FaTrash, FaPlus, FaPen } from 'react-icons/fa6'
import { useToastStore } from '../hooks/toastStore'
import type {
    ApiKey,
    GlobalConfigEntry,
    McpServerConfig,
    ShellConfigData,
    ProviderResponse,
} from '../interfaces/types'
import { hasPermission } from '../utils/auth'
import UsersSection from '../components/UsersSection'
import RolesSection from '../components/RolesSection'

type Section = 'password' | 'api-keys' | 'providers' | 'mcp-servers' | 'shell' | 'users' | 'roles'

export default function Settings() {
    const addToast = useToastStore((s) => s.addToast)
    const [activeSection, setActiveSection] = useState<Section>('password')

    // Password change state
    const [currentPassword, setCurrentPassword] = useState('')
    const [newPassword, setNewPassword] = useState('')
    const [confirmPassword, setConfirmPassword] = useState('')
    const [passwordChanging, setPasswordChanging] = useState(false)
    const [passwordMessage, setPasswordMessage] = useState<{
        type: 'success' | 'error'
        text: string
    } | null>(null)

    // API Keys state
    const [apiKeys, setApiKeys] = useState<ApiKey[]>([])
    const [loadingKeys, setLoadingKeys] = useState(false)
    const [showCreateKeyForm, setShowCreateKeyForm] = useState(false)
    const [newKeyName, setNewKeyName] = useState('')
    const [newKeyExpiry, setNewKeyExpiry] = useState('90')
    const [createdKey, setCreatedKey] = useState<string | null>(null)
    const [creatingKey, setCreatingKey] = useState(false)

    // MCP Servers state
    const [mcpServers, setMcpServers] = useState<
        Record<string, McpServerConfig>
    >({})
    const [loadingMcp, setLoadingMcp] = useState(false)
    const [savingMcp, setSavingMcp] = useState(false)
    const [showAddMcp, setShowAddMcp] = useState(false)
    const [editingMcp, setEditingMcp] = useState<string | null>(null)
    const [mcpForm, setMcpForm] = useState<{
        name: string
        host: 'local' | 'http'
        command: string
        args: string
        uri: string
        env: string
    }>({
        name: '',
        host: 'local',
        command: '',
        args: '',
        uri: '',
        env: '',
    })

    // Shell config state
    const [shellConfig, setShellConfig] = useState<ShellConfigData | null>(null)
    const [loadingShell, setLoadingShell] = useState(false)
    const [savingShell, setSavingShell] = useState(false)
    const [shellForm, setShellForm] = useState<ShellConfigData>({
        environment: 'local',
        path: '.',
    })

    // Providers state
    const [providers, setProviders] = useState<ProviderResponse[]>([])
    const [loadingProviders, setLoadingProviders] = useState(false)
    const [editingProvider, setEditingProvider] = useState<string | null>(null)
    const [providerForm, setProviderForm] = useState({
        api_key: '',
        base_url: '',
    })

    useEffect(() => {
        if (activeSection === 'api-keys') loadApiKeys()
        if (activeSection === 'providers') loadProviders()
        if (activeSection === 'mcp-servers') loadMcpServers()
        if (activeSection === 'shell') loadShellConfig()
    }, [activeSection])

    const loadApiKeys = async () => {
        try {
            setLoadingKeys(true)
            const response = await listApiKeys()
            setApiKeys(response.data || [])
        } catch (error) {
            console.error('Failed to load API keys:', error)
        } finally {
            setLoadingKeys(false)
        }
    }

    const loadProviders = async () => {
        try {
            setLoadingProviders(true)
            const res = await listProviders()
            setProviders(res.data || [])
        } catch {
            addToast('error', 'Failed to load providers')
        } finally {
            setLoadingProviders(false)
        }
    }

    const handleSaveProvider = async (variant: string) => {
        try {
            await upsertProvider(variant, {
                api_key: providerForm.api_key || undefined,
                base_url: providerForm.base_url || undefined,
            })
            addToast('success', `${variant} provider saved`)
            setEditingProvider(null)
            setProviderForm({ api_key: '', base_url: '' })
            await loadProviders()
        } catch (err: any) {
            addToast(
                'error',
                err?.response?.data?.message || 'Failed to save provider'
            )
        }
    }

    const handleDeleteProvider = async (variant: string) => {
        if (!confirm(`Remove ${variant} provider?`)) return
        try {
            await deleteProvider(variant)
            addToast('success', `${variant} provider removed`)
            await loadProviders()
        } catch (err: any) {
            addToast(
                'error',
                err?.response?.data?.message || 'Failed to delete provider'
            )
        }
    }

    const loadMcpServers = async () => {
        try {
            setLoadingMcp(true)
            const response = await getMcpServers()
            if (response.data && response.data.value.type === 'McpServers') {
                setMcpServers(
                    response.data.value.data as Record<string, McpServerConfig>
                )
            } else {
                setMcpServers({})
            }
        } catch (error) {
            console.error('Failed to load MCP servers:', error)
        } finally {
            setLoadingMcp(false)
        }
    }

    const loadShellConfig = async () => {
        try {
            setLoadingShell(true)
            const response = await getShellConfig()
            if (response.data && response.data.value.type === 'Shell') {
                setShellConfig(response.data.value.data as ShellConfigData)
                setShellForm(response.data.value.data as ShellConfigData)
            }
        } catch (error) {
            console.error('Failed to load shell config:', error)
        } finally {
            setLoadingShell(false)
        }
    }

    const handlePasswordChange = async (e: FormEvent) => {
        e.preventDefault()
        setPasswordMessage(null)
        if (newPassword !== confirmPassword) {
            setPasswordMessage({
                type: 'error',
                text: 'New passwords do not match',
            })
            return
        }
        if (newPassword.length < 8) {
            setPasswordMessage({
                type: 'error',
                text: 'Password must be at least 8 characters',
            })
            return
        }
        setPasswordChanging(true)
        try {
            await changePassword(currentPassword, newPassword)
            setPasswordMessage({
                type: 'success',
                text: 'Password changed successfully',
            })
            setCurrentPassword('')
            setNewPassword('')
            setConfirmPassword('')
        } catch (error: any) {
            setPasswordMessage({
                type: 'error',
                text:
                    error.response?.data?.message ||
                    'Failed to change password',
            })
        } finally {
            setPasswordChanging(false)
        }
    }

    const handleCreateApiKey = async () => {
        if (!newKeyName.trim()) return
        setCreatingKey(true)
        try {
            const response = await createApiKey(
                newKeyName,
                parseInt(newKeyExpiry)
            )
            setCreatedKey(response.data.key)
            await loadApiKeys()
            setNewKeyName('')
            setNewKeyExpiry('90')
        } catch (error) {
            console.error('Failed to create API key:', error)
            alert('Failed to create API key')
        } finally {
            setCreatingKey(false)
        }
    }

    const handleDeleteApiKey = async (keyId: string) => {
        if (!confirm('Are you sure you want to delete this API key?')) return
        try {
            await deleteApiKey(keyId)
            await loadApiKeys()
        } catch (error) {
            console.error('Failed to delete API key:', error)
        }
    }

    const closeCreatedKeyModal = () => {
        setCreatedKey(null)
        setShowCreateKeyForm(false)
    }

    const parseEnvString = (
        envStr: string
    ): Record<string, string> | undefined => {
        if (!envStr.trim()) return undefined
        const env: Record<string, string> = {}
        for (const line of envStr.split('\n')) {
            const idx = line.indexOf('=')
            if (idx > 0) {
                env[line.slice(0, idx).trim()] = line.slice(idx + 1).trim()
            }
        }
        return Object.keys(env).length > 0 ? env : undefined
    }

    const envToString = (env?: Record<string, string>): string => {
        if (!env) return ''
        return Object.entries(env)
            .map(([k, v]) => `${k}=${v}`)
            .join('\n')
    }

    const handleSaveMcpServer = async () => {
        if (!mcpForm.name.trim()) {
            addToast('error', 'Server name is required')
            return
        }

        let config: McpServerConfig
        if (mcpForm.host === 'local') {
            if (!mcpForm.command.trim()) {
                addToast('error', 'Command is required for local MCP servers')
                return
            }
            config = {
                host: 'local',
                command: mcpForm.command,
                args: mcpForm.args
                    ? mcpForm.args
                          .split(' ')
                          .map((a) => a.trim())
                          .filter(Boolean)
                    : [],
                env: parseEnvString(mcpForm.env),
            }
        } else {
            if (!mcpForm.uri.trim()) {
                addToast('error', 'URI is required for HTTP MCP servers')
                return
            }
            config = { host: 'http', uri: mcpForm.uri }
        }

        const updated = { ...mcpServers }
        if (editingMcp) {
            delete updated[editingMcp]
        }
        updated[mcpForm.name] = config

        setSavingMcp(true)
        try {
            await upsertMcpServers(updated)
            setMcpServers(updated)
            setShowAddMcp(false)
            setEditingMcp(null)
            setMcpForm({
                name: '',
                host: 'local',
                command: '',
                args: '',
                uri: '',
                env: '',
            })
            addToast('success', 'MCP servers saved. Restart to take effect.')
        } catch (err: any) {
            addToast(
                'error',
                err?.response?.data?.message || 'Failed to save MCP servers'
            )
        } finally {
            setSavingMcp(false)
        }
    }

    const handleDeleteMcpServer = async (name: string) => {
        if (!confirm(`Remove MCP server "${name}"?`)) return
        const updated = { ...mcpServers }
        delete updated[name]

        setSavingMcp(true)
        try {
            await upsertMcpServers(updated)
            setMcpServers(updated)
            addToast('success', 'MCP server removed. Restart to take effect.')
        } catch (err: any) {
            addToast(
                'error',
                err?.response?.data?.message || 'Failed to delete MCP server'
            )
        } finally {
            setSavingMcp(false)
        }
    }

    const handleEditMcpServer = (name: string) => {
        const cfg = mcpServers[name]
        setEditingMcp(name)
        setMcpForm({
            name,
            host: cfg.host,
            command: cfg.command || '',
            args: cfg.args?.join(' ') || '',
            uri: cfg.uri || '',
            env: envToString(cfg.env),
        })
        setShowAddMcp(true)
    }

    const handleSaveShell = async () => {
        setSavingShell(true)
        try {
            await upsertShellConfig(shellForm)
            setShellConfig(shellForm)
            addToast('success', 'Shell config saved. Restart to take effect.')
        } catch (err: any) {
            addToast(
                'error',
                err?.response?.data?.message || 'Failed to save shell config'
            )
        } finally {
            setSavingShell(false)
        }
    }

    const pInputStyle: React.CSSProperties = {
        width: '100%',
        padding: '0.5rem 0.75rem',
        borderRadius: '0.375rem',
        border: '1px solid var(--border)',
        background: 'var(--surface)',
        color: 'var(--text-primary)',
        fontSize: '0.875rem',
        outline: 'none',
    }

    return (
        <>
            <div className="main-header">
                <div>
                    <h3 style={{ margin: 0 }}>Settings</h3>
                </div>
            </div>

            {/* Mobile section nav */}
            <div className="flex md:hidden border-b border-[var(--border)] px-4 gap-2 py-2 overflow-x-auto">
                {(
                    [
                        ...(hasPermission('settings:password') ? [['password', 'Password'] as const] : []),
                        ...(hasPermission('settings:api_keys') ? [['api-keys', 'API Keys'] as const] : []),
                        ...(hasPermission('settings:providers') ? [['providers', 'Providers'] as const] : []),
                        ...(hasPermission('settings:mcp_servers') ? [['mcp-servers', 'MCP Servers'] as const] : []),
                        ...(hasPermission('settings:shell') ? [['shell', 'Shell'] as const] : []),
                        ...(hasPermission('users:manage') ? [['users', 'Users'] as const] : []),
                        ...(hasPermission('roles:manage') ? [['roles', 'Roles'] as const] : []),
                    ] as const
                ).map(([key, label]) => (
                    <button
                        key={key}
                        onClick={() => setActiveSection(key)}
                        className={`px-3 py-1.5 text-sm font-medium rounded-t transition-colors whitespace-nowrap ${activeSection === key ? 'bg-[var(--surface)] text-[var(--text-primary)] border-b-2 border-[var(--accent-primary)]' : 'text-[var(--text-tertiary)]'}`}
                    >
                        {label}
                    </button>
                ))}
            </div>

            <div className="flex" style={{ height: 'calc(100vh - 60px)' }}>
                {/* Desktop sidebar nav */}
                <div
                    className="hidden md:block"
                    style={{
                        width: '200px',
                        borderRight: '1px solid var(--border)',
                        padding: '24px 16px',
                    }}
                >
                    {(
                        [
                            ...(hasPermission('settings:password') ? [['password', 'Password'] as const] : []),
                            ...(hasPermission('settings:api_keys') ? [['api-keys', 'API Keys'] as const] : []),
                            ...(hasPermission('settings:providers') ? [['providers', 'Providers'] as const] : []),
                            ...(hasPermission('settings:mcp_servers') ? [['mcp-servers', 'MCP Servers'] as const] : []),
                            ...(hasPermission('settings:shell') ? [['shell', 'Shell Config'] as const] : []),
                            ...(hasPermission('users:manage') ? [['users', 'Users'] as const] : []),
                            ...(hasPermission('roles:manage') ? [['roles', 'Roles'] as const] : []),
                        ] as const
                    ).map(([key, label]) => (
                        <div
                            key={key}
                            className={`nav-item ${activeSection === key ? 'active' : ''}`}
                            onClick={() => setActiveSection(key)}
                        >
                            {label}
                        </div>
                    ))}
                </div>

                {/* Content */}
                <div
                    className="flex-1 overflow-auto"
                    style={{ padding: '24px' }}
                >
                    {/* Password Section */}
                    {activeSection === 'password' && (
                        <div style={{ maxWidth: '600px' }}>
                            <h2 style={{ marginBottom: '1rem' }}>
                                Change Password
                            </h2>
                            <p
                                style={{
                                    color: 'var(--text-secondary)',
                                    marginBottom: '2rem',
                                    fontSize: '14px',
                                }}
                            >
                                Update your account password. Make sure to use a
                                strong password.
                            </p>
                            {passwordMessage && (
                                <div
                                    style={{
                                        padding: '12px',
                                        background:
                                            passwordMessage.type === 'success'
                                                ? '#e8f5e9'
                                                : '#ffebee',
                                        border: `1px solid ${passwordMessage.type === 'success' ? '#c8e6c9' : '#ffcdd2'}`,
                                        borderRadius: '4px',
                                        color:
                                            passwordMessage.type === 'success'
                                                ? '#2e7d32'
                                                : '#c62828',
                                        fontSize: '14px',
                                        marginBottom: '1rem',
                                    }}
                                >
                                    {passwordMessage.text}
                                </div>
                            )}
                            <form
                                onSubmit={handlePasswordChange}
                                style={{
                                    display: 'flex',
                                    flexDirection: 'column',
                                    gap: '1rem',
                                }}
                            >
                                <div className="input-group">
                                    <label htmlFor="current-password">
                                        Current Password
                                    </label>
                                    <input
                                        id="current-password"
                                        type="password"
                                        value={currentPassword}
                                        onChange={(e) =>
                                            setCurrentPassword(e.target.value)
                                        }
                                        required
                                        disabled={passwordChanging}
                                    />
                                </div>
                                <div className="input-group">
                                    <label htmlFor="new-password">
                                        New Password
                                    </label>
                                    <input
                                        id="new-password"
                                        type="password"
                                        value={newPassword}
                                        onChange={(e) =>
                                            setNewPassword(e.target.value)
                                        }
                                        required
                                        minLength={8}
                                        disabled={passwordChanging}
                                    />
                                </div>
                                <div className="input-group">
                                    <label htmlFor="confirm-password">
                                        Confirm New Password
                                    </label>
                                    <input
                                        id="confirm-password"
                                        type="password"
                                        value={confirmPassword}
                                        onChange={(e) =>
                                            setConfirmPassword(e.target.value)
                                        }
                                        required
                                        minLength={8}
                                        disabled={passwordChanging}
                                    />
                                </div>
                                <button
                                    type="submit"
                                    className="btn btn-primary"
                                    disabled={passwordChanging}
                                    style={{ alignSelf: 'flex-start' }}
                                >
                                    {passwordChanging
                                        ? 'Changing...'
                                        : 'Change Password'}
                                </button>
                            </form>
                        </div>
                    )}

                    {/* API Keys Section */}
                    {activeSection === 'api-keys' && (
                        <div>
                            <div
                                style={{
                                    display: 'flex',
                                    justifyContent: 'space-between',
                                    alignItems: 'center',
                                    marginBottom: '1.5rem',
                                }}
                            >
                                <div>
                                    <h2 style={{ marginBottom: '0.5rem' }}>
                                        API Keys
                                    </h2>
                                    <p
                                        style={{
                                            color: 'var(--text-secondary)',
                                            fontSize: '14px',
                                        }}
                                    >
                                        Manage API keys for programmatic access
                                    </p>
                                </div>
                                {!showCreateKeyForm && (
                                    <button
                                        className="btn btn-primary"
                                        onClick={() =>
                                            setShowCreateKeyForm(true)
                                        }
                                    >
                                        + Create API Key
                                    </button>
                                )}
                            </div>
                            {showCreateKeyForm && !createdKey && (
                                <div
                                    className="card"
                                    style={{ marginBottom: '1.5rem' }}
                                >
                                    <h3 style={{ marginBottom: '1rem' }}>
                                        Create New API Key
                                    </h3>
                                    <div
                                        style={{
                                            display: 'flex',
                                            flexDirection: 'column',
                                            gap: '1rem',
                                        }}
                                    >
                                        <div className="input-group">
                                            <label htmlFor="key-name">
                                                Key Name
                                            </label>
                                            <input
                                                id="key-name"
                                                type="text"
                                                value={newKeyName}
                                                onChange={(e) =>
                                                    setNewKeyName(
                                                        e.target.value
                                                    )
                                                }
                                                placeholder="My API Key"
                                                required
                                            />
                                        </div>
                                        <div className="input-group">
                                            <label htmlFor="key-expiry">
                                                Expires In (days)
                                            </label>
                                            <input
                                                id="key-expiry"
                                                type="number"
                                                value={newKeyExpiry}
                                                onChange={(e) =>
                                                    setNewKeyExpiry(
                                                        e.target.value
                                                    )
                                                }
                                                min="1"
                                                max="365"
                                            />
                                        </div>
                                        <div
                                            style={{
                                                display: 'flex',
                                                gap: '8px',
                                            }}
                                        >
                                            <button
                                                className="btn btn-primary"
                                                onClick={handleCreateApiKey}
                                                disabled={
                                                    !newKeyName.trim() ||
                                                    creatingKey
                                                }
                                            >
                                                {creatingKey
                                                    ? 'Creating...'
                                                    : 'Create'}
                                            </button>
                                            <button
                                                className="btn btn-secondary"
                                                onClick={() => {
                                                    setShowCreateKeyForm(false)
                                                    setNewKeyName('')
                                                    setNewKeyExpiry('90')
                                                }}
                                                disabled={creatingKey}
                                            >
                                                Cancel
                                            </button>
                                        </div>
                                    </div>
                                </div>
                            )}
                            {loadingKeys ? (
                                <p
                                    style={{
                                        textAlign: 'center',
                                        padding: '2rem',
                                        color: 'var(--text-tertiary)',
                                    }}
                                >
                                    Loading API keys...
                                </p>
                            ) : apiKeys.length === 0 ? (
                                <p
                                    style={{
                                        textAlign: 'center',
                                        padding: '2rem',
                                        color: 'var(--text-tertiary)',
                                    }}
                                >
                                    No API keys yet. Create one to get started.
                                </p>
                            ) : (
                                <div
                                    style={{
                                        display: 'flex',
                                        flexDirection: 'column',
                                        gap: '1rem',
                                    }}
                                >
                                    {apiKeys.map((key) => (
                                        <div
                                            key={key.id}
                                            className="card"
                                            style={{
                                                display: 'flex',
                                                justifyContent: 'space-between',
                                                alignItems: 'center',
                                            }}
                                        >
                                            <div>
                                                <h4
                                                    style={{
                                                        marginBottom: '0.25rem',
                                                    }}
                                                >
                                                    {key.name}
                                                </h4>
                                                <div
                                                    style={{
                                                        fontSize: '12px',
                                                        color: 'var(--text-tertiary)',
                                                    }}
                                                >
                                                    <div>
                                                        Created:{' '}
                                                        {new Date(
                                                            key.created_at
                                                        ).toLocaleString()}
                                                    </div>
                                                    {key.expires_at && (
                                                        <div>
                                                            Expires:{' '}
                                                            {new Date(
                                                                key.expires_at
                                                            ).toLocaleString()}
                                                        </div>
                                                    )}
                                                    {key.last_used_at && (
                                                        <div>
                                                            Last used:{' '}
                                                            {new Date(
                                                                key.last_used_at
                                                            ).toLocaleString()}
                                                        </div>
                                                    )}
                                                </div>
                                            </div>
                                            <button
                                                className="btn btn-ghost"
                                                onClick={() =>
                                                    handleDeleteApiKey(key.id)
                                                }
                                                style={{ color: '#c00' }}
                                            >
                                                <FaTrash size={16} />
                                                <span>Delete</span>
                                            </button>
                                        </div>
                                    ))}
                                </div>
                            )}
                        </div>
                    )}

                    {/* Providers Section */}
                    {activeSection === 'providers' && (
                        <div style={{ maxWidth: '600px' }}>
                            <h2 style={{ marginBottom: '1rem' }}>Providers</h2>
                            <p
                                style={{
                                    color: 'var(--text-secondary)',
                                    marginBottom: '2rem',
                                    fontSize: '14px',
                                }}
                            >
                                Configure AI provider credentials. These are
                                shared across all agents.
                            </p>

                            {loadingProviders ? (
                                <p style={{ color: 'var(--text-tertiary)' }}>
                                    Loading...
                                </p>
                            ) : (
                                <div
                                    style={{
                                        display: 'flex',
                                        flexDirection: 'column',
                                        gap: '1rem',
                                    }}
                                >
                                    {(() => {
                                        const ALL_VARIANTS = [
                                            'ollama',
                                            'openai',
                                            'anthropic',
                                            'deepseek',
                                            'openrouter',
                                            'gemini',
                                            'mimo',
                                            'llama_cpp',
                                        ]
                                        const configured = providers.map(
                                            (p) => p.variant
                                        )
                                        const unconfigured =
                                            ALL_VARIANTS.filter(
                                                (v) => !configured.includes(v)
                                            )
                                        const provInputStyle: React.CSSProperties =
                                            {
                                                width: '100%',
                                                padding: '0.5rem 0.75rem',
                                                borderRadius: '0.375rem',
                                                border: '1px solid var(--border)',
                                                background: 'var(--surface)',
                                                color: 'var(--text-primary)',
                                                fontSize: '0.875rem',
                                                outline: 'none',
                                            }

                                        return (
                                            <>
                                                {providers.map((p) => (
                                                    <div
                                                        key={p.variant}
                                                        style={{
                                                            padding: '1rem',
                                                            border: '1px solid var(--border)',
                                                            borderRadius:
                                                                '0.5rem',
                                                            display: 'flex',
                                                            flexDirection:
                                                                'column',
                                                            gap: '0.75rem',
                                                        }}
                                                    >
                                                        <div
                                                            style={{
                                                                display: 'flex',
                                                                justifyContent:
                                                                    'space-between',
                                                                alignItems:
                                                                    'center',
                                                            }}
                                                        >
                                                            <div>
                                                                <strong
                                                                    style={{
                                                                        textTransform:
                                                                            'capitalize',
                                                                    }}
                                                                >
                                                                    {p.variant}
                                                                </strong>
                                                                {p.base_url && (
                                                                    <span
                                                                        style={{
                                                                            color: 'var(--text-tertiary)',
                                                                            marginLeft:
                                                                                '0.75rem',
                                                                            fontSize:
                                                                                '0.8rem',
                                                                        }}
                                                                    >
                                                                        {
                                                                            p.base_url
                                                                        }
                                                                    </span>
                                                                )}
                                                            </div>
                                                            <div
                                                                style={{
                                                                    display:
                                                                        'flex',
                                                                    gap: '0.5rem',
                                                                }}
                                                            >
                                                                <button
                                                                    onClick={() => {
                                                                        setEditingProvider(
                                                                            p.variant
                                                                        )
                                                                        setProviderForm(
                                                                            {
                                                                                api_key:
                                                                                    '',
                                                                                base_url:
                                                                                    '',
                                                                            }
                                                                        )
                                                                    }}
                                                                    style={{
                                                                        background:
                                                                            'none',
                                                                        border: 'none',
                                                                        cursor: 'pointer',
                                                                        color: 'var(--text-secondary)',
                                                                        padding:
                                                                            '0.25rem',
                                                                    }}
                                                                >
                                                                    <FaPen
                                                                        size={
                                                                            16
                                                                        }
                                                                    />
                                                                </button>
                                                                <button
                                                                    onClick={() =>
                                                                        handleDeleteProvider(
                                                                            p.variant
                                                                        )
                                                                    }
                                                                    style={{
                                                                        background:
                                                                            'none',
                                                                        border: 'none',
                                                                        cursor: 'pointer',
                                                                        color: '#ef4444',
                                                                        padding:
                                                                            '0.25rem',
                                                                    }}
                                                                >
                                                                    <FaTrash
                                                                        size={
                                                                            16
                                                                        }
                                                                    />
                                                                </button>
                                                            </div>
                                                        </div>
                                                        <div
                                                            style={{
                                                                fontSize:
                                                                    '0.8rem',
                                                                color: 'var(--text-tertiary)',
                                                            }}
                                                        >
                                                            {p.has_api_key
                                                                ? 'API key configured'
                                                                : p.variant ===
                                                                    'ollama'
                                                                  ? 'No API key required'
                                                                  : 'No API key'}
                                                        </div>
                                                        {editingProvider ===
                                                            p.variant && (
                                                            <div
                                                                style={{
                                                                    display:
                                                                        'flex',
                                                                    flexDirection:
                                                                        'column',
                                                                    gap: '0.5rem',
                                                                    paddingTop:
                                                                        '0.5rem',
                                                                    borderTop:
                                                                        '1px solid var(--border)',
                                                                }}
                                                            >
                                                                {p.variant !==
                                                                    'ollama' && (
                                                                    <div>
                                                                        <label
                                                                            style={{
                                                                                display:
                                                                                    'block',
                                                                                marginBottom:
                                                                                    '0.25rem',
                                                                                fontSize:
                                                                                    '0.8rem',
                                                                                color: 'var(--text-secondary)',
                                                                            }}
                                                                        >
                                                                            API
                                                                            Key
                                                                        </label>
                                                                        <input
                                                                            style={
                                                                                provInputStyle
                                                                            }
                                                                            type="password"
                                                                            placeholder="sk-..."
                                                                            value={
                                                                                providerForm.api_key
                                                                            }
                                                                            onChange={(
                                                                                e
                                                                            ) =>
                                                                                setProviderForm(
                                                                                    {
                                                                                        ...providerForm,
                                                                                        api_key:
                                                                                            e
                                                                                                .target
                                                                                                .value,
                                                                                    }
                                                                                )
                                                                            }
                                                                        />
                                                                    </div>
                                                                )}
                                                                {(p.variant ===
                                                                    'ollama' ||
                                                                    p.variant ===
                                                                        'openai' ||
                                                                    p.variant ===
                                                                        'anthropic') && (
                                                                    <div>
                                                                        <label
                                                                            style={{
                                                                                display:
                                                                                    'block',
                                                                                marginBottom:
                                                                                    '0.25rem',
                                                                                fontSize:
                                                                                    '0.8rem',
                                                                                color: 'var(--text-secondary)',
                                                                            }}
                                                                        >
                                                                            Base
                                                                            URL
                                                                        </label>
                                                                        <input
                                                                            style={
                                                                                provInputStyle
                                                                            }
                                                                            placeholder={
                                                                                p.variant ===
                                                                                'ollama'
                                                                                    ? 'http://localhost:11434'
                                                                                    : p.variant ===
                                                                                        'anthropic'
                                                                                      ? 'https://api.anthropic.com'
                                                                                      : 'https://api.openai.com/v1'
                                                                            }
                                                                            value={
                                                                                providerForm.base_url
                                                                            }
                                                                            onChange={(
                                                                                e
                                                                            ) =>
                                                                                setProviderForm(
                                                                                    {
                                                                                        ...providerForm,
                                                                                        base_url:
                                                                                            e
                                                                                                .target
                                                                                                .value,
                                                                                    }
                                                                                )
                                                                            }
                                                                        />
                                                                    </div>
                                                                )}
                                                                <div
                                                                    style={{
                                                                        display:
                                                                            'flex',
                                                                        gap: '0.5rem',
                                                                    }}
                                                                >
                                                                    <button
                                                                        onClick={() =>
                                                                            setEditingProvider(
                                                                                null
                                                                            )
                                                                        }
                                                                        style={{
                                                                            padding:
                                                                                '0.4rem 1rem',
                                                                            borderRadius:
                                                                                '0.375rem',
                                                                            border: '1px solid var(--border)',
                                                                            background:
                                                                                'transparent',
                                                                            cursor: 'pointer',
                                                                            fontSize:
                                                                                '0.8rem',
                                                                        }}
                                                                    >
                                                                        Cancel
                                                                    </button>
                                                                    <button
                                                                        onClick={() =>
                                                                            handleSaveProvider(
                                                                                p.variant
                                                                            )
                                                                        }
                                                                        style={{
                                                                            padding:
                                                                                '0.4rem 1rem',
                                                                            borderRadius:
                                                                                '0.375rem',
                                                                            border: 'none',
                                                                            background:
                                                                                'var(--accent-primary)',
                                                                            color: '#fff',
                                                                            cursor: 'pointer',
                                                                            fontSize:
                                                                                '0.8rem',
                                                                        }}
                                                                    >
                                                                        Save
                                                                    </button>
                                                                </div>
                                                            </div>
                                                        )}
                                                    </div>
                                                ))}
                                                {unconfigured.map((variant) => (
                                                    <div
                                                        key={variant}
                                                        style={{
                                                            padding: '1rem',
                                                            border: '1px dashed var(--border)',
                                                            borderRadius:
                                                                '0.5rem',
                                                            display: 'flex',
                                                            flexDirection:
                                                                'column',
                                                            gap: '0.75rem',
                                                        }}
                                                    >
                                                        <div
                                                            style={{
                                                                display: 'flex',
                                                                justifyContent:
                                                                    'space-between',
                                                                alignItems:
                                                                    'center',
                                                            }}
                                                        >
                                                            <strong
                                                                style={{
                                                                    textTransform:
                                                                        'capitalize',
                                                                    color: 'var(--text-tertiary)',
                                                                }}
                                                            >
                                                                {variant}
                                                            </strong>
                                                            <button
                                                                onClick={() => {
                                                                    setEditingProvider(
                                                                        variant
                                                                    )
                                                                    setProviderForm(
                                                                        {
                                                                            api_key:
                                                                                '',
                                                                            base_url:
                                                                                '',
                                                                        }
                                                                    )
                                                                }}
                                                                style={{
                                                                    background:
                                                                        'none',
                                                                    border: 'none',
                                                                    cursor: 'pointer',
                                                                    color: 'var(--accent-primary)',
                                                                    padding:
                                                                        '0.25rem',
                                                                }}
                                                            >
                                                                <FaPlus
                                                                    size={16}
                                                                />
                                                            </button>
                                                        </div>
                                                        {editingProvider ===
                                                            variant && (
                                                            <div
                                                                style={{
                                                                    display:
                                                                        'flex',
                                                                    flexDirection:
                                                                        'column',
                                                                    gap: '0.5rem',
                                                                }}
                                                            >
                                                                {variant !==
                                                                    'ollama' && (
                                                                    <div>
                                                                        <label
                                                                            style={{
                                                                                display:
                                                                                    'block',
                                                                                marginBottom:
                                                                                    '0.25rem',
                                                                                fontSize:
                                                                                    '0.8rem',
                                                                                color: 'var(--text-secondary)',
                                                                            }}
                                                                        >
                                                                            API
                                                                            Key
                                                                        </label>
                                                                        <input
                                                                            style={
                                                                                provInputStyle
                                                                            }
                                                                            type="password"
                                                                            placeholder="sk-..."
                                                                            value={
                                                                                providerForm.api_key
                                                                            }
                                                                            onChange={(
                                                                                e
                                                                            ) =>
                                                                                setProviderForm(
                                                                                    {
                                                                                        ...providerForm,
                                                                                        api_key:
                                                                                            e
                                                                                                .target
                                                                                                .value,
                                                                                    }
                                                                                )
                                                                            }
                                                                        />
                                                                    </div>
                                                                )}
                                                                {(variant ===
                                                                    'ollama' ||
                                                                    variant ===
                                                                        'openai' ||
                                                                    variant ===
                                                                        'anthropic') && (
                                                                    <div>
                                                                        <label
                                                                            style={{
                                                                                display:
                                                                                    'block',
                                                                                marginBottom:
                                                                                    '0.25rem',
                                                                                fontSize:
                                                                                    '0.8rem',
                                                                                color: 'var(--text-secondary)',
                                                                            }}
                                                                        >
                                                                            Base
                                                                            URL
                                                                        </label>
                                                                        <input
                                                                            style={
                                                                                provInputStyle
                                                                            }
                                                                            placeholder={
                                                                                variant ===
                                                                                'ollama'
                                                                                    ? 'http://localhost:11434'
                                                                                    : variant ===
                                                                                        'anthropic'
                                                                                      ? 'https://api.anthropic.com'
                                                                                      : 'https://api.openai.com/v1'
                                                                            }
                                                                            value={
                                                                                providerForm.base_url
                                                                            }
                                                                            onChange={(
                                                                                e
                                                                            ) =>
                                                                                setProviderForm(
                                                                                    {
                                                                                        ...providerForm,
                                                                                        base_url:
                                                                                            e
                                                                                                .target
                                                                                                .value,
                                                                                    }
                                                                                )
                                                                            }
                                                                        />
                                                                    </div>
                                                                )}
                                                                <div
                                                                    style={{
                                                                        display:
                                                                            'flex',
                                                                        gap: '0.5rem',
                                                                    }}
                                                                >
                                                                    <button
                                                                        onClick={() =>
                                                                            setEditingProvider(
                                                                                null
                                                                            )
                                                                        }
                                                                        style={{
                                                                            padding:
                                                                                '0.4rem 1rem',
                                                                            borderRadius:
                                                                                '0.375rem',
                                                                            border: '1px solid var(--border)',
                                                                            background:
                                                                                'transparent',
                                                                            cursor: 'pointer',
                                                                            fontSize:
                                                                                '0.8rem',
                                                                        }}
                                                                    >
                                                                        Cancel
                                                                    </button>
                                                                    <button
                                                                        onClick={() =>
                                                                            handleSaveProvider(
                                                                                variant
                                                                            )
                                                                        }
                                                                        style={{
                                                                            padding:
                                                                                '0.4rem 1rem',
                                                                            borderRadius:
                                                                                '0.375rem',
                                                                            border: 'none',
                                                                            background:
                                                                                'var(--accent-primary)',
                                                                            color: '#fff',
                                                                            cursor: 'pointer',
                                                                            fontSize:
                                                                                '0.8rem',
                                                                        }}
                                                                    >
                                                                        Save
                                                                    </button>
                                                                </div>
                                                            </div>
                                                        )}
                                                    </div>
                                                ))}
                                            </>
                                        )
                                    })()}
                                </div>
                            )}
                        </div>
                    )}

                    {/* MCP Servers Section */}
                    {activeSection === 'mcp-servers' && (
                        <div style={{ maxWidth: '700px' }}>
                            <div
                                style={{
                                    display: 'flex',
                                    justifyContent: 'space-between',
                                    alignItems: 'center',
                                    marginBottom: '1.5rem',
                                }}
                            >
                                <div>
                                    <h2 style={{ marginBottom: '0.5rem' }}>
                                        MCP Servers
                                    </h2>
                                    <p
                                        style={{
                                            color: 'var(--text-secondary)',
                                            fontSize: '14px',
                                        }}
                                    >
                                        Configure Model Context Protocol server
                                        connections. Agents can reference these
                                        by name.
                                    </p>
                                </div>
                                {!showAddMcp && (
                                    <button
                                        className="btn btn-primary"
                                        onClick={() => {
                                            setShowAddMcp(true)
                                            setEditingMcp(null)
                                            setMcpForm({
                                                name: '',
                                                host: 'local',
                                                command: '',
                                                args: '',
                                                uri: '',
                                                env: '',
                                            })
                                        }}
                                    >
                                        <FaPlus size={14} /> Add Server
                                    </button>
                                )}
                            </div>

                            {/* Add/Edit MCP Form */}
                            {showAddMcp && (
                                <div
                                    className="card"
                                    style={{ marginBottom: '1.5rem' }}
                                >
                                    <h3 style={{ marginBottom: '1rem' }}>
                                        {editingMcp
                                            ? `Edit ${editingMcp}`
                                            : 'Add MCP Server'}
                                    </h3>
                                    <div
                                        style={{
                                            display: 'flex',
                                            flexDirection: 'column',
                                            gap: '0.75rem',
                                        }}
                                    >
                                        <div>
                                            <label
                                                style={{
                                                    display: 'block',
                                                    marginBottom: '0.25rem',
                                                    fontSize: '0.8rem',
                                                    color: 'var(--text-secondary)',
                                                }}
                                            >
                                                Name
                                            </label>
                                            <input
                                                style={pInputStyle}
                                                placeholder="my-server"
                                                value={mcpForm.name}
                                                onChange={(e) =>
                                                    setMcpForm({
                                                        ...mcpForm,
                                                        name: e.target.value,
                                                    })
                                                }
                                                disabled={!!editingMcp}
                                            />
                                        </div>
                                        <div>
                                            <label
                                                style={{
                                                    display: 'block',
                                                    marginBottom: '0.25rem',
                                                    fontSize: '0.8rem',
                                                    color: 'var(--text-secondary)',
                                                }}
                                            >
                                                Type
                                            </label>
                                            <div
                                                style={{
                                                    display: 'flex',
                                                    gap: '1rem',
                                                }}
                                            >
                                                <label
                                                    style={{
                                                        display: 'flex',
                                                        alignItems: 'center',
                                                        gap: '0.4rem',
                                                        fontSize: '0.85rem',
                                                        cursor: 'pointer',
                                                    }}
                                                >
                                                    <input
                                                        type="radio"
                                                        checked={
                                                            mcpForm.host ===
                                                            'local'
                                                        }
                                                        onChange={() =>
                                                            setMcpForm({
                                                                ...mcpForm,
                                                                host: 'local',
                                                            })
                                                        }
                                                    />{' '}
                                                    Local Process
                                                </label>
                                                <label
                                                    style={{
                                                        display: 'flex',
                                                        alignItems: 'center',
                                                        gap: '0.4rem',
                                                        fontSize: '0.85rem',
                                                        cursor: 'pointer',
                                                    }}
                                                >
                                                    <input
                                                        type="radio"
                                                        checked={
                                                            mcpForm.host ===
                                                            'http'
                                                        }
                                                        onChange={() =>
                                                            setMcpForm({
                                                                ...mcpForm,
                                                                host: 'http',
                                                            })
                                                        }
                                                    />{' '}
                                                    HTTP
                                                </label>
                                            </div>
                                        </div>
                                        {mcpForm.host === 'local' ? (
                                            <>
                                                <div>
                                                    <label
                                                        style={{
                                                            display: 'block',
                                                            marginBottom:
                                                                '0.25rem',
                                                            fontSize: '0.8rem',
                                                            color: 'var(--text-secondary)',
                                                        }}
                                                    >
                                                        Command
                                                    </label>
                                                    <input
                                                        style={pInputStyle}
                                                        placeholder="npx"
                                                        value={mcpForm.command}
                                                        onChange={(e) =>
                                                            setMcpForm({
                                                                ...mcpForm,
                                                                command:
                                                                    e.target
                                                                        .value,
                                                            })
                                                        }
                                                    />
                                                </div>
                                                <div>
                                                    <label
                                                        style={{
                                                            display: 'block',
                                                            marginBottom:
                                                                '0.25rem',
                                                            fontSize: '0.8rem',
                                                            color: 'var(--text-secondary)',
                                                        }}
                                                    >
                                                        Arguments
                                                        (space-separated)
                                                    </label>
                                                    <input
                                                        style={pInputStyle}
                                                        placeholder="-y @modelcontextprotocol/server-filesystem /path"
                                                        value={mcpForm.args}
                                                        onChange={(e) =>
                                                            setMcpForm({
                                                                ...mcpForm,
                                                                args: e.target
                                                                    .value,
                                                            })
                                                        }
                                                    />
                                                </div>
                                            </>
                                        ) : (
                                            <div>
                                                <label
                                                    style={{
                                                        display: 'block',
                                                        marginBottom: '0.25rem',
                                                        fontSize: '0.8rem',
                                                        color: 'var(--text-secondary)',
                                                    }}
                                                >
                                                    URI
                                                </label>
                                                <input
                                                    style={pInputStyle}
                                                    placeholder="http://localhost:3000/mcp"
                                                    value={mcpForm.uri}
                                                    onChange={(e) =>
                                                        setMcpForm({
                                                            ...mcpForm,
                                                            uri: e.target.value,
                                                        })
                                                    }
                                                />
                                            </div>
                                        )}
                                        <div>
                                            <label
                                                style={{
                                                    display: 'block',
                                                    marginBottom: '0.25rem',
                                                    fontSize: '0.8rem',
                                                    color: 'var(--text-secondary)',
                                                }}
                                            >
                                                Environment Variables
                                                (KEY=value, one per line)
                                            </label>
                                            <textarea
                                                style={{
                                                    ...pInputStyle,
                                                    minHeight: '60px',
                                                    resize: 'vertical',
                                                    fontFamily: 'monospace',
                                                    fontSize: '0.8rem',
                                                }}
                                                placeholder="API_KEY=abc123"
                                                value={mcpForm.env}
                                                onChange={(e) =>
                                                    setMcpForm({
                                                        ...mcpForm,
                                                        env: e.target.value,
                                                    })
                                                }
                                            />
                                        </div>
                                        <div
                                            style={{
                                                display: 'flex',
                                                gap: '0.5rem',
                                            }}
                                        >
                                            <button
                                                className="btn btn-primary"
                                                onClick={handleSaveMcpServer}
                                                disabled={savingMcp}
                                            >
                                                {savingMcp
                                                    ? 'Saving...'
                                                    : 'Save'}
                                            </button>
                                            <button
                                                className="btn btn-secondary"
                                                onClick={() => {
                                                    setShowAddMcp(false)
                                                    setEditingMcp(null)
                                                }}
                                                disabled={savingMcp}
                                            >
                                                Cancel
                                            </button>
                                        </div>
                                    </div>
                                </div>
                            )}

                            {/* MCP Servers List */}
                            {loadingMcp ? (
                                <p
                                    style={{
                                        textAlign: 'center',
                                        padding: '2rem',
                                        color: 'var(--text-tertiary)',
                                    }}
                                >
                                    Loading...
                                </p>
                            ) : Object.keys(mcpServers).length === 0 ? (
                                <p
                                    style={{
                                        textAlign: 'center',
                                        padding: '2rem',
                                        color: 'var(--text-tertiary)',
                                    }}
                                >
                                    No MCP servers configured.
                                </p>
                            ) : (
                                <div
                                    style={{
                                        display: 'flex',
                                        flexDirection: 'column',
                                        gap: '0.75rem',
                                    }}
                                >
                                    {Object.entries(mcpServers).map(
                                        ([name, cfg]) => (
                                            <div
                                                key={name}
                                                style={{
                                                    padding: '1rem',
                                                    border: '1px solid var(--border)',
                                                    borderRadius: '0.5rem',
                                                    display: 'flex',
                                                    justifyContent:
                                                        'space-between',
                                                    alignItems: 'center',
                                                }}
                                            >
                                                <div>
                                                    <strong>{name}</strong>
                                                    <span
                                                        style={{
                                                            marginLeft:
                                                                '0.75rem',
                                                            fontSize: '0.75rem',
                                                            padding: '2px 8px',
                                                            borderRadius: '4px',
                                                            background:
                                                                cfg.host ===
                                                                'local'
                                                                    ? 'var(--surface)'
                                                                    : 'var(--accent-primary)',
                                                            color:
                                                                cfg.host ===
                                                                'local'
                                                                    ? 'var(--text-secondary)'
                                                                    : '#fff',
                                                        }}
                                                    >
                                                        {cfg.host}
                                                    </span>
                                                    <div
                                                        style={{
                                                            fontSize: '0.8rem',
                                                            color: 'var(--text-tertiary)',
                                                            marginTop:
                                                                '0.25rem',
                                                        }}
                                                    >
                                                        {cfg.host === 'local'
                                                            ? `${cfg.command} ${(cfg.args || []).join(' ')}`
                                                            : cfg.uri}
                                                    </div>
                                                </div>
                                                <div
                                                    style={{
                                                        display: 'flex',
                                                        gap: '0.5rem',
                                                    }}
                                                >
                                                    <button
                                                        onClick={() =>
                                                            handleEditMcpServer(
                                                                name
                                                            )
                                                        }
                                                        style={{
                                                            background: 'none',
                                                            border: 'none',
                                                            cursor: 'pointer',
                                                            color: 'var(--text-secondary)',
                                                            padding: '0.25rem',
                                                            fontSize: '0.8rem',
                                                        }}
                                                    >
                                                        Edit
                                                    </button>
                                                    <button
                                                        onClick={() =>
                                                            handleDeleteMcpServer(
                                                                name
                                                            )
                                                        }
                                                        style={{
                                                            background: 'none',
                                                            border: 'none',
                                                            cursor: 'pointer',
                                                            color: '#ef4444',
                                                            padding: '0.25rem',
                                                        }}
                                                    >
                                                        <FaTrash size={14} />
                                                    </button>
                                                </div>
                                            </div>
                                        )
                                    )}
                                </div>
                            )}
                        </div>
                    )}

                    {/* Shell Config Section */}
                    {activeSection === 'shell' && (
                        <div style={{ maxWidth: '600px' }}>
                            <h2 style={{ marginBottom: '1rem' }}>
                                Shell Configuration
                            </h2>
                            <p
                                style={{
                                    color: 'var(--text-secondary)',
                                    marginBottom: '2rem',
                                    fontSize: '14px',
                                }}
                            >
                                Configure the shell environment for agent
                                command execution.
                            </p>

                            {loadingShell ? (
                                <p style={{ color: 'var(--text-tertiary)' }}>
                                    Loading...
                                </p>
                            ) : (
                                <div
                                    style={{
                                        display: 'flex',
                                        flexDirection: 'column',
                                        gap: '1rem',
                                    }}
                                >
                                    <div>
                                        <label
                                            style={{
                                                display: 'block',
                                                marginBottom: '0.5rem',
                                                fontSize: '0.85rem',
                                                fontWeight: 500,
                                                color: 'var(--text-secondary)',
                                            }}
                                        >
                                            Environment
                                        </label>
                                        <div
                                            style={{
                                                display: 'flex',
                                                gap: '1rem',
                                            }}
                                        >
                                            <label
                                                style={{
                                                    display: 'flex',
                                                    alignItems: 'center',
                                                    gap: '0.4rem',
                                                    fontSize: '0.85rem',
                                                    cursor: 'pointer',
                                                }}
                                            >
                                                <input
                                                    type="radio"
                                                    checked={
                                                        shellForm.environment ===
                                                        'local'
                                                    }
                                                    onChange={() =>
                                                        setShellForm({
                                                            ...shellForm,
                                                            environment:
                                                                'local',
                                                            path: '.',
                                                        })
                                                    }
                                                />{' '}
                                                Local
                                            </label>
                                            <label
                                                style={{
                                                    display: 'flex',
                                                    alignItems: 'center',
                                                    gap: '0.4rem',
                                                    fontSize: '0.85rem',
                                                    cursor: 'pointer',
                                                }}
                                            >
                                                <input
                                                    type="radio"
                                                    checked={
                                                        shellForm.environment ===
                                                        'docker'
                                                    }
                                                    onChange={() =>
                                                        setShellForm({
                                                            ...shellForm,
                                                            environment:
                                                                'docker',
                                                            container_name:
                                                                'vizier',
                                                            image: {
                                                                source: 'pull',
                                                                name: 'ubuntu:latest',
                                                            },
                                                        })
                                                    }
                                                />{' '}
                                                Docker
                                            </label>
                                        </div>
                                    </div>

                                    {shellForm.environment === 'local' ? (
                                        <div>
                                            <label
                                                style={{
                                                    display: 'block',
                                                    marginBottom: '0.25rem',
                                                    fontSize: '0.8rem',
                                                    color: 'var(--text-secondary)',
                                                }}
                                            >
                                                Working Directory
                                            </label>
                                            <input
                                                style={pInputStyle}
                                                placeholder="."
                                                value={shellForm.path || ''}
                                                onChange={(e) =>
                                                    setShellForm({
                                                        ...shellForm,
                                                        path: e.target.value,
                                                    })
                                                }
                                            />
                                        </div>
                                    ) : (
                                        <>
                                            <div>
                                                <label
                                                    style={{
                                                        display: 'block',
                                                        marginBottom: '0.25rem',
                                                        fontSize: '0.8rem',
                                                        color: 'var(--text-secondary)',
                                                    }}
                                                >
                                                    Image Name
                                                </label>
                                                <input
                                                    style={pInputStyle}
                                                    placeholder="ubuntu:latest"
                                                    value={
                                                        shellForm.image?.name ||
                                                        ''
                                                    }
                                                    onChange={(e) =>
                                                        setShellForm({
                                                            ...shellForm,
                                                            image: {
                                                                source: 'pull',
                                                                name: e.target
                                                                    .value,
                                                            },
                                                        })
                                                    }
                                                />
                                            </div>
                                            <div>
                                                <label
                                                    style={{
                                                        display: 'block',
                                                        marginBottom: '0.25rem',
                                                        fontSize: '0.8rem',
                                                        color: 'var(--text-secondary)',
                                                    }}
                                                >
                                                    Container Name
                                                </label>
                                                <input
                                                    style={pInputStyle}
                                                    placeholder="vizier"
                                                    value={
                                                        shellForm.container_name ||
                                                        ''
                                                    }
                                                    onChange={(e) =>
                                                        setShellForm({
                                                            ...shellForm,
                                                            container_name:
                                                                e.target.value,
                                                        })
                                                    }
                                                />
                                            </div>
                                        </>
                                    )}

                                    <div>
                                        <label
                                            style={{
                                                display: 'block',
                                                marginBottom: '0.25rem',
                                                fontSize: '0.8rem',
                                                color: 'var(--text-secondary)',
                                            }}
                                        >
                                            Environment Variables (KEY=value,
                                            one per line)
                                        </label>
                                        <textarea
                                            style={{
                                                ...pInputStyle,
                                                minHeight: '80px',
                                                resize: 'vertical',
                                                fontFamily: 'monospace',
                                                fontSize: '0.8rem',
                                            }}
                                            placeholder="PATH=/usr/local/bin:/usr/bin"
                                            value={envToString(shellForm.env)}
                                            onChange={(e) =>
                                                setShellForm({
                                                    ...shellForm,
                                                    env: parseEnvString(
                                                        e.target.value
                                                    ),
                                                })
                                            }
                                        />
                                    </div>

                                    <button
                                        className="btn btn-primary"
                                        onClick={handleSaveShell}
                                        disabled={savingShell}
                                        style={{ alignSelf: 'flex-start' }}
                                    >
                                        {savingShell
                                            ? 'Saving...'
                                            : 'Save Shell Config'}
                                    </button>
                                </div>
                            )}
                        </div>
                    )}

                    {/* Users Section */}
                    {activeSection === 'users' && (
                        <UsersSection />
                    )}

                    {/* Roles Section */}
                    {activeSection === 'roles' && (
                        <RolesSection />
                    )}
                </div>
            </div>

            {/* Created Key Modal */}
            {createdKey && (
                <>
                    <div
                        style={{
                            position: 'fixed',
                            top: 0,
                            left: 0,
                            right: 0,
                            bottom: 0,
                            background: 'rgba(0, 0, 0, 0.5)',
                            zIndex: 1000,
                        }}
                        onClick={closeCreatedKeyModal}
                    />
                    <div
                        style={{
                            position: 'fixed',
                            top: '50%',
                            left: '50%',
                            transform: 'translate(-50%, -50%)',
                            background: 'var(--background)',
                            borderRadius: '8px',
                            padding: '2rem',
                            maxWidth: '600px',
                            width: '90%',
                            zIndex: 1001,
                            border: '1px solid var(--border)',
                        }}
                    >
                        <h2 style={{ marginBottom: '1rem' }}>
                            API Key Created
                        </h2>
                        <p
                            style={{
                                color: '#c62828',
                                background: '#ffebee',
                                padding: '12px',
                                borderRadius: '4px',
                                fontSize: '14px',
                                marginBottom: '1rem',
                            }}
                        >
                            <strong>Important:</strong> This key will only be
                            shown once. Make sure to copy it now.
                        </p>
                        <div
                            style={{
                                background: 'var(--surface)',
                                padding: '16px',
                                borderRadius: '4px',
                                fontFamily: 'var(--font-mono)',
                                fontSize: '14px',
                                wordBreak: 'break-all',
                                marginBottom: '1.5rem',
                                border: '1px solid var(--border)',
                            }}
                        >
                            {createdKey}
                        </div>
                        <div style={{ display: 'flex', gap: '8px' }}>
                            <button
                                className="btn btn-primary"
                                onClick={() => {
                                    navigator.clipboard.writeText(createdKey)
                                    alert('API key copied to clipboard')
                                }}
                            >
                                Copy to Clipboard
                            </button>
                            <button
                                className="btn btn-secondary"
                                onClick={closeCreatedKeyModal}
                            >
                                Close
                            </button>
                        </div>
                    </div>
                </>
            )}
        </>
    )
}
