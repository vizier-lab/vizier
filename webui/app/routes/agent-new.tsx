import { useNavigate } from 'react-router'
import { useToastStore } from '../hooks/toastStore'
import { createAgent } from '../services/vizier'
import AgentForm from '../components/AgentForm'
import type { CreateAgentRequest } from '../interfaces/types'

export default function AgentNew() {
  const navigate = useNavigate()
  const addToast = useToastStore((s) => s.addToast)

  const handleSubmit = async (form: CreateAgentRequest) => {
    await createAgent(form)
    addToast('success', `Agent "${form.name}" created`)
    setTimeout(() => {
      window.location.href = `/${form.agent_id}/chat`
    }, 500)
  }

  return (
    <>
      <div className="main-header">
        <h3 style={{ margin: 0 }}>Create New Agent</h3>
      </div>

      <AgentForm
        mode="create"
        onSubmit={handleSubmit}
        onCancel={() => navigate('/')}
      />
    </>
  )
}
