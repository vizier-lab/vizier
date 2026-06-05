import { useEffect, useState } from 'react'
import {
    listRoles,
    createRole,
    updateRole,
    deleteRole,
    getAvailablePermissions,
} from '../services/vizier'
import { FaTrash, FaPlus, FaPen } from 'react-icons/fa6'
import { useToastStore } from '../hooks/toastStore'
import type { Role } from '../interfaces/types'
import { hasPermission } from '../utils/auth'

const permissionGroups = [
    {
        label: 'Agents',
        permissions: [
            ['all_agents:view', 'View All Agents'],
            ['owned_agents:view', 'View Owned Agents'],
            ['all_agents:create', 'Create Agents'],
            ['all_agents:edit', 'Edit All Agents'],
            ['owned_agents:edit', 'Edit Owned Agents'],
            ['all_agents:delete', 'Delete All Agents'],
            ['owned_agents:delete', 'Delete Owned Agents'],
        ],
    },
    {
        label: 'Settings',
        permissions: [
            ['settings:providers', 'Manage Providers'],
            ['agents:mcp_config', 'Configure Agent MCP'],
            ['agents:shell_config', 'Configure Agent Shell'],
            ['settings:password', 'Change Password'],
            ['settings:api_keys', 'Manage API Keys'],
        ],
    },
    {
        label: 'Administration',
        permissions: [
            ['users:manage', 'Manage Users'],
            ['roles:manage', 'Manage Roles'],
        ],
    },
]

