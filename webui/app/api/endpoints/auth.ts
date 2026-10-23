import { api } from '../client'

export type CurrentUser = {
    user_id: string
    username: string
    role_name: string
    permissions: string[]
}

export const login = (username: string, password: string) =>
    api.post<{ token: string }>('/auth/login', { username, password })

export const currentUser = () => api.get<CurrentUser>('/auth/users/me')

export const setupStatus = () => api.get<{ needs_setup: boolean }>('/auth/setup-status')

export type UserSummary = { user_id: string; username: string; role_id: string; role_name?: string | null; created_at: string }

/** Everyone, for pickers (sharing). The Users page pages its own list. */
export const listUsers = () => api.get<UserSummary[]>('/auth/users')

export type ServerStatus = { online: boolean; version: string; setup_required: boolean }

/** Public: whether vizier is up, which version, and whether first-run setup is still needed. */
export const serverStatus = () => api.get<ServerStatus>('/status')

export const setupFirstUser = (username: string, password: string) =>
    api.post<{ token: string }>('/auth/setup', { username, password })

export type Profile = {
    user_id: string
    discord_id?: string | null
    discord_username?: string | null
    telegram_id?: string | null
    telegram_username?: string | null
    alias: string[]
}
export const myProfile = () => api.get<Profile>('/auth/users/me/profile')
export const saveMyProfile = (body: Partial<Omit<Profile, 'user_id'>>) => api.put<Profile>('/auth/users/me/profile', body)
export const changePassword = (current_password: string, new_password: string) =>
    api.post<string>('/auth/change-password', { current_password, new_password })

export type ApiKey = { id: string; name: string; prefix?: string | null; expires_at?: string | null; created_at: string; last_used_at?: string | null }
export const createApiKey = (name: string, expires_in_days?: number) =>
    api.post<ApiKey & { key: string }>('/auth/api-keys', { name, expires_in_days })
export const deleteApiKey = (id: string) => api.delete<string>(`/auth/api-keys/${encodeURIComponent(id)}`)

export type Role = { role_id: string; name: string; permissions: string[]; is_system: boolean; created_at: string; people: number }
export const availablePermissions = () => api.get<{ permissions: string[] }>('/auth/roles/available-permissions').then((r) => r?.permissions ?? [])
export const createRole = (name: string, permissions: string[]) => api.post<Role>('/auth/roles', { name, permissions })
export const updateRole = (id: string, name: string, permissions: string[]) => api.put<void>(`/auth/roles/${encodeURIComponent(id)}`, { name, permissions })
export const deleteRole = (id: string) => api.delete<void>(`/auth/roles/${encodeURIComponent(id)}`)

export const createUser = (username: string, password: string, role_id?: string) => api.post<UserSummary>('/auth/users', { username, password, role_id })
export const updateUser = (id: string, body: { username?: string; role_id?: string; password?: string }) => api.put<void>(`/auth/users/${encodeURIComponent(id)}`, body)
export const deleteUser = (id: string) => api.delete<void>(`/auth/users/${encodeURIComponent(id)}`)
