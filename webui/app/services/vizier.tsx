import axios, { AxiosError } from 'axios'

export const base_url = import.meta.env.DEV
  ? 'localhost:9999'
  : window.location.host

export const CHANNEL_ID = 'vizier-webui' // Hardcoded channel_id

// Create axios instance
const apiClient = axios.create({
  baseURL: `http://${base_url}/api/v1`,
  headers: {
    'Content-Type': 'application/json',
  },
})

// Add auth interceptor
apiClient.interceptors.request.use((config) => {
  const token = localStorage.getItem('auth_token')
  if (token) {
    config.headers.Authorization = `Bearer ${token}`
  }
  return config
})

// Response interceptor for handling auth errors
apiClient.interceptors.response.use(
  (response) => response,
  (error: AxiosError) => {
    if (error.response?.status === 401) {
      localStorage.removeItem('auth_token')
      window.location.href = '/login'
    }
    return Promise.reject(error)
  }
)

// ============================================================================
// AUTH ENDPOINTS
// ============================================================================

export const login = async (username: string, password: string) => {
  const res = await apiClient.post('/auth/login', { username, password })
  if (res.data?.data?.token) {
    localStorage.setItem('auth_token', res.data.data.token)
  }
  return res.data
}

export const changePassword = async (currentPassword: string, newPassword: string) => {
  const res = await apiClient.post('/auth/change-password', {
    current_password: currentPassword,
    new_password: newPassword,
  })
  return res.data
}

export const createApiKey = async (name: string, expiresInDays?: number) => {
  const res = await apiClient.post('/auth/api-keys', {
    name,
    expires_in_days: expiresInDays,
  })
  return res.data
}

export const listApiKeys = async () => {
  const res = await apiClient.get('/auth/api-keys')
  return res.data
}

export const deleteApiKey = async (keyId: string) => {
  const res = await apiClient.delete(`/auth/api-keys/${keyId}`)
  return res.data
}

// ============================================================================
// SETUP ENDPOINTS
// ============================================================================

export const getSetupStatus = async () => {
  const res = await apiClient.get('/auth/setup-status')
  return res.data
}

export const setupFirstUser = async (username: string, password: string) => {
  const res = await apiClient.post('/auth/setup', { username, password })
  if (res.data?.data?.token) {
    localStorage.setItem('auth_token', res.data.data.token)
  }
  return res.data
}

// ============================================================================
// USER ENDPOINTS
// ============================================================================

export const getCurrentUser = async () => {
  const res = await apiClient.get('/auth/users/me')
  return res.data
}

// ============================================================================
// USER PROFILE ENDPOINTS
// ============================================================================

export const getMyProfile = async () => {
  const res = await apiClient.get('/auth/users/me/profile')
  return res.data
}

export const updateMyProfile = async (data: import('../interfaces/types').UpdateUserProfileRequest) => {
  const res = await apiClient.put('/auth/users/me/profile', data)
  return res.data
}

// ============================================================================
// ROLE ENDPOINTS
// ============================================================================

export const listRoles = async () => {
  const res = await apiClient.get('/auth/roles')
  return res.data
}

export const createRole = async (name: string, permissions: string[]) => {
  const res = await apiClient.post('/auth/roles', { name, permissions })
  return res.data
}

export const updateRole = async (roleId: string, name: string, permissions: string[]) => {
  const res = await apiClient.put(`/auth/roles/${roleId}`, { name, permissions })
  return res.data
}

export const deleteRole = async (roleId: string) => {
  const res = await apiClient.delete(`/auth/roles/${roleId}`)
  return res.data
}

export const getAvailablePermissions = async () => {
  const res = await apiClient.get('/auth/roles/available-permissions')
  return res.data
}

// ============================================================================
// USER ENDPOINTS
// ============================================================================

export const listUsers = async () => {
  const res = await apiClient.get('/auth/users')
  return res.data
}

