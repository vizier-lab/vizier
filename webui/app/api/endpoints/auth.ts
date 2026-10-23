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