export default function RolesSection() {
    const addToast = useToastStore((s) => s.addToast)
    const [roles, setRoles] = useState<Role[]>([])
    const [loading, setLoading] = useState(false)
    const [showCreateForm, setShowCreateForm] = useState(false)
    const [editingRole, setEditingRole] = useState<Role | null>(null)
    const [form, setForm] = useState({
        name: '',
        permissions: [] as string[],
    })

    const canManageRoles = hasPermission('roles:manage')

    useEffect(() => {
        if (canManageRoles) {
            loadRoles()
        }
    }, [canManageRoles])

    const loadRoles = async () => {
        try {
            setLoading(true)
            const rolesRes = await listRoles()
            setRoles(rolesRes.data || [])
        } catch (err: any) {
            addToast('error', err?.response?.data?.message || 'Failed to load roles')
        } finally {
            setLoading(false)
        }
    }

    const handleCreate = async () => {
        if (!form.name) {
            addToast('error', 'Role name is required')
            return
        }

        try {
            await createRole(form.name, form.permissions)
            addToast('success', 'Role created successfully')
            setShowCreateForm(false)
            setForm({ name: '', permissions: [] })
            loadRoles()
        } catch (err: any) {
            addToast('error', err?.response?.data?.message || 'Failed to create role')
        }
    }

    const handleUpdate = async () => {
        if (!editingRole) return

        try {
            await updateRole(editingRole.role_id, form.name, form.permissions)
            addToast('success', 'Role updated successfully')
            setEditingRole(null)
            setForm({ name: '', permissions: [] })
            loadRoles()
        } catch (err: any) {
            addToast('error', err?.response?.data?.message || 'Failed to update role')
        }
    }

    const handleDelete = async (roleId: string) => {
        if (!confirm('Are you sure you want to delete this role?')) return

        try {
            await deleteRole(roleId)
            addToast('success', 'Role deleted successfully')
            loadRoles()
        } catch (err: any) {
            addToast('error', err?.response?.data?.message || 'Failed to delete role')
        }
    }

    const startEdit = (role: Role) => {
        setEditingRole(role)
        setForm({
            name: role.name,
            permissions: [...role.permissions],
        })
    }

    const togglePermission = (permission: string) => {
        setForm((prev) => ({
            ...prev,
            permissions: prev.permissions.includes(permission)
                ? prev.permissions.filter((p) => p !== permission)
                : [...prev.permissions, permission],
        }))
    }

    if (!canManageRoles) {
        return (
            <div style={{ maxWidth: '600px' }}>
                <h2 style={{ marginBottom: '1rem' }}>Roles</h2>
                <p style={{ color: 'var(--text-secondary)' }}>
                    You don't have permission to manage roles.
                </p>
            </div>
        )
    }

    return (
        <div style={{ maxWidth: '800px' }}>
            <div style={{ display: 'flex', justifyContent: 'space-between', alignItems: 'center', marginBottom: '1.5rem' }}>
                <h2 style={{ margin: 0 }}>Roles</h2>
                <button
                    className="btn btn-primary"
                    onClick={() => {
                        setShowCreateForm(true)
                        setEditingRole(null)
                        setForm({ name: '', permissions: [] })
                    }}
                >
                    <FaPlus style={{ marginRight: '0.5rem' }} />
                    Add Role
                </button>
            </div>

            {/* Create/Edit Form */}
            {(showCreateForm || editingRole) && (
                <div style={{
                    padding: '1.5rem',
                    background: 'var(--surface)',
                    borderRadius: '0.5rem',
                    border: '1px solid var(--border)',
                    marginBottom: '1.5rem',
                }}>
                    <h3 style={{ marginTop: 0, marginBottom: '1rem' }}>
                        {editingRole ? 'Edit Role' : 'Create Role'}
                    </h3>
                    <div style={{ display: 'flex', flexDirection: 'column', gap: '1rem' }}>
                        <div>
                            <label style={{ display: 'block', marginBottom: '0.5rem', fontSize: '14px', fontWeight: '500' }}>
                                Role Name
                            </label>
                            <input
                                type="text"
                                value={form.name}
                                onChange={(e) => setForm({ ...form, name: e.target.value })}
                                disabled={editingRole?.is_system}
                                style={{
                                    width: '100%',
                                    padding: '0.5rem 0.75rem',
                                    borderRadius: '0.375rem',
                                    border: '1px solid var(--border)',
                                    background: 'var(--background)',
                                    color: 'var(--text-primary)',
                                }}
                            />
                        </div>
                        <div>
                            <label style={{ display: 'block', marginBottom: '0.5rem', fontSize: '14px', fontWeight: '500' }}>
                                Permissions
                            </label>
                            <div style={{
                                display: 'flex',
                                flexDirection: 'column',
                                gap: '1rem',
                                padding: '0.75rem',
                                background: 'var(--background)',
                                borderRadius: '0.375rem',
                                border: '1px solid var(--border)',
                            }}>
                                {permissionGroups.map((group) => (
                                    <div key={group.label}>
                                        <div style={{
                                            fontSize: '12px',
                                            fontWeight: '600',
                                            color: 'var(--text-secondary)',
                                            textTransform: 'uppercase',
                                            letterSpacing: '0.05em',
                                            marginBottom: '0.5rem',
                                        }}>
                                            {group.label}
                                        </div>
                                        <div style={{
                                            display: 'grid',
                                            gridTemplateColumns: 'repeat(auto-fill, minmax(200px, 1fr))',
                                            gap: '0.5rem',
                                        }}>
                                            {group.permissions.map(([perm, label]) => (
                                                <label
                                                    key={perm}
                                                    style={{
                                                        display: 'flex',
                                                        alignItems: 'center',
                                                        gap: '0.5rem',
                                                        fontSize: '14px',
                                                        cursor: 'pointer',
                                                    }}
                                                >
                                                    <input
                                                        type="checkbox"
                                                        checked={form.permissions.includes(perm)}
                                                        onChange={() => togglePermission(perm)}
                                                    />
                                                    {label}
                                                </label>
                                            ))}
                                        </div>
                                    </div>
                                ))}
                            </div>
                        </div>
                        <div style={{ display: 'flex', gap: '0.75rem', justifyContent: 'flex-end' }}>
                            <button
                                className="btn"
                                onClick={() => {
                                    setShowCreateForm(false)
                                    setEditingRole(null)
                                    setForm({ name: '', permissions: [] })
                                }}
                            >
                                Cancel
                            </button>
                            <button
                                className="btn btn-primary"
                                onClick={editingRole ? handleUpdate : handleCreate}
                            >
                                {editingRole ? 'Update' : 'Create'}
                            </button>
                        </div>
                    </div>
                </div>
            )}

            {/* Roles Table */}
            {loading ? (
                <div className="thinking-dots">
                    <span>.</span><span>.</span><span>.</span>
                </div>
            ) : roles.length === 0 ? (
                <p style={{ color: 'var(--text-secondary)' }}>No roles found.</p>
            ) : (
                <div style={{
                    background: 'var(--surface)',
                    borderRadius: '0.5rem',
                    border: '1px solid var(--border)',
                    overflow: 'hidden',
                }}>
                    <table style={{ width: '100%', borderCollapse: 'collapse' }}>
                        <thead>
                            <tr style={{ borderBottom: '1px solid var(--border)' }}>
                                <th style={{ padding: '0.75rem 1rem', textAlign: 'left', fontSize: '14px', fontWeight: '600' }}>Name</th>
                                <th style={{ padding: '0.75rem 1rem', textAlign: 'left', fontSize: '14px', fontWeight: '600' }}>Permissions</th>
                                <th style={{ padding: '0.75rem 1rem', textAlign: 'left', fontSize: '14px', fontWeight: '600' }}>Type</th>
                                <th style={{ padding: '0.75rem 1rem', textAlign: 'right', fontSize: '14px', fontWeight: '600' }}>Actions</th>
                            </tr>
                        </thead>
                        <tbody>
                            {roles.map((role) => (
                                <tr key={role.role_id} style={{ borderBottom: '1px solid var(--border)' }}>
                                    <td style={{ padding: '0.75rem 1rem' }}>{role.name}</td>
                                    <td style={{ padding: '0.75rem 1rem' }}>
                                        <div style={{ display: 'flex', flexWrap: 'wrap', gap: '0.25rem' }}>
                                            {role.permissions.slice(0, 3).map((p) => (
                                                <span key={p} style={{
                                                    padding: '0.125rem 0.375rem',
                                                    background: 'var(--background)',
                                                    borderRadius: '0.25rem',
                                                    fontSize: '12px',
                                                }}>
                                                    {p}
                                                </span>
                                            ))}
                                            {role.permissions.length > 3 && (
                                                <span style={{
                                                    padding: '0.125rem 0.375rem',
                                                    background: 'var(--background)',
                                                    borderRadius: '0.25rem',
                                                    fontSize: '12px',
                                                }}>
                                                    +{role.permissions.length - 3} more
                                                </span>
                                            )}
                                        </div>
                                    </td>
                                    <td style={{ padding: '0.75rem 1rem' }}>
                                        {role.is_system ? (
                                            <span style={{
                                                padding: '0.125rem 0.375rem',
                                                background: 'rgba(16, 185, 129, 0.1)',
                                                color: '#10b981',
                                                borderRadius: '0.25rem',
                                                fontSize: '12px',
                                            }}>
                                                System
                                            </span>
                                        ) : (
                                            <span style={{
                                                padding: '0.125rem 0.375rem',
                                                background: 'var(--background)',
                                                borderRadius: '0.25rem',
                                                fontSize: '12px',
                                            }}>
                                                Custom
                                            </span>
                                        )}
                                    </td>
                                    <td style={{ padding: '0.75rem 1rem', textAlign: 'right' }}>
                                        {!role.is_system && (
                                            <>
                                                <button
                                                    className="btn btn-ghost"
                                                    onClick={() => startEdit(role)}
                                                    style={{ marginRight: '0.5rem' }}
                                                >
                                                    <FaPen />
                                                </button>
                                                <button
                                                    className="btn btn-ghost text-red-500"
                                                    onClick={() => handleDelete(role.role_id)}
                                                >
                                                    <FaTrash />
                                                </button>
                                            </>
                                        )}
                                    </td>
                                </tr>
                            ))}
                        </tbody>
                    </table>
                </div>
            )}
        </div>
    )
}