export const createUser = async (username: string, password: string, roleId?: string) => {
  const res = await apiClient.post('/auth/users', { username, password, role_id: roleId })
  return res.data
}

export const updateUser = async (userId: string, data: { username?: string; role_id?: string; password?: string }) => {
  const res = await apiClient.put(`/auth/users/${userId}`, data)
  return res.data
}

export const deleteUser = async (userId: string) => {
  const res = await apiClient.delete(`/auth/users/${userId}`)
  return res.data
}

// ============================================================================
// AGENT ENDPOINTS
// ============================================================================

export const listAgents = async () => {
  const res = await apiClient.get('/agents')
  return res.data
}

export const getAgentDetail = async (agentId: string) => {
  const res = await apiClient.get(`/agents/${agentId}`)
  return res.data
}

export const getAgentUsage = async (
  agentId: string,
  startDate?: string,
  endDate?: string
) => {
  const params = new URLSearchParams()
  if (startDate) params.append('start_date', startDate)
  if (endDate) params.append('end_date', endDate)

  const res = await apiClient.get(`/agents/${agentId}/usage?${params}`)
  return res.data
}

export const createAgent = async (data: import('../interfaces/types').CreateAgentRequest) => {
  const res = await apiClient.post('/agents', data)
  return res.data
}

export const updateAgent = async (agentId: string, data: import('../interfaces/types').CreateAgentRequest) => {
  const res = await apiClient.put(`/agents/${agentId}`, data)
  return res.data
}

export const deleteAgent = async (agentId: string, deleteWorkspace: boolean = false) => {
  const res = await apiClient.delete(`/agents/${agentId}?delete_workspace=${deleteWorkspace}`)
  return res.data
}

// ============================================================================
// AGENT SHARING ENDPOINTS
// ============================================================================

export const getAgentSharing = async (agentId: string) => {
  const res = await apiClient.get(`/agents/${agentId}/sharing`)
  return res.data
}

export const updateAgentSharing = async (
  agentId: string,
  data: import('../interfaces/types').UpdateSharingRequest
) => {
  const res = await apiClient.patch(`/agents/${agentId}/sharing`, data)
  return res.data
}

// ============================================================================
// PROVIDER ENDPOINTS
// ============================================================================

export const listProviders = async () => {
  const res = await apiClient.get('/providers')
  return res.data
}

export const upsertProvider = async (variant: string, data: import('../interfaces/types').UpsertProviderRequest) => {
  const res = await apiClient.put(`/providers/${variant}`, data)
  return res.data
}

export const deleteProvider = async (variant: string) => {
  const res = await apiClient.delete(`/providers/${variant}`)
  return res.data
}

// ============================================================================
// CHAT/TOPIC ENDPOINTS
// ============================================================================

export const listTopics = async (agentId: string) => {
  const res = await apiClient.get(`/agents/${agentId}/channel/${CHANNEL_ID}/topics`)
  return res.data
}

export const getTopicHistory = async (
  agentId: string,
  topicId: string,
  before?: string,
  limit?: number
) => {
  const params = new URLSearchParams()
  if (before) params.append('before', before)
  if (limit) params.append('limit', limit.toString())

  const res = await apiClient.get(
    `/agents/${agentId}/channel/${CHANNEL_ID}/topic/${topicId}/history?${params}`
  )
  return res.data
}

export const getChatWebSocketUrl = (agentId: string, topicId: string) => {
  const token = localStorage.getItem('auth_token')
  const protocol = window.location.protocol === 'https:' ? 'wss:' : 'ws:'
  return `${protocol}//${base_url}/api/v1/agents/${agentId}/channel/${CHANNEL_ID}/topic/${topicId}/chat?token=${token}`
}

export const deleteTopic = async (agentId: string, topicId: string) => {
  const res = await apiClient.delete(`/agents/${agentId}/channel/${CHANNEL_ID}/topic/${topicId}`)
  return res.data
}

// ============================================================================
// MEMORY ENDPOINTS
// ============================================================================

