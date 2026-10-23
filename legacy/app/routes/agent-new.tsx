import { useNavigate } from 'react-router'
import { useToastStore } from '../hooks/toastStore'
import { isBuildFailure, useBuildStore } from '../hooks/buildStore'
import { createAgent } from '../services/vizier'
import AgentForm from '../components/AgentForm'
import type { CreateAgentRequest } from '../interfaces/types'

export default function AgentNew() {
  const navigate = useNavigate()
  const addToast = useToastStore((s) => s.addToast)
  const begin = useBuildStore((s) => s.begin)
  const ready = useBuildStore((s) => s.ready)
  const fail = useBuildStore((s) => s.fail)
  const close = useBuildStore((s) => s.close)

  const handleSubmit = async (form: CreateAgentRequest) => {
    // The modal shows at once, so submitting always gives visible feedback.
    const buildId = begin(form.agent_id, `Creating agent "${form.name}"`, () => {
      void handleSubmit(form)
    })
    try {
      const res = await createAgent(form)
      for (const w of res.data?.warnings || []) addToast('warning', w.message)
      addToast('success', `Agent "${form.name}" created`)
      ready(buildId)
      setTimeout(() => {
        window.location.href = `/${form.agent_id}/chat`
      }, 500)
    } catch (err) {
      const message =
        (err as { response?: { data?: { message?: string } } })?.response?.data?.message ??
        'The agent could not be created.'
      if (isBuildFailure(message)) {
        // The build ran and failed: report it in the modal, with Retry.
        fail(buildId, message)
      } else {
        // The request was refused before any build: retrying the same form can't help.
        close(buildId)
        addToast('error', message)
      }
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
