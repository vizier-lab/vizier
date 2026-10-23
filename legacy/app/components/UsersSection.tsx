import { useEffect, useState } from 'react'
import {
    listUsers,
    createUser,
    updateUser,
    deleteUser,
    listRoles,
} from '../services/vizier'
import { FaTrash, FaPlus, FaPen } from 'react-icons/fa6'
import { useToastStore } from '../hooks/toastStore'
import type { User, Role } from '../interfaces/types'
import { hasPermission } from '../utils/auth'

export default function UsersSection() {
    const addToast = useToastStore((s) => s.addToast)
    const [users, setUsers] = useState<User[]>([])
    const [roles, setRoles] = useState<Role[]>([])
    const [loading, setLoading] = useState(false)
    const [showCreateForm, setShowCreateForm] = useState(false)
    const [editingUser, setEditingUser] = useState<User | null>(null)
    const [form, setForm] = useState({
        username: '',
        password: '',
        role_id: '',
    })

    const canManageUsers = hasPermission('users:manage')

    useEffect(() => {
        if (canManageUsers) {
            loadData()
        }
    }, [canManageUsers])

    const loadData = async () => {
        try {
            setLoading(true)
            const [usersRes, rolesRes] = await Promise.all([
                listUsers(),
                listRoles(),
            ])
            setUsers(usersRes.data || [])
            setRoles(rolesRes.data || [])
        } catch (err: any) {
            addToast('error', err?.response?.data?.message || 'Failed to load users')
        } finally {
            setLoading(false)
        }
    }

    const handleCreate = async () => {
        if (!form.username || !form.password) {
            addToast('error', 'Username and password are required')
            return
        }

        try {
            await createUser(form.username, form.password, form.role_id || undefined)
            addToast('success', 'User created successfully')
            setShowCreateForm(false)
            setForm({ username: '', password: '', role_id: '' })
            loadData()
        } catch (err: any) {
            addToast('error', err?.response?.data?.message || 'Failed to create user')
        }
    }

    const handleUpdate = async () => {
        if (!editingUser) return

        try {
            const updateData: { username?: string; role_id?: string; password?: string } = {}
            if (form.username) updateData.username = form.username
            if (form.role_id) updateData.role_id = form.role_id
            if (form.password) updateData.password = form.password

            await updateUser(editingUser.user_id, updateData)
            addToast('success', 'User updated successfully')
            setEditingUser(null)
            setForm({ username: '', password: '', role_id: '' })
            loadData()
        } catch (err: any) {
            addToast('error', err?.response?.data?.message || 'Failed to update user')
        }
    }

    const handleDelete = async (userId: string) => {
        if (!confirm('Are you sure you want to delete this user?')) return

        try {
            await deleteUser(userId)
            addToast('success', 'User deleted successfully')
            loadData()
        } catch (err: any) {
            addToast('error', err?.response?.data?.message || 'Failed to delete user')
        }
    }

    const startEdit = (user: User) => {
        setEditingUser(user)
        setForm({
            username: user.username,
            password: '',
            role_id: user.role_id,
        })
    }

    if (!canManageUsers) {
        return (
            <div style={{ maxWidth: '600px' }}>
                <h2 style={{ marginBottom: '1rem' }}>Users</h2>
                <p style={{ color: 'var(--text-secondary)' }}>
                    You don't have permission to manage users.
                </p>
            </div>
        )
    }

    return (
        <div style={{ maxWidth: '800px' }}>
            <div style={{ display: 'flex', justifyContent: 'space-between', alignItems: 'center', marginBottom: '1.5rem' }}>
                <h2 style={{ margin: 0 }}>Users</h2>
                <button
                    className="btn btn-primary"
                    onClick={() => {
                        setShowCreateForm(true)
                        setEditingUser(null)
                        setForm({ username: '', password: '', role_id: '' })
                    }}
                >
                    <FaPlus style={{ marginRight: '0.5rem' }} />
                    Add User
                </button>
            </div>

            {/* Create/Edit Form */}
            {(showCreateForm || editingUser) && (
                <div style={{
                    padding: '1.5rem',
                    background: 'var(--surface)',
                    borderRadius: '0.5rem',
                    border: '1px solid var(--border)',
                    marginBottom: '1.5rem',
                }}>
                    <h3 style={{ marginTop: 0, marginBottom: '1rem' }}>
                        {editingUser ? 'Edit User' : 'Create User'}
                    </h3>
                    <div style={{ display: 'flex', flexDirection: 'column', gap: '1rem' }}>
                        <div>
                            <label style={{ display: 'block', marginBottom: '0.5rem', fontSize: '14px', fontWeight: '500' }}>
                                Username
                            </label>
                            <input
                                type="text"
                                value={form.username}
                                onChange={(e) => setForm({ ...form, username: e.target.value })}
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
                                {editingUser ? 'New Password (leave empty to keep current)' : 'Password'}
                            </label>
                            <input
                                type="password"
                                value={form.password}
                                onChange={(e) => setForm({ ...form, password: e.target.value })}
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
                                Role
                            </label>
                            <select
                                value={form.role_id}
                                onChange={(e) => setForm({ ...form, role_id: e.target.value })}
                                style={{
                                    width: '100%',
                                    padding: '0.5rem 0.75rem',
                                    borderRadius: '0.375rem',
                                    border: '1px solid var(--border)',
                                    background: 'var(--background)',
                                    color: 'var(--text-primary)',
                                }}
                            >
                                <option value="">Select a role</option>
                                {roles.map((role) => (
                                    <option key={role.role_id} value={role.role_id}>
                                        {role.name} {role.is_system ? '(System)' : ''}
                                    </option>
                                ))}
                            </select>
                        </div>
                        <div style={{ display: 'flex', gap: '0.75rem', justifyContent: 'flex-end' }}>
                            <button
                                className="btn"
                                onClick={() => {
                                    setShowCreateForm(false)
                                    setEditingUser(null)
                                    setForm({ username: '', password: '', role_id: '' })
                                }}
                            >
                                Cancel
                            </button>
                            <button
                                className="btn btn-primary"
                                onClick={editingUser ? handleUpdate : handleCreate}
                            >
                                {editingUser ? 'Update' : 'Create'}
                            </button>
                        </div>
                    </div>
                </div>
            )}

            {/* Users Table */}
            {loading ? (
                <div className="thinking-dots">
                    <span>.</span><span>.</span><span>.</span>
                </div>
            ) : users.length === 0 ? (
                <p style={{ color: 'var(--text-secondary)' }}>No users found.</p>
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
                                <th style={{ padding: '0.75rem 1rem', textAlign: 'left', fontSize: '14px', fontWeight: '600' }}>Username</th>
                                <th style={{ padding: '0.75rem 1rem', textAlign: 'left', fontSize: '14px', fontWeight: '600' }}>Role</th>
                                <th style={{ padding: '0.75rem 1rem', textAlign: 'left', fontSize: '14px', fontWeight: '600' }}>Created</th>
                                <th style={{ padding: '0.75rem 1rem', textAlign: 'right', fontSize: '14px', fontWeight: '600' }}>Actions</th>
                            </tr>
                        </thead>
                        <tbody>
                            {users.map((user) => (
                                <tr key={user.user_id} style={{ borderBottom: '1px solid var(--border)' }}>
                                    <td style={{ padding: '0.75rem 1rem' }}>{user.username}</td>
                                    <td style={{ padding: '0.75rem 1rem' }}>{user.role_name || 'Unknown'}</td>
                                    <td style={{ padding: '0.75rem 1rem', fontSize: '14px', color: 'var(--text-secondary)' }}>
                                        {new Date(user.created_at).toLocaleDateString()}
                                    </td>
                                    <td style={{ padding: '0.75rem 1rem', textAlign: 'right' }}>
                                        <button
                                            className="btn btn-ghost"
                                            onClick={() => startEdit(user)}
                                            style={{ marginRight: '0.5rem' }}
                                        >
                                            <FaPen />
                                        </button>
                                        <button
                                            className="btn btn-ghost text-red-500"
                                            onClick={() => handleDelete(user.user_id)}
                                        >
                                            <FaTrash />
                                        </button>
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
