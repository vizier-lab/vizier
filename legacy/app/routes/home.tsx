import { useState, useMemo } from 'react'
import { useNavigate } from 'react-router'
import { FaPaperPlane } from 'react-icons/fa6'
import { useThemeStore } from '../hooks/themeStore'
import { useAgentStore } from '../hooks/agentStore'
import { useUserStore } from '../hooks/userStore'
import { useQuickChatStore } from '../hooks/quickChatStore'
import Avatar from '../components/avatar'
import MarkdownEditor from '../components/MarkdownEditor'
import { Skeleton } from '../components/Skeleton'
import type { Route } from './+types/home'

export function meta({ }: Route.MetaArgs) {
  const hostname = typeof window !== 'undefined' ? window.location.hostname : 'Vizier'
  return [
    { title: `Vizier - ${hostname}` },
    { name: 'description', content: '21st Century Digital Steward' },
  ]
}

const GREETINGS = [
  "What shall we conquer today?",
  "Awaiting your command.",
  "The digital realm is our canvas.",
  "What knowledge shall we seek?",
  "Ready to navigate the unknown.",
  "Your steward awaits.",
  "Where shall we venture?",
  "The archive is at your disposal.",
  "What mysteries shall we unravel?",
  "At your service, as always.",
  "Shall we shape something extraordinary?",
  "The stage is set. What's the mission?",
  "Your vision, my execution.",
  "What horizons shall we explore?",
  "Standing by for direction.",
  "Let's craft something remarkable.",
  "What shall we build today?",
  "The possibilities are endless.",
  "Your next move, Captain.",
  "How shall we bend the universe today?",
  "I've been preparing for this moment.",
  "What legacy shall we write?",
]

export default function Home() {
  const resolvedTheme = useThemeStore((state) => state.resolvedTheme)
  const { agents, loading, lastAgentId, setLastAgentId: setStoreLastAgentId } = useAgentStore()
  const { user } = useUserStore()
  const navigate = useNavigate()
  const [showAll, setShowAll] = useState(false)

  const selectedAgent = !showAll ? agents.find((a) => a.agent_id === lastAgentId) : null

  if (loading) {
    return (
      <>
        <div className="main-header">
          <h3 style={{ margin: 0 }}>Home</h3>
        </div>
        <div className="main-body" style={{ display: 'flex', flexDirection: 'column', alignItems: 'center', justifyContent: 'center' }}>
          <div style={{ display: 'flex', justifyContent: 'center', gap: '2rem' }}>
            {[1, 2, 3].map((i) => (
              <div key={i} style={{ display: 'flex', flexDirection: 'column', alignItems: 'center', gap: '0.75rem', width: 160 }}>
                <Skeleton variant="circular" width={96} height={96} />
                <Skeleton variant="text" width={80} height={16} />
                <Skeleton variant="text" width="100%" />
              </div>
            ))}
          </div>
        </div>
      </>
    )
  }

  if (agents.length === 0) {
    return (
      <>
        <div className="main-header">
          <h3 style={{ margin: 0 }}>Home</h3>
        </div>
        <div className="main-body" style={{ display: 'flex', flexDirection: 'column', alignItems: 'center', justifyContent: 'center' }}>
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
        </div>
      </>
    )
  }

  if (selectedAgent) {
    return <QuickChat agent={selectedAgent} onShowAll={() => setShowAll(true)} />
  }

  return (
    <>
      <div className="main-header">
        <h3 style={{ margin: 0 }}>Home</h3>
      </div>
      <div className="main-body" style={{ display: 'flex', flexDirection: 'column', alignItems: 'center', justifyContent: 'center' }}>
        <div style={{
          display: 'flex',
          justifyContent: 'center',
          gap: '2rem',
          flexWrap: 'wrap',
        }}>
          {agents.map((agent) => (
            <div
              key={agent.agent_id}
              onClick={() => {
                if (user?.user_id) {
                  localStorage.setItem(`vizier_last_agent_${user.user_id}`, agent.agent_id)
                }
                setStoreLastAgentId(agent.agent_id)
                setShowAll(false)
              }}
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
      </div>
    </>
  )
}

function QuickChat({ agent, onShowAll }: { agent: { agent_id: string; name: string; description?: string; avatar_url?: string }; onShowAll: () => void }) {
  const navigate = useNavigate()
  const [input, setInput] = useState('')
  const greeting = useMemo(() => GREETINGS[Math.floor(Math.random() * GREETINGS.length)], [])
  const setPendingMessage = useQuickChatStore((s) => s.setPendingMessage)

  const handleSubmit = (e: React.FormEvent) => {
    e.preventDefault()
    const text = input.trim()
    if (!text) return
    const topicId = `chat-${Date.now()}-${Math.random().toString(36).slice(2, 8)}`
    setPendingMessage(text)
    navigate(`/${agent.agent_id}/chat/${topicId}`)
  }

  return (
    <>
      <div className="main-header">
        <h3 style={{ margin: 0 }}>Home</h3>
      </div>
      <div className="main-body" style={{ display: 'flex', flexDirection: 'column', alignItems: 'center', justifyContent: 'center' }}>
        <Avatar name={agent.agent_id} size="xl" avatarUrl={agent.avatar_url} />
        {agent.description && (
          <span style={{ fontSize: '0.85rem', color: 'var(--text-secondary)', marginTop: '0.25rem', textAlign: 'center', maxWidth: 400 }}>
            {agent.description}
          </span>
        )}
        <h2 style={{ margin: '1.25rem 0 0', fontSize: '1.25rem', fontWeight: 500, color: 'var(--text-secondary)' }}>
          {greeting}
        </h2>

        <form onSubmit={handleSubmit} style={{ width: '100%', maxWidth: 600, marginTop: '2rem' }}>
          <div
            onKeyDown={(e) => {
              if (e.key === 'Enter' && (e.ctrlKey || e.metaKey)) {
                e.preventDefault()
                handleSubmit(e)
              }
            }}
            className="chat-input-container"
          >
            <MarkdownEditor
              value={input}
              onChange={setInput}
              placeholder="Say something..."
              className="chat-mdx-editor"
              hideToolbar
            />
            <div className="chat-input-bottom-bar">
              <div className="chat-input-bottom-row">
                <div className={`chat-keyboard-hint${input.trim() ? ' visible' : ''}`}>
                  <strong>Ctrl+Enter</strong> to send
                </div>
                <button
                  type="submit"
                  className={`chat-send-btn chat-send-btn-inline${input.trim() ? ' has-content' : ''}`}
                  disabled={!input.trim()}
                >
                  <FaPaperPlane size={14} />
                </button>
              </div>
            </div>
          </div>
        </form>

        <button
          onClick={onShowAll}
          style={{
            marginTop: '2rem',
            padding: '0.4rem 1rem',
            borderRadius: '0.5rem',
            border: '1px solid var(--border)',
            background: 'transparent',
            color: 'var(--text-secondary)',
            cursor: 'pointer',
            fontSize: '0.8rem',
          }}
        >
          Switch agent
        </button>
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
