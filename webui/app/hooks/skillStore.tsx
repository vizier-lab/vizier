import { create } from 'zustand'
import { listSkills, listAgentSkills, getSkill, getAgentSkill, createSkill, createAgentSkill, updateSkill, updateAgentSkill, deleteSkill, deleteAgentSkill } from '../services/vizier'
import type { Skill, CreateSkillRequest, UpdateSkillRequest } from '../interfaces/types'

interface SkillState {
  skills: Skill[]
  selectedSkill: Skill | null
  loading: boolean
  error: string | null
  agentId: string | null

  loadSkills: (agentId?: string) => Promise<void>
  selectSkill: (slug: string, agentId?: string) => Promise<void>
  clearSelection: () => void
  createSkill: (data: CreateSkillRequest, agentId?: string) => Promise<void>
  updateSkill: (slug: string, data: UpdateSkillRequest, agentId?: string) => Promise<void>
  deleteSkill: (slug: string, agentId?: string) => Promise<void>
}

export const useSkillStore = create<SkillState>((set, get) => ({
  skills: [],
  selectedSkill: null,
  loading: false,
  error: null,
  agentId: null,

  loadSkills: async (agentId?: string) => {
    set({ loading: true, error: null, agentId: agentId || null })
    try {
      // Load global skills
      const globalResponse = await listSkills()
      const globalSkills = globalResponse.data || []

      // Load agent-specific skills if agentId is provided
      let agentSkills: Skill[] = []
      if (agentId) {
        const agentResponse = await listAgentSkills(agentId)
        agentSkills = agentResponse.data || []
      }

      // Merge: agent skills override global skills with same name
      const allSkills = [...globalSkills]
      for (const agentSkill of agentSkills) {
        const existingIndex = allSkills.findIndex(s => s.name === agentSkill.name)
        if (existingIndex >= 0) {
          allSkills[existingIndex] = agentSkill
        } else {
          allSkills.push(agentSkill)
        }
      }

      set({ skills: allSkills })
    } catch (error) {
      console.error('Failed to load skills:', error)
      set({ error: 'Failed to load skills' })
    } finally {
      set({ loading: false })
    }
  },

  selectSkill: async (slug: string, agentId?: string) => {
    try {
      let response
      if (agentId) {
        response = await getAgentSkill(agentId, slug)
      } else {
        response = await getSkill(slug)
      }
      set({ selectedSkill: response.data })
    } catch (error) {
      console.error('Failed to load skill:', error)
      set({ error: 'Failed to load skill' })
    }
  },

  clearSelection: () => {
    set({ selectedSkill: null })
  },

  createSkill: async (data: CreateSkillRequest, agentId?: string) => {
    try {
      if (agentId) {
        await createAgentSkill(agentId, data)
      } else {
        await createSkill(data)
      }
      await get().loadSkills(agentId || get().agentId || undefined)
    } catch (error) {
      console.error('Failed to create skill:', error)
      throw error
    }
  },

  updateSkill: async (slug: string, data: UpdateSkillRequest, agentId?: string) => {
    try {
      if (agentId) {
        await updateAgentSkill(agentId, slug, data)
      } else {
        await updateSkill(slug, data)
      }
      await get().loadSkills(agentId || get().agentId || undefined)
      if (get().selectedSkill?.name === slug) {
        await get().selectSkill(slug, agentId)
      }
    } catch (error) {
      console.error('Failed to update skill:', error)
      throw error
    }
  },

  deleteSkill: async (slug: string, agentId?: string) => {
    try {
      if (agentId) {
        await deleteAgentSkill(agentId, slug)
      } else {
        await deleteSkill(slug)
      }
      set({ selectedSkill: null })
      await get().loadSkills(agentId || get().agentId || undefined)
    } catch (error) {
      console.error('Failed to delete skill:', error)
      throw error
    }
  },
}))