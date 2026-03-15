export interface Chat {
  user_id?: string
  username?: string
  user_type: 'agent' | 'user'
  content: 'thinking' | string
  timestamp?: string
}

export interface WSChatResponse {
  content: string
  thinking: boolean
}

export interface AgentDetail {
  agent_id: string
  name: string
  description: string
}
