import { useNavigate } from 'react-router'
import { useToastStore } from '../hooks/toastStore'
import { createAgent } from '../services/vizier'
import AgentForm from '../components/AgentForm'
import type { CreateAgentRequest } from '../interfaces/types'

export default function AgentNew() {
  const navigate = useNavigate()
  const addToast = useToastStore((s) => s.addToast)

  const handleSubmit = async (form: CreateAgentRequest) => {
    try {
      const res = await createAgent(form)
      for (const w of res.data?.warnings || []) addToast('warning', w.message)
      addToast('success', `Agent "${form.name}" created`)
      setTimeout(() => {
        window.location.href = `/${form.agent_id}/chat`
      }, 500)
    } catch {
      addToast('warning', 'Agent created but failed to start. Update settings and save to retry.')
      window.location.href = `/${form.agent_id}/config`
    }
  }

  return (
    <>
      <div className="main-header">
        <h3 style={{ margin: 0 }}>Create New Agent</h3>
      </div>

      <div className="main-body" style={{ padding: 0 }}>
        <AgentForm
          mode="create"
          onSubmit={handleSubmit}
          onCancel={() => navigate('/')}
        />
      </div>
    </>
  )
}
