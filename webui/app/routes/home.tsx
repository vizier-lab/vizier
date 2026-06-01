import { useEffect } from 'react'
import { useNavigate } from 'react-router'
import { useThemeStore } from '../hooks/themeStore'
import { useAgentStore } from '../hooks/agentStore'
import { useUserStore } from '../hooks/userStore'
import type { Route } from './+types/home'

export function meta({ }: Route.MetaArgs) {
  return [
    { title: 'Vizier' },
    { name: 'description', content: '21st Century Digital Steward' },
  ]
}

export default function Home() {
  const navigate = useNavigate()
  const resolvedTheme = useThemeStore((state) => state.resolvedTheme)
  const { agents, loading } = useAgentStore()
  const { user } = useUserStore()

  useEffect(() => {
    if (loading || !user?.user_id) return

    const lastAgentId = localStorage.getItem(`vizier_last_agent_${user.user_id}`)
    if (!lastAgentId) return

    const agentExists = agents.some((a) => a.agent_id === lastAgentId)
    if (!agentExists) {
      localStorage.removeItem(`vizier_last_agent_${user.user_id}`)
      return
    }

    const lastTopic = localStorage.getItem(`vizier_last_topic_${user.user_id}_${lastAgentId}`)
    navigate(`/${lastAgentId}/chat/${lastTopic || 'General'}`, { replace: true })
  }, [loading, agents, navigate, user?.user_id])

  return (
    <div style={{
      display: 'flex',
      alignItems: 'center',
      justifyContent: 'center',
      height: '100%',
      flexDirection: 'column',
      gap: '1.5rem',
    }}>
      <img src={`/vizier-logo-${resolvedTheme}.svg`} alt="Vizier" style={{ height: '64px' }} />
      <p style={{ color: 'var(--text-secondary)' }}>
        Select an agent from the sidebar to begin
      </p>
      <button
        onClick={() => navigate('/agents/new')}
        style={{
          padding: '0.6rem 1.5rem',
          borderRadius: '0.5rem',
          border: 'none',
          background: 'var(--accent-primary)',
          color: '#fff',
          cursor: 'pointer',
          fontSize: '0.9rem',
          fontWeight: 500,
        }}
      >
        Create New Agent
      </button>
    </div>
  )
}