export const listMemories = async (agentId: string) => {
  const res = await apiClient.get(`/agents/${agentId}/memory`)
  return res.data
}

export const getMemory = async (agentId: string, slug: string) => {
  const res = await apiClient.get(`/agents/${agentId}/memory/${slug}`)
  return res.data
}

export const createMemory = async (
  agentId: string,
  title: string,
  content: string,
  slug?: string,
  visibility?: string,
  sharedTo?: string[]
) => {
  const res = await apiClient.post(`/agents/${agentId}/memory`, {
    title,
    content,
    slug,
    visibility,
    shared_to: sharedTo,
  })
  return res.data
}

export const updateMemory = async (
  agentId: string,
  slug: string,
  title: string,
  content: string,
  visibility?: string,
  sharedTo?: string[]
) => {
  const res = await apiClient.put(`/agents/${agentId}/memory/${slug}`, {
    title,
    content,
    visibility,
    shared_to: sharedTo,
  })
  return res.data
}

export const deleteMemory = async (agentId: string, slug: string) => {
  const res = await apiClient.delete(`/agents/${agentId}/memory/${slug}`)
  return res.data
}

export const queryMemories = async (
  agentId: string,
  query: string,
  limit?: number,
  threshold?: number
) => {
  const params = new URLSearchParams()
  params.append('query', query)
  if (limit) params.append('limit', limit.toString())
  if (threshold) params.append('threshold', threshold.toString())

  const res = await apiClient.get(`/agents/${agentId}/memory/query?${params}`)
  return res.data
}

// ============================================================================
// TASK ENDPOINTS
// ============================================================================

export const listTasks = async (agentId: string, isActive?: boolean) => {
  const params = new URLSearchParams()
  if (isActive !== undefined) params.append('is_active', isActive.toString())

  const res = await apiClient.get(`/agents/${agentId}/tasks?${params}`)
  return res.data
}

export const getTask = async (agentId: string, slug: string) => {
  const res = await apiClient.get(`/agents/${agentId}/tasks/${slug}`)
  return res.data
}

export const createTask = async (
  agentId: string,
  data: {
    slug: string
    user: string
    title: string
    instruction: string
    schedule: { type: 'Cron'; expression: string } | { type: 'OneTime'; datetime: string }
  }
) => {
  const res = await apiClient.post(`/agents/${agentId}/tasks`, data)
  return res.data
}

export const updateTask = async (
  agentId: string,
  slug: string,
  data: {
    slug: string
    user: string
    title: string
    instruction: string
    schedule: { type: 'Cron'; expression: string } | { type: 'OneTime'; datetime: string }
  }
) => {
  const res = await apiClient.put(`/agents/${agentId}/tasks/${slug}`, data)
  return res.data
}

export const deleteTask = async (agentId: string, slug: string) => {
  const res = await apiClient.delete(`/agents/${agentId}/tasks/${slug}`)
  return res.data
}

// ============================================================================
// DOCUMENT ENDPOINTS
// ============================================================================

export const getAgentDocument = async (agentId: string) => {
  const res = await apiClient.get(`/agents/${agentId}/documents/agent`)
  return res.data
}

export const updateAgentDocument = async (agentId: string, content: string) => {
  const res = await apiClient.put(`/agents/${agentId}/documents/agent`, { content })
  return res.data
}

export const getIdentityDocument = async (agentId: string) => {
  const res = await apiClient.get(`/agents/${agentId}/documents/identity`)
  return res.data
}

export const updateIdentityDocument = async (agentId: string, content: string) => {
  const res = await apiClient.put(`/agents/${agentId}/documents/identity`, { content })
  return res.data
}

export const getHeartbeatDocument = async (agentId: string) => {
  const res = await apiClient.get(`/agents/${agentId}/documents/heartbeat`)
  return res.data
}

