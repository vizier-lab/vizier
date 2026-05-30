import { useEffect, useState, useRef } from 'react'
import { Outlet, useNavigate, useParams, useLocation } from 'react-router'
import { listAgents, deleteAgent } from './services/vizier'
import { FiSettings, FiCheckCircle, FiLogOut, FiTrendingUp, FiChevronDown, FiChevronLeft, FiMessageSquare, FiSun, FiMoon, FiMenu, FiPlus, FiTrash2, FiEdit3, FiAlertTriangle } from 'react-icons/fi'
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
  const { collapsed, toggleSidebar, mobileOpen, closeMobile } = useSidebarStore()
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

  // Close mobile drawer on route change
  useEffect(() => {
    closeMobile()
  }, [location.pathname, closeMobile])

  const handleLogout = () => {
    disconnect()
    localStorage.removeItem('auth_token')
    navigate('/login')
  }

  const handleSelectAgent = (agentId: string) => {
    setShowAgentDropdown(false)
    closeMobile()
    disconnect()
    navigate(`/${agentId}/chat`)
  }

  const getCurrentView = () => {
    if (location.pathname.includes('/memory')) return 'memory'
    if (location.pathname.includes('/tasks')) return 'tasks'
    if (location.pathname.includes('/documents')) return 'documents'
    if (location.pathname.includes('/usage')) return 'usage'
    if (location.pathname.includes('/settings')) return 'settings'
    if (location.pathname.includes('/danger')) return 'danger'
    if (location.pathname.includes('/edit')) return 'edit'
    if (location.pathname.includes('/chat')) return 'chat'
    return null
  }

  const handleNavClick = (path: string) => {
    closeMobile()
    navigate(path)
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

      {/* Backdrop for mobile drawer */}
      <div
        className={`sidebar-backdrop ${mobileOpen ? 'visible' : ''}`}
        onClick={closeMobile}
      />

      {/* Single sidebar */}
      <div className={`nav-sidebar ${collapsed ? 'collapsed' : ''} ${mobileOpen ? 'mobile-open' : ''}`}>
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
              <div
                className="agent-dropdown-item"
                onClick={() => {
                  setShowAgentDropdown(false)
                  navigate('/agents/new')
                }}
                style={{ borderTop: '1px solid var(--border)', marginTop: '0.25rem', paddingTop: '0.5rem' }}
              >
                <FiPlus size={18} />
                <div className="agent-dropdown-info">
                  <span className="agent-dropdown-name">Create Agent</span>
                </div>
              </div>
            </div>
          )}
        </div>

        {/* Tools section */}
        <div className="nav-content">
          <div className="nav-section">
            {([
              ['chat', 'Chat', FiMessageSquare],
              ['memory', 'Memory', FaBook],
              ['tasks', 'Tasks', FiCheckCircle],
              ['documents', 'Documents', FaFolder],
              ['usage', 'Usage', FiTrendingUp],
              ['edit', 'Edit Agent', FiEdit3],
              ['danger', 'Danger Zone', FiAlertTriangle],
            ] as const).map(([view, label, Icon]) => (
              <div
                key={view}
                className={`nav-item ${currentView === view ? 'active' : ''}`}
                onClick={() => currentAgentId && handleNavClick(`/${currentAgentId}/${view}`)}
                title={collapsed ? label : undefined}
                style={{
                  ...(!currentAgentId ? { opacity: 0.4, cursor: 'not-allowed', pointerEvents: 'none' } : {}),
                  ...(view === 'danger' ? { color: currentView === 'danger' ? '#ef4444' : undefined } : {}),
                }}
              >
                <Icon size={18} />
                <span>{label}</span>
              </div>
            ))}
          </div>
        </div>

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
            onClick={() => handleNavClick('/settings')}
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
        {/* Mobile-only header bar with hamburger */}
        <div className="flex items-center px-4 py-2 border-b border-[var(--border)] bg-[var(--surface)] md:hidden">
          <button className="mobile-menu-btn" onClick={() => useSidebarStore.getState().toggleMobile()}>
            <FiMenu size={22} />
          </button>
          {currentAgent && (
            <span className="ml-3 font-semibold text-sm">{currentAgent.name}</span>
          )}
        </div>
        <Outlet />
      </div>
    </div>
  )
}
