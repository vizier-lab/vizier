import axios from 'axios'
import type { AgentDetail } from '~/interfaces/chat'

export const base_url = import.meta.env.DEV
  ? 'localhost:9999'
  : window.location.host

const apiClient = axios.create({
  baseURL: `http://${base_url}/api/v1`,
  headers: {
    'Content-Type': 'application/json',
  },
})

export const ping = async () => {
  const res = await apiClient.get('/ping')

  return res
}

export const createSession = async (agent_id: string, sessionId?: string) => {
  const session = sessionId ?? `${new Date().getTime()}`
  const res = await apiClient.post(
    session ? `/agents/${agent_id}/session/${session}` : `/agents/${agent_id}/session/${agent_id}`
  )

  return res
}

export const deleteSession = async (sessionId: string) => {
  const res = await apiClient.delete(`/session/${sessionId}`)

  return res
}

export const listSession = async () => {
  const res = await apiClient.get(`/session`)

  return res
}

export const listAgents = async () => {
  const res = await apiClient.get(`/agents`)

  return res
}

export const getAgentDetail = async (agent_id: string) => {
  const res = await apiClient.get(`/agents/${agent_id}`)

  return res
}