export const updateHeartbeatDocument = async (agentId: string, content: string) => {
  const res = await apiClient.put(`/agents/${agentId}/documents/heartbeat`, { content })
  return res.data
}

// ============================================================================
// UTILITY
// ============================================================================

export const ping = async () => {
  const res = await apiClient.get('/ping')
  return res.data
}

// ============================================================================
// MCP SERVERS ENDPOINTS
// ============================================================================

export const getMcpServers = async () => {
  const res = await apiClient.get('/global-config/mcp-servers')
  return res.data
}

export const upsertMcpServers = async (data: unknown) => {
  const res = await apiClient.put('/global-config/mcp-servers', data)
  return res.data
}

export const deleteMcpServers = async () => {
  const res = await apiClient.delete('/global-config/mcp-servers')
  return res.data
}

// ============================================================================
// SHELL CONFIG ENDPOINTS
// ============================================================================

export const getShellConfig = async () => {
  const res = await apiClient.get('/global-config/shell')
  return res.data
}

export const upsertShellConfig = async (data: unknown) => {
  const res = await apiClient.put('/global-config/shell', data)
  return res.data
}

export const deleteShellConfig = async () => {
  const res = await apiClient.delete('/global-config/shell')
  return res.data
}

// ============================================================================
// FILE UPLOAD ENDPOINTS
// ============================================================================

export interface UploadResponse {
  file_id: string
  filename: string
  url: string
}

export const fileToBase64 = (file: File): Promise<string> => {
  return new Promise((resolve, reject) => {
    const reader = new FileReader()
    reader.onload = () => {
      const result = reader.result as string
      const base64 = result.split(',')[1] || ''
      resolve(base64)
    }
    reader.onerror = reject
    reader.readAsDataURL(file)
  })
}

export const uploadFile = async (file: File): Promise<UploadResponse> => {
  const base64 = await fileToBase64(file)
  const res = await apiClient.post('/files/upload', {
    file: base64,
    filename: file.name,
  })
  return res.data.data
}

// ============================================================================
// SKILL ENDPOINTS
// ============================================================================

export const listSkills = async () => {
  const res = await apiClient.get('/skills')
  return res.data
}

export const getSkill = async (slug: string) => {
  const res = await apiClient.get(`/skills/${slug}`)
  return res.data
}

export const createSkill = async (data: import('../interfaces/types').CreateSkillRequest) => {
  const res = await apiClient.post('/skills', data)
  return res.data
}

export const updateSkill = async (slug: string, data: import('../interfaces/types').UpdateSkillRequest) => {
  const res = await apiClient.put(`/skills/${slug}`, data)
  return res.data
}

export const deleteSkill = async (slug: string) => {
  const res = await apiClient.delete(`/skills/${slug}`)
  return res.data
}

export const listSkillResources = async (slug: string) => {
  const res = await apiClient.get(`/skills/${slug}/resources`)
  return res.data
}

export const getSkillResource = async (slug: string, path: string) => {
  const res = await apiClient.get(`/skills/${slug}/resources/${path}`)
  return res.data
}

// Agent skills
export const listAgentSkills = async (agentId: string) => {
  const res = await apiClient.get(`/agents/${agentId}/skills`)
  return res.data
}

export const getAgentSkill = async (agentId: string, slug: string) => {
  const res = await apiClient.get(`/agents/${agentId}/skills/${slug}`)
  return res.data
}

export const createAgentSkill = async (agentId: string, data: import('../interfaces/types').CreateSkillRequest) => {
  const res = await apiClient.post(`/agents/${agentId}/skills`, data)
  return res.data
}

export const updateAgentSkill = async (agentId: string, slug: string, data: import('../interfaces/types').UpdateSkillRequest) => {
  const res = await apiClient.put(`/agents/${agentId}/skills/${slug}`, data)
  return res.data
}

export const deleteAgentSkill = async (agentId: string, slug: string) => {
  const res = await apiClient.delete(`/agents/${agentId}/skills/${slug}`)
  return res.data
}
