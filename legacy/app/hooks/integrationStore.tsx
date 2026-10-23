import { useEffect } from 'react'
import { create } from 'zustand'
import {
  createAgentIntegration,
  deleteAgentIntegration,
  listAgentIntegrations,
  listIntegrationKinds,
  updateIntegration,
} from '../services/vizier'
import type {
  CreateIntegrationRequest,
  IntegrationFamily,
  IntegrationItem,
  IntegrationStatus,
  KindListing,
  UpdateIntegrationRequest,
} from '../interfaces/types'

/** How often an open Integrations page refreshes statuses. */
const STATUS_POLL_MS = 10_000

/** The selected agent's integrations of one family, as an Integrations page shows them. */
interface IntegrationStore {
  agentId: string | null
  family: IntegrationFamily | null
  items: IntegrationItem[]
  kinds: KindListing[]
  statuses: Record<string, IntegrationStatus>
  canManage: boolean
  loading: boolean
  load: (agentId: string, family: IntegrationFamily) => Promise<void>
  /** Re-read the list quietly (statuses included), for polling and after a write. */
  refreshStatus: () => Promise<void>
  create: (req: CreateIntegrationRequest) => Promise<IntegrationItem>
  update: (integrationId: string, req: UpdateIntegrationRequest) => Promise<void>
  /** Delete one of the agent's integrations (or unlink a legacy one from it). */
  remove: (integrationId: string) => Promise<void>
}

const byStatus = (items: IntegrationItem[]) =>
  Object.fromEntries(items.map((i) => [i.id, i.status]))

export const useIntegrationStore = create<IntegrationStore>()((set, get) => ({
  agentId: null,
  family: null,
  items: [],
  kinds: [],
  statuses: {},
  canManage: false,
  loading: false,

  load: async (agentId, family) => {
    const changed = get().agentId !== agentId || get().family !== family
    set({ agentId, family, loading: true, ...(changed ? { items: [], statuses: {} } : {}) })
    try {
      const [list, kinds] = await Promise.all([
        listAgentIntegrations(agentId, family),
        listIntegrationKinds(family),
      ])
      // A switch to another agent or page while this was in flight wins.
      if (get().agentId !== agentId || get().family !== family) return
      set({
        items: list.data.items,
        statuses: byStatus(list.data.items),
        canManage: list.data.can_manage,
        kinds: kinds.data,
      })
    } finally {
      if (get().agentId === agentId && get().family === family) set({ loading: false })
    }
  },

  refreshStatus: async () => {
    const { agentId, family } = get()
    if (!agentId || !family) return
    try {
      const list = await listAgentIntegrations(agentId, family)
      if (get().agentId !== agentId || get().family !== family) return
      set({
        items: list.data.items,
        statuses: byStatus(list.data.items),
        canManage: list.data.can_manage,
      })
    } catch {
      // Polling: the next tick tries again.
    }
  },

  create: async (req) => {
    const { agentId } = get()
    if (!agentId) throw new Error('no agent selected')
    const res = await createAgentIntegration(agentId, req)
    await get().refreshStatus()
    return res.data
  },

  update: async (integrationId, req) => {
    await updateIntegration(integrationId, req)
    await get().refreshStatus()
  },

  remove: async (integrationId) => {
    const { agentId } = get()
    if (!agentId) return
    await deleteAgentIntegration(agentId, integrationId)
    await get().refreshStatus()
  },
}))

/** Load the agent's integrations of `family`, reload when the agent changes, and poll
 *  statuses while the page is open. */
export function useIntegrations(agentId: string | undefined, family: IntegrationFamily) {
  const load = useIntegrationStore((s) => s.load)
  const refreshStatus = useIntegrationStore((s) => s.refreshStatus)
  useEffect(() => {
    if (!agentId) return
    load(agentId, family).catch(() => {})
    const timer = setInterval(() => refreshStatus(), STATUS_POLL_MS)
    return () => clearInterval(timer)
  }, [agentId, family, load, refreshStatus])
  return useIntegrationStore()
}

/** The server's `{status, message, data}` from a failed request. */
export function apiError(err: unknown): { message: string; data?: Record<string, unknown> } {
  const resp = (err as { response?: { data?: { message?: string; data?: Record<string, unknown> } } })
    ?.response?.data
  return { message: resp?.message || 'Something went wrong', data: resp?.data }
}
