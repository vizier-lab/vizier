import { useEffect, useState } from 'react'
import { useNavigate } from 'react-router'
import Avatar from './avatar'
import { Skeleton } from './Skeleton'
import { listMemories, listTasks, listAgentSkills, listTopics } from '../services/vizier'
import type { Agent } from '../interfaces/types'

interface AgentCardProps {
  agent: Agent
}

interface AgentStats {
  memories: number
  tasks: number
  activeTasks: number
  skills: number
  topics: number
}

export default function AgentCard({ agent }: AgentCardProps) {
  const navigate = useNavigate()
  const [stats, setStats] = useState<AgentStats | null>(null)
  const [loading, setLoading] = useState(true)

  useEffect(() => {
    let cancelled = false
    const load = async () => {
      try {
        const [memRes, allTasks, activeTasks, skills, topics] = await Promise.allSettled([
          listMemories(agent.agent_id, { limit: 1 }),
          listTasks(agent.agent_id),
          listTasks(agent.agent_id, true),
          listAgentSkills(agent.agent_id),
          listTopics(agent.agent_id),
        ])
        if (cancelled) return
        setStats({
          memories: memRes.status === 'fulfilled' ? (memRes.value as any).total ?? 0 : 0,
          tasks: allTasks.status === 'fulfilled' ? (Array.isArray(allTasks.value) ? allTasks.value.length : 0) : 0,
          activeTasks: activeTasks.status === 'fulfilled' ? (Array.isArray(activeTasks.value) ? activeTasks.value.length : 0) : 0,
          skills: skills.status === 'fulfilled' ? (Array.isArray(skills.value) ? skills.value.length : 0) : 0,
          topics: topics.status === 'fulfilled' ? (Array.isArray(topics.value) ? topics.value.length : 0) : 0,
        })
      } catch {
        // silent
      } finally {
        if (!cancelled) setLoading(false)
      }
    }
    load()
    return () => { cancelled = true }
  }, [agent.agent_id])

  return (
    <div
      className="card"
      onClick={() => navigate(`/${agent.agent_id}/chat/General`)}
      style={{ cursor: 'pointer', display: 'flex', flexDirection: 'column', gap: '1rem' }}
    >
      <div style={{ display: 'flex', alignItems: 'center', gap: '0.75rem' }}>
        <Avatar name={agent.agent_id} size="md" avatarUrl={agent.avatar_url} />
        <div style={{ minWidth: 0, flex: 1 }}>
          <div style={{ fontWeight: 600, fontSize: '1rem', overflow: 'hidden', textOverflow: 'ellipsis', whiteSpace: 'nowrap' }}>
            {agent.name}
          </div>
          {agent.description && (
            <div style={{ color: 'var(--text-secondary)', fontSize: '0.8rem', overflow: 'hidden', textOverflow: 'ellipsis', whiteSpace: 'nowrap' }}>
              {agent.description}
            </div>
          )}
        </div>
      </div>

      <div style={{ display: 'grid', gridTemplateColumns: '1fr 1fr', gap: '0.5rem 1rem', fontSize: '0.8rem', color: 'var(--text-secondary)' }}>
        {loading ? (
          <>
            <Skeleton variant="text" width="60%" />
            <Skeleton variant="text" width="60%" />
            <Skeleton variant="text" width="60%" />
            <Skeleton variant="text" width="60%" />
          </>
        ) : stats && (
          <>
            <StatRow icon="📝" label="Memories" value={stats.memories} />
            <StatRow icon="💬" label="Topics" value={stats.topics} />
            <StatRow icon="✅" label="Tasks" value={`${stats.activeTasks}/${stats.tasks}`} />
            <StatRow icon="🧠" label="Skills" value={stats.skills} />
          </>
        )}
      </div>
    </div>
  )
}

function StatRow({ icon, label, value }: { icon: string; label: string; value: number | string }) {
  return (
    <div style={{ display: 'flex', alignItems: 'center', gap: '0.4rem' }}>
      <span>{icon}</span>
      <span>{value}</span>
      <span style={{ opacity: 0.7 }}>{label}</span>
    </div>
  )
}

export function NewAgentCard() {
  const navigate = useNavigate()

  return (
    <div
      onClick={() => navigate('/agents/new')}
      style={{
        cursor: 'pointer',
        display: 'flex',
        flexDirection: 'column',
        alignItems: 'center',
        justifyContent: 'center',
        gap: '0.75rem',
        minHeight: 180,
        border: '2px dashed var(--border)',
        borderRadius: 12,
        background: 'transparent',
        color: 'var(--text-secondary)',
        transition: 'all var(--transition-base)',
      }}
      onMouseEnter={(e) => {
        e.currentTarget.style.borderColor = 'var(--accent-primary)'
        e.currentTarget.style.color = 'var(--accent-primary)'
        e.currentTarget.style.background = 'var(--accent-light)'
      }}
      onMouseLeave={(e) => {
        e.currentTarget.style.borderColor = 'var(--border)'
        e.currentTarget.style.color = 'var(--text-secondary)'
        e.currentTarget.style.background = 'transparent'
      }}
    >
      <div style={{ fontSize: '2rem', lineHeight: 1 }}>+</div>
      <div style={{ fontSize: '0.9rem', fontWeight: 500 }}>New Agent</div>
    </div>
  )
}
