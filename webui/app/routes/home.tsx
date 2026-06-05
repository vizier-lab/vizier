import { useThemeStore } from '../hooks/themeStore'
import { useAgentStore } from '../hooks/agentStore'
import Avatar from '../components/avatar'
import { Skeleton } from '../components/Skeleton'
import { useNavigate } from 'react-router'
import type { Route } from './+types/home'

export function meta({ }: Route.MetaArgs) {
  const hostname = typeof window !== 'undefined' ? window.location.hostname : 'Vizier'
  return [
    { title: `Vizier - ${hostname}` },
    { name: 'description', content: '21st Century Digital Steward' },
  ]
}

export default function Home() {
  const resolvedTheme = useThemeStore((state) => state.resolvedTheme)
  const { agents, loading } = useAgentStore()
  const navigate = useNavigate()

  return (
    <>
      <div className="main-header">
        <h3 style={{ margin: 0 }}>Home</h3>
      </div>

      <div className="main-body" style={{ display: 'flex', flexDirection: 'column', alignItems: 'center', justifyContent: 'center' }}>
        {loading ? (
          <div style={{ display: 'flex', justifyContent: 'center', gap: '2rem' }}>
            {[1, 2, 3].map((i) => (
              <div key={i} style={{ display: 'flex', flexDirection: 'column', alignItems: 'center', gap: '0.75rem', width: 160 }}>
                <Skeleton variant="circular" width={96} height={96} />
                <Skeleton variant="text" width={80} height={16} />
                <Skeleton variant="text" width="100%" />
              </div>
            ))}
          </div>
        ) : agents.length === 0 ? (
          <div style={{
            display: 'flex',
            alignItems: 'center',
            justifyContent: 'center',
            flexDirection: 'column',
            gap: '1.5rem',
          }}>
            <img src={`/vizier-logo-${resolvedTheme}.svg`} alt="Vizier" style={{ height: '64px' }} />
            <p style={{ color: 'var(--text-secondary)', fontSize: '1.1rem' }}>
              No agents yet. Create your first agent to get started.
            </p>
            <NewAgentCard onClick={() => navigate('/agents/new')} />
          </div>
        ) : (
          <>
            <div style={{
              display: 'flex',
              justifyContent: 'center',
              gap: '2rem',
              flexWrap: 'wrap',
            }}>
              {agents.map((agent) => (
                <div
                  key={agent.agent_id}
                  onClick={() => navigate(`/${agent.agent_id}/chat/General`)}
                  style={{
                    cursor: 'pointer',
                    display: 'flex',
                    flexDirection: 'column',
                    alignItems: 'center',
                    gap: '0.5rem',
                    width: 160,
                    transition: 'transform 0.15s ease',
                  }}
                  onMouseEnter={(e) => { e.currentTarget.style.transform = 'scale(1.05)' }}
                  onMouseLeave={(e) => { e.currentTarget.style.transform = 'scale(1)' }}
                >
                  <Avatar name={agent.agent_id} size="xl" avatarUrl={agent.avatar_url} />
                  <span style={{ fontSize: '0.9rem', fontWeight: 600, textAlign: 'center' }}>
                    {agent.name}
                  </span>
                  {agent.description && (
                    <span style={{
                      fontSize: '0.75rem',
                      color: 'var(--text-secondary)',
                      textAlign: 'center',
                      overflow: 'hidden',
                      textOverflow: 'ellipsis',
                      display: '-webkit-box',
                      WebkitLineClamp: 2,
                      WebkitBoxOrient: 'vertical',
                      lineHeight: '1.3',
                    }}>
                      {agent.description}
                    </span>
                  )}
                </div>
              ))}
              <NewAgentCard onClick={() => navigate('/agents/new')} />
            </div>
            <p style={{ color: 'var(--text-tertiary)', fontSize: '0.85rem', marginTop: '1.5rem' }}>
              Select an agent to start
            </p>
          </>
        )}
      </div>
    </>
  )
}

function NewAgentCard({ onClick }: { onClick: () => void }) {
  return (
    <div
      onClick={onClick}
      style={{
        cursor: 'pointer',
        display: 'flex',
        flexDirection: 'column',
        alignItems: 'center',
        gap: '0.5rem',
        width: 160,
        transition: 'transform 0.15s ease',
      }}
      onMouseEnter={(e) => { e.currentTarget.style.transform = 'scale(1.05)' }}
      onMouseLeave={(e) => { e.currentTarget.style.transform = 'scale(1)' }}
    >
      <div style={{
        width: 96,
        height: 96,
        borderRadius: 12,
        border: '2px dashed var(--border)',
        display: 'flex',
        alignItems: 'center',
        justifyContent: 'center',
        fontSize: '2rem',
        color: 'var(--text-secondary)',
        transition: 'border-color var(--transition-base), color var(--transition-base)',
      }}
        onMouseEnter={(e) => {
          e.currentTarget.style.borderColor = 'var(--accent-primary)'
          e.currentTarget.style.color = 'var(--accent-primary)'
        }}
        onMouseLeave={(e) => {
          e.currentTarget.style.borderColor = 'var(--border)'
          e.currentTarget.style.color = 'var(--text-secondary)'
        }}
      >+</div>
      <span style={{ fontSize: '0.8rem', color: 'var(--text-secondary)' }}>New Agent</span>
    </div>
  )
}
