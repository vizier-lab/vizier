import { Button, Icon, IconButton } from '@vizier/design'
import { useEffect, useState } from 'react'
import { transport } from '../../api/transport'
import { canUseProjects } from '../../lib/access'
import { useAgents } from '../../stores/agents'
import { projectKey, useProjects } from '../../stores/projects'
import { OpenProjectDialog } from './OpenProjectDialog'
import { ProjectRow } from './ProjectRow'

/** Whether the sidebar's section is shown at all (FR-005): a permitted viewer with projects. */
export function projectsSectionVisible(
    agent: { owner_id?: string } | undefined,
    projects: unknown[]
): boolean {
    return !!agent && canUseProjects(agent) && projects.length > 0
}

/**
 * The Projects section above Sessions in the agent sidebar (FR-005): hidden when the person
 * lacks a project permission or the agent has no projects. Only the desktop app can create one.
 */
export function ProjectsSection({ agentId }: { agentId: string }) {
    const agent = useAgents((s) => s.agents.find((a) => a.agent_id === agentId))
    const projects = useProjects((s) => s.byAgent[agentId] ?? [])
    const sessions = useProjects((s) => s.sessions)
    const refresh = useProjects((s) => s.refresh)
    const loadSessions = useProjects((s) => s.loadSessions)
    const start = useProjects((s) => s.start)
    const [expanded, setExpanded] = useState<Record<string, boolean>>({})
    const [creating, setCreating] = useState(false)
    const allowed = !!agent && canUseProjects(agent)

    useEffect(() => {
        if (!allowed) return
        void refresh(agentId)
        return start(agentId)
    }, [agentId, allowed, refresh, start])

    // Every listed project's sessions, so the rows can show them without another click.
    useEffect(() => {
        if (!allowed) return
        for (const project of projects) void loadSessions(agentId, project.id)
    }, [agentId, allowed, projects, loadSessions])

    if (!allowed || projects.length === 0) return null

    return (
        <section aria-label="Projects" className="flex flex-col gap-8 px-10">
            <div className="flex items-center justify-between gap-8">
                <span className="type-mono-xs text-faint">Projects</span>
                {transport.authMode === 'desktop' && (
                    <IconButton
                        size="sm"
                        variant="ghost"
                        icon="plus"
                        aria-label="Open a project"
                        title="Open a project"
                        onClick={() => setCreating(true)}
                    />
                )}
            </div>
            <div className="flex flex-col gap-8">
                {projects.map((project) => (
                    <ProjectRow
                        key={project.id}
                        agentId={agentId}
                        project={project}
                        sessions={sessions[projectKey(agentId, project.id)] ?? []}
                        expanded={!!expanded[project.id]}
                        onToggle={() =>
                            setExpanded((cur) => ({ ...cur, [project.id]: !cur[project.id] }))
                        }
                    />
                ))}
            </div>
            <OpenProjectDialog agentId={agentId} open={creating} onClose={() => setCreating(false)} />
        </section>
    )
}

/** The empty-conversation prompt: a desktop person with no projects is offered one. */
export function OpenProjectButton({ agentId }: { agentId: string }) {
    const [open, setOpen] = useState(false)
    if (transport.authMode !== 'desktop') return null
    return (
        <>
            <Button variant="primary" icon="folder" onClick={() => setOpen(true)}>
                Open a Project
            </Button>
            <OpenProjectDialog agentId={agentId} open={open} onClose={() => setOpen(false)} />
        </>
    )
}

/** The standard refused state for a direct project URL (FR-018). */
export function ProjectsRefused() {
    return (
        <div className="mx-auto flex max-w-3xl flex-col items-center gap-8 px-24 py-48 text-center">
            <Icon name="lock" size={22} tone="faint" />
            <h2 className="type-title text-fg">Projects are not available to you</h2>
            <p className="type-body text-muted">
                You do not have a project permission for this agent.
            </p>
        </div>
    )
}
