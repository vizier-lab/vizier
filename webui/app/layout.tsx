import { useEffect, useState, useRef } from 'react'
import { Outlet, useNavigate, useParams, useLocation } from 'react-router'
import { listAgents } from './services/vizier'
import { FiSettings, FiCheckCircle, FiLogOut, FiTrendingUp, FiChevronDown, FiChevronLeft, FiMessageSquare, FiSun, FiMoon } from 'react-icons/fi'
import { FaBook, FaFolder } from 'react-icons/fa'
import Avatar from './components/avatar'
import ToastContainer from './components/Toast'
import { useConnectionStore } from './hooks/connectionStore'
import { useSidebarStore } from './hooks/sidebarStore'
import { useThemeStore } from './hooks/themeStore'
import type { Agent } from './interfaces/types'

export default function Layout() {
  const [agents, setAgents] = useState<Agent[]>([])
  const [loading, setLoading] = useState(true)
  const [showAgentDropdown, setShowAgentDropdown] = useState(false)
  const navigate = useNavigate()
  const params = useParams()
  const location = useLocation()
  const agentCardRef = useRef<HTMLDivElement>(null)

  const currentAgentId = params.agentId
  const currentTopicId = params.topicId

  const { connected, connect, disconnect } = useConnectionStore()
  const { collapsed, toggleSidebar } = useSidebarStore()
  const { theme, toggleTheme } = useThemeStore()

  // Check auth
  useEffect(() => {
    const token = localStorage.getItem('auth_token')
    if (!token) {
      disconnect()
      navigate('/login')
    }
  }, [navigate, disconnect])

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

  // WebSocket lifecycle — connect when on a chat route, keep alive on other routes
  useEffect(() => {
    if (!currentAgentId || !currentTopicId) return

    const token = localStorage.getItem('auth_token')
    if (!token) return

    connect(currentAgentId, currentTopicId)
  }, [currentAgentId, currentTopicId, connect])

  // Disconnect on agent change (switching to a different agent)
  useEffect(() => {
    return () => {
      // Only disconnect when the agent actually changes, not on every re-render
    }
  }, [currentAgentId])

  // Close dropdown on outside click
  useEffect(() => {
    if (!showAgentDropdown) return
    const handleClick = (e: MouseEvent) => {
      if (agentCardRef.current && !agentCardRef.current.contains(e.target as Node)) {
        setShowAgentDropdown(false)
      }
    }
    document.addEventListener('mousedown', handleClick)
    return () => document.removeEventListener('mousedown', handleClick)
  }, [showAgentDropdown])

  const handleLogout = () => {
    disconnect()
    localStorage.removeItem('auth_token')
    navigate('/login')
  }

  const handleSelectAgent = (agentId: string) => {
    setShowAgentDropdown(false)
    disconnect()
    navigate(`/${agentId}/chat`)
  }

  const getCurrentView = () => {
    if (location.pathname.includes('/memory')) return 'memory'
    if (location.pathname.includes('/tasks')) return 'tasks'
    if (location.pathname.includes('/documents')) return 'documents'
    if (location.pathname.includes('/usage')) return 'usage'
    if (location.pathname.includes('/settings')) return 'settings'
    if (location.pathname.includes('/chat')) return 'chat'
    return null
  }

  const currentView = getCurrentView()
  const currentAgent = agents.find(a => a.agent_id === currentAgentId)

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

      {/* Single sidebar */}
      <div className={`nav-sidebar ${collapsed ? 'collapsed' : ''}`}>
        {/* Agent card with dropdown */}
        <div className="agent-card-wrapper" ref={agentCardRef}>
          <div
            className="agent-card"
            onClick={() => setShowAgentDropdown(!showAgentDropdown)}
            title={collapsed && currentAgent ? currentAgent.name : undefined}
          >
            {currentAgent ? (
              <>
                <Avatar
                  name={currentAgent.agent_id}
                  rounded={false}
                  size="sm"
                  showStatus
                  online={connected}
                />
                <div className="agent-card-info">
                  <span className="agent-card-name">{currentAgent.name}</span>
                  <span className="agent-card-id">{currentAgent.agent_id}</span>
                </div>
              </>
            ) : (
              <>
                <Avatar name="empty" rounded={false} size="sm" />
                <span className="agent-card-placeholder">Select an agent</span>
              </>
            )}
            <FiChevronDown size={18} className={`agent-card-chevron ${showAgentDropdown ? 'open' : ''}`} />
          </div>

          {showAgentDropdown && (
            <div className={`agent-dropdown ${collapsed ? 'agent-dropdown-collapsed' : ''}`}>
              {agents.map((agent) => (
                <div
                  key={agent.agent_id}
                  className={`agent-dropdown-item ${currentAgentId === agent.agent_id ? 'active' : ''}`}
                  onClick={() => handleSelectAgent(agent.agent_id)}
                >
                  <Avatar name={agent.agent_id} rounded={false} size="sm" />
                  <div className="agent-dropdown-info">
                    <span className="agent-dropdown-name">{agent.name}</span>
                    <span className="agent-dropdown-id">{agent.agent_id}</span>
                  </div>
                </div>
              ))}
            </div>
          )}
        </div>

        {/* Tools section — only when agent is selected */}
        {currentAgentId && (
          <div className="nav-content">
            <div className="nav-section">
              <div
                className={`nav-item ${currentView === 'chat' ? 'active' : ''}`}
                onClick={() => navigate(`/${currentAgentId}/chat`)}
                title={collapsed ? 'Chat' : undefined}
              >
                <FiMessageSquare size={18} />
                <span>Chat</span>
              </div>
              <div
                className={`nav-item ${currentView === 'memory' ? 'active' : ''}`}
                onClick={() => navigate(`/${currentAgentId}/memory`)}
                title={collapsed ? 'Memory' : undefined}
              >
                <FaBook size={18} />
                <span>Memory</span>
              </div>
              <div
                className={`nav-item ${currentView === 'tasks' ? 'active' : ''}`}
                onClick={() => navigate(`/${currentAgentId}/tasks`)}
                title={collapsed ? 'Tasks' : undefined}
              >
                <FiCheckCircle size={18} />
                <span>Tasks</span>
              </div>
              <div
                className={`nav-item ${currentView === 'documents' ? 'active' : ''}`}
                onClick={() => navigate(`/${currentAgentId}/documents`)}
                title={collapsed ? 'Documents' : undefined}
              >
                <FaFolder size={18} />
                <span>Documents</span>
              </div>
              <div
                className={`nav-item ${currentView === 'usage' ? 'active' : ''}`}
                onClick={() => navigate(`/${currentAgentId}/usage`)}
                title={collapsed ? 'Usage' : undefined}
              >
                <FiTrendingUp size={18} />
                <span>Usage</span>
              </div>
            </div>
          </div>
        )}

        {/* Bottom: Toggle, Settings, Theme, Logout */}
        <div className="nav-bottom">
          <div
            className="nav-item sidebar-toggle"
            onClick={toggleSidebar}
            title={collapsed ? 'Expand sidebar' : 'Collapse sidebar'}
          >
            <FiChevronLeft
              size={18}
              style={{
                transition: 'transform 0.2s ease',
                transform: collapsed ? 'rotate(180deg)' : 'none',
              }}
            />
            <span>Collapse</span>
          </div>
          <div
            className={`nav-item ${currentView === 'settings' ? 'active' : ''}`}
            onClick={() => navigate(currentAgentId ? `/${currentAgentId}/settings` : '/settings')}
            title={collapsed ? 'Settings' : undefined}
          >
            <FiSettings size={18} />
            <span>Settings</span>
          </div>
          <div
            className="nav-item nav-theme-row"
            onClick={toggleTheme}
            title={collapsed ? `Switch to ${theme === 'dark' ? 'light' : 'dark'} mode` : undefined}
          >
            <FiSun className="theme-icon-light" size={18} />
            <FiMoon className="theme-icon-dark" size={18} />
            <span className="theme-label">Theme</span>
          </div>
          <div
            className="nav-item"
            onClick={handleLogout}
            title={collapsed ? 'Logout' : undefined}
          >
            <FiLogOut size={18} />
            <span>Logout</span>
          </div>
        </div>
      </div>

      {/* Main content area */}
      <div className="main-content">
        <Outlet />
      </div>
    </div>
  )
}
