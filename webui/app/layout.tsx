import { useEffect, useState } from 'react'
import { Outlet, useNavigate, useParams, useLocation } from 'react-router'
import { listAgents, listTopics, deleteTopic } from './services/vizier'
import { FiSettings, FiMessageCircle, FiCheckCircle, FiLogOut, FiSearch, FiTrash2 } from 'react-icons/fi'
import { FaBook, FaFile } from 'react-icons/fa'
import Avatar from './components/avatar'
import ThemeToggle from './components/ThemeToggle'
import ToastContainer from './components/Toast'
import { useToastStore } from './hooks/toastStore'
import type { Agent, Topic } from './interfaces/types'

export default function Layout() {
  const [agents, setAgents] = useState<Agent[]>([])
  const [topics, setTopics] = useState<Topic[]>([])
  const [loading, setLoading] = useState(true)
  const navigate = useNavigate()
  const params = useParams()
  const location = useLocation()
  const { addToast } = useToastStore()

  const currentAgentId = params.agentId
  const currentTopicId = params.topicId
  const [previousTopicId, setPreviousTopicId] = useState<string | undefined>()

  // Check auth
  useEffect(() => {
    const token = localStorage.getItem('auth_token')
    if (!token) {
      navigate('/login')
    }
  }, [navigate])

  // Load agents
  useEffect(() => {
    const loadAgents = async () => {
      try {
        const response = await listAgents()
        setAgents(response.data || [])
        setLoading(false)
      } catch (error) {
        console.error('Failed to load agents:', error)
        setLoading(false)
      }
    }
    loadAgents()
  }, [])

  // Load topics when agent changes or when navigating away from /new
  useEffect(() => {
    if (!currentAgentId) return

    const loadTopics = async () => {
      try {
        const response = await listTopics(currentAgentId)
        const topicsList = response.data || []

        // If we just created a new topic (navigating from 'new' to a real topic),
        // add it optimistically if it's not in the list yet
        if (previousTopicId === 'new' && currentTopicId && currentTopicId !== 'new') {
          const topicExists = topicsList.some(t => t.topic_id === currentTopicId)
          if (!topicExists) {
            topicsList.push({
              topic_id: currentTopicId,
              title: currentTopicId, // Use topic_id as title until backend provides it
              created_at: new Date().toISOString(),
            } as Topic)
          }
        }

        setTopics(topicsList)
      } catch (error) {
        console.error('Failed to load topics:', error)
      }
    }

    loadTopics()

    // Update previous topic
    setPreviousTopicId(currentTopicId)
  }, [currentAgentId, currentTopicId, previousTopicId])

  const handleLogout = () => {
    localStorage.removeItem('auth_token')
    navigate('/login')
  }

  const handleDeleteTopic = async (e: React.MouseEvent, topicId: string) => {
    e.stopPropagation()
    if (!currentAgentId) return
    if (!confirm('Are you sure you want to delete this topic?')) return

    try {
      await deleteTopic(currentAgentId, topicId)
      addToast('success', 'Topic deleted successfully')
      const response = await listTopics(currentAgentId)
      setTopics(response.data || [])
      if (currentTopicId === topicId) {
        navigate(`/${currentAgentId}/chat/new`)
      }
    } catch (error: any) {
      console.error('Failed to delete topic:', error)
      addToast('error', 'Failed to delete topic', error.response?.data?.message || 'Please try again')
    }
  }

  const getCurrentView = () => {
    if (location.pathname.includes('/memory')) return 'memory'
    if (location.pathname.includes('/tasks')) return 'tasks'
    if (location.pathname.includes('/documents')) return 'documents'
    if (location.pathname.includes('/settings')) return 'settings'
    if (location.pathname.includes('/chat')) return 'chat'
    return null
  }

  const currentView = getCurrentView()

  if (loading) {
    return (
      <div style={{
        display: 'flex',
        alignItems: 'center',
        justifyContent: 'center',
        height: '100vh',
        color: 'var(--text-tertiary)',
      }}>
        <div className="flex flex-col items-center gap-4">
          <div className="w-12 h-12 rounded-xl bg-gradient-to-br from-emerald-400 to-cyan-500 animate-pulse" />
          <p>Loading Vizier...</p>
        </div>
      </div>
    )
  }

  return (
    <div className="layout-container">
      <ToastContainer />

      {/* Workspace sidebar (left) - Agent selector with settings and layout at bottom */}
      <div className="workspace-sidebar">
        <div className="workspace-items">
          {agents.map((agent) => (
            <div
              key={agent.agent_id}
              className={`workspace-item ${currentAgentId === agent.agent_id ? 'active' : ''}`}
              onClick={() => {
                // Navigate to chat with first topic, or show empty state
                if (topics.length > 0) {
                  navigate(`/${agent.agent_id}/chat/${topics[0].topic_id}`)
                } else {
                  navigate(`/${agent.agent_id}/chat/new`)
                }
              }}
              title={agent.name}
            >
              <Avatar name={agent.agent_id} rounded={false} />
            </div>
          ))}
        </div>

        {/* Bottom workspace controls */}
        <div className="workspace-bottom">
          <ThemeToggle />
          <div
            className={`workspace-item ${currentView === 'settings' ? 'active' : ''}`}
            onClick={() => navigate(currentAgentId ? `/${currentAgentId}/settings` : '/settings')}
            title="Settings"
          >
            <FiSettings size={20} />
          </div>
          <div
            className="workspace-item"
            onClick={handleLogout}
            title="Logout"
          >
            <FiLogOut size={20} />
          </div>
        </div>
      </div>

      {/* Navigation sidebar (middle) - Topics and navigation */}
      {currentAgentId && (
        <div className="nav-sidebar">
          <div className="nav-header">
            <div className='flex flex-col'>
              <span>{agents.find(a => a.agent_id === currentAgentId)?.name || currentAgentId}</span>
              <div className='opacity-50 m-2 text-xs font-mono font-extralight'>{currentAgentId}</div>
            </div>
          </div>

          <div className="nav-content">
            {/* Search for memories - only show on memory page */}
            {currentView === 'memory' && (
              <div style={{ marginBottom: '16px' }}>
              </div>
            )}

            {/* Tools section (moved above topics) */}
            <div className="nav-section">
              <div className="nav-section-title">Tools</div>
              <div
                className={`nav-item ${currentView === 'memory' ? 'active' : ''}`}
                onClick={() => navigate(`/${currentAgentId}/memory`)}
              >
                <FaBook size={16} />
                <span>Memory</span>
              </div>
              <div
                className={`nav-item ${currentView === 'tasks' ? 'active' : ''}`}
                onClick={() => navigate(`/${currentAgentId}/tasks`)}
              >
                <FiCheckCircle size={16} />
                <span>Tasks</span>
              </div>
              <div
                className={`nav-item ${currentView === 'documents' ? 'active' : ''}`}
                onClick={() => navigate(`/${currentAgentId}/documents`)}
              >
                <FaFile size={16} />
                <span>Documents</span>
              </div>
            </div>

            <div className="divider" />

            {/* Topics section - now showing only topic_id/slug */}
            <div className="nav-section">
              <div className="nav-section-title">Topics</div>
              {topics.map((topic) => (
                <div
                  key={topic.topic_id}
                  className={`nav-item group ${currentTopicId === topic.topic_id ? 'active' : ''}`}
                  onClick={() => navigate(`/${currentAgentId}/chat/${topic.topic_id}`)}
                  title={topic.title}
                >
                  <FiMessageCircle size={16} />
                  <span className="flex-1 truncate">{topic.topic_id}</span>
                  <button
                    className="opacity-0 group-hover:opacity-100 p-1 hover:text-red-500 transition-opacity"
                    onClick={(e) => handleDeleteTopic(e, topic.topic_id)}
                    title="Delete topic"
                  >
                    <FiTrash2 size={14} />
                  </button>
                </div>
              ))}
              <div
                className="nav-item"
                onClick={() => navigate(`/${currentAgentId}/chat/new`)}
                style={{ color: 'var(--text-tertiary)' }}
              >
                <span style={{ fontSize: '18px', lineHeight: '1' }}>+</span>
                <span>New Topic</span>
              </div>
            </div>
          </div>
        </div>
      )
      }

      {/* Main content area */}
      <div className="main-content">
        <Outlet />
      </div>
    </div >
  )
}
