export interface Chat {
  user_id?: string
  username?: string
  user_type: 'agent' | 'user'
  content: 'thinking' | string
  choice?: Choice
  timestamp?: string,
}


export interface Choice {
  name: string
  args: any
}

export interface WSChatResponse {
  content: string
  thinking: boolean,
  choice?: Choice
}

export interface AgentDetail {
  agent_id: string
  name: string
  description: string
}
