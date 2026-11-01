import { useEffect, useState } from 'react'
import { useNavigate, useParams } from 'react-router'
import { createProjectSession, listProjectSessions } from '../api/endpoints/projects'
import { canUseProjects } from '../lib/access'
import { mostRecentSession } from '../lib/projectSessions'
import { ProjectsRefused } from '../screens/projects/ProjectsSection'
import { useAgents } from '../stores/agents'
import { useProjects } from '../stores/projects'

/**
 * `/:agentId/projects/:projectId` opens the caller's most recent session in the project, or
 * starts one when they have none. The list is in creation order, so the session to open is picked
 * by `last_activity` rather than by being first. A person without the project permission sees the
 * refused state.
 */
export default function ProjectIndex() {
    const { agentId = '', projectId = '' } = useParams()
    const navigate = useNavigate()
    const agent = useAgents((s) => s.agents.find((a) => a.agent_id === agentId))
    const insertSession = useProjects((s) => s.insertSession)
    const [failed, setFailed] = useState(false)
    const allowed = !agent || canUseProjects(agent)

    useEffect(() => {
        if (!allowed) return
        let alive = true
        listProjectSessions(agentId, projectId)
            .then(async (sessions) => {
                if (!alive) return
                const existing = mostRecentSession(sessions ?? [])
                if (existing) {
                    navigate(`/${agentId}/projects/${projectId}/chat/${existing.topic}`, { replace: true })
                    return
                }
                const created = await createProjectSession(agentId, projectId)
                if (!alive) return
                insertSession(agentId, projectId, {
                    topic: created.topic,
                    title: created.title,
                    is_thinking: false,
                    last_activity: null,
                })
                navigate(`/${agentId}/projects/${projectId}/chat/${created.topic}`, { replace: true })
            })
            .catch(() => {
                if (alive) setFailed(true)
            })
        return () => {
            alive = false
        }
    }, [agentId, projectId, allowed, navigate, insertSession])

    if (!allowed || failed) return <ProjectsRefused />
    return <p className="px-24 py-48 type-body text-faint">Opening the project…</p>
}
