import { useEffect, useState, useRef } from 'react'
import { createPortal } from 'react-dom'
import { Outlet, useNavigate, useParams, useLocation } from 'react-router'
import { FaRobot, FaGear, FaCircleCheck, FaRightFromBracket, FaArrowTrendUp, FaChevronDown, FaChevronLeft, FaComment, FaSun, FaMoon, FaBars, FaPlus, FaBook, FaWandMagicSparkles, FaHouse } from 'react-icons/fa6'
import Avatar from './components/avatar'
import ToastContainer from './components/Toast'
import { useConnectionStore } from './hooks/connectionStore'
import { useSidebarStore } from './hooks/sidebarStore'
import { useThemeStore } from './hooks/themeStore'
import { useAgentStore } from './hooks/agentStore'
import { useUserStore } from './hooks/userStore'
import { hasPermission } from './utils/auth'

export default function Layout() {
  const { agents, loading, loadAgents, lastAgentId, setLastAgentId: setStoreLastAgentId } = useAgentStore()
  const { user, loadUser } = useUserStore()
  const [showAgentDropdown, setShowAgentDropdown] = useState(false)
  const [dropdownRect, setDropdownRect] = useState<DOMRect | null>(null)
  const navigate = useNavigate()
  const params = useParams()
  const location = useLocation()
  const agentCardRef = useRef<HTMLDivElement>(null)
  const dropdownRef = useRef<HTMLDivElement>(null)

  const currentAgentId = params.agentId || lastAgentId
  const currentTopicId = params.topicId

  const { connected, connect, disconnect } = useConnectionStore()
  const { collapsed, toggleSidebar, mobileOpen, closeMobile } = useSidebarStore()
  const { toggleTheme } = useThemeStore()

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
    loadAgents()
  }, [loadAgents])

  // Load current user
  useEffect(() => {
    loadUser()
  }, [loadUser])

  // Initialize lastAgentId from localStorage
  useEffect(() => {
    if (!user?.user_id) return
    const stored = localStorage.getItem(`vizier_last_agent_${user.user_id}`)
    if (stored) setStoreLastAgentId(stored)
  }, [user?.user_id])

  // Sync URL agent param to store + localStorage
  useEffect(() => {
    if (params.agentId && user?.user_id) {
      localStorage.setItem(`vizier_last_agent_${user.user_id}`, params.agentId)
      setStoreLastAgentId(params.agentId)
    }
  }, [params.agentId, user?.user_id])

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
      const target = e.target as Node
      if (agentCardRef.current?.contains(target)) return
      if (dropdownRef.current?.contains(target)) return
      setShowAgentDropdown(false)
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
    if (user?.user_id) {
      localStorage.setItem(`vizier_last_agent_${user.user_id}`, agentId)
    }
    setStoreLastAgentId(agentId)
    navigate('/')
  }

  const getCurrentView = () => {
    if (location.pathname === '/') return 'home'
    if (location.pathname.includes('/memory')) return 'memory'
    if (location.pathname.includes('/tasks')) return 'tasks'
    if (location.pathname.includes('/skills')) return 'skills'
    if (location.pathname.includes('/usage')) return 'usage'
    if (location.pathname === '/settings') return 'global-settings'
    if (location.pathname.includes('/settings')) return 'settings'
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
            onClick={() => {
              if (!showAgentDropdown && agentCardRef.current) {
                setDropdownRect(agentCardRef.current.getBoundingClientRect())
              }
              setShowAgentDropdown(!showAgentDropdown)
            }}
          >
            {currentAgent ? (
              <>
                <Avatar
                  name={currentAgent.agent_id}
                  rounded={false}
                  size="sm"
                  showStatus
                  online={connected}
                  avatarUrl={currentAgent.avatar_url}
                />
                <div className="agent-card-info">
                  <span className="agent-card-name">{currentAgent.name}</span>
                  <span className="agent-card-id">
                    {currentAgent.agent_id}
                    {currentAgent.owner_username && (
                      <> <span className="agent-card-owner">· @{currentAgent.owner_username}</span></>
                    )}
                  </span>
                </div>
              </>
            ) : (
              <>
                <Avatar name="empty" rounded={false} size="sm" />
                <span className="agent-card-placeholder">Select an agent</span>
              </>
            )}
            <FaChevronDown size={18} className={`agent-card-chevron ${showAgentDropdown ? 'open' : ''}`} />
          </div>

          {showAgentDropdown && dropdownRect && createPortal(
            <div
              ref={dropdownRef}
              style={{
                position: 'fixed',
                left: collapsed ? dropdownRect.right + 4 : dropdownRect.left,
                top: collapsed ? dropdownRect.top : dropdownRect.bottom + 4,
                minWidth: collapsed ? 220 : dropdownRect.width,
                maxHeight: 320,
                overflowY: 'auto',
                background: 'var(--surface)',
                border: '1px solid var(--border)',
                borderRadius: 10,
                boxShadow: 'var(--shadow-lg)',
                zIndex: 1000,
              }}
              onClick={(e) => e.stopPropagation()}
            >
              {agents.map((agent) => (
                <div
                  key={agent.agent_id}
                  className={`agent-dropdown-item ${currentAgentId === agent.agent_id ? 'active' : ''}`}
                  onClick={() => handleSelectAgent(agent.agent_id)}
                >
                  <Avatar name={agent.agent_id} rounded={false} size="sm" avatarUrl={agent.avatar_url} />
                  <div className="agent-dropdown-info">
                    <span className="agent-dropdown-name">{agent.name}</span>
                    <span className="agent-dropdown-id">
                      {agent.agent_id}
                      {agent.owner_username && (
                        <> <span className="agent-dropdown-owner">@{agent.owner_username}</span></>
                      )}
                    </span>
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
                <FaPlus size={18} />
                <div className="agent-dropdown-info">
                  <span className="agent-dropdown-name">Create Agent</span>
                </div>
              </div>
            </div>,
            document.body
          )}
        </div>

        {/* Tools section */}
        <div className="nav-content">
          <div className="nav-section">
            {([
              ['home', 'Home', FaHouse],
              ['chat', 'Chat', FaComment],
              ['memory', 'Memory', FaBook],
              ['tasks', 'Tasks', FaCircleCheck],
              ['skills', 'Skills', FaWandMagicSparkles],
              ['usage', 'Usage', FaArrowTrendUp],
              ['settings', 'Agent Config', FaRobot],
            ] as const).map(([view, label, Icon]) => {
              const isHome = view === 'home'
              const isSettings = view === 'settings'
              const canEditAgent = hasPermission('all_agents:edit') || currentAgent?.owner_id === user?.user_id
              const isDisabled = !isHome && (!currentAgentId || (isSettings && !canEditAgent))

              return (
                <div
                  key={view}
                  className={`nav-item ${currentView === view ? 'active' : ''}`}
                  onClick={() => !isDisabled && handleNavClick(isHome ? '/' : `/${currentAgentId}/${view}`)}
                  style={{
                    ...(isDisabled ? { opacity: 0.4, cursor: 'not-allowed', pointerEvents: 'none' } : {}),
                  }}
                  title={isSettings && !canEditAgent ? 'Only the agent owner can edit config' : undefined}
                >
                  <Icon size={18} />
                  <span>{label}</span>
                </div>
              )
            })}
          </div>
        </div>

        {/* Bottom: Toggle, Settings, Theme, Logout */}
        <div className="nav-bottom">
          <div
            className="nav-item sidebar-toggle"
            onClick={toggleSidebar}
          >
            <FaChevronLeft
              size={18}
              style={{
                transition: 'transform 0.2s ease',
                transform: collapsed ? 'rotate(180deg)' : 'none',
              }}
            />
            <span>Collapse</span>
          </div>
          <div
            className={`nav-item ${currentView === 'global-settings' ? 'active' : ''}`}
            onClick={() => handleNavClick('/settings')}
          >
            <FaGear size={18} />
            <span>Settings</span>
          </div>
          <div
            className="nav-item nav-theme-row"
            onClick={toggleTheme}
          >
            <FaSun className="theme-icon-light" size={18} />
            <FaMoon className="theme-icon-dark" size={18} />
            <span className="theme-label">Theme</span>
          </div>
          <div
            className="nav-item"
            onClick={handleLogout}
          >
            <FaRightFromBracket size={18} />
            <span>Logout</span>
          </div>
        </div>
      </div>

      {/* Main content area */}
      <div className="main-content">
        {/* Mobile-only header bar with hamburger */}
        <div className="flex items-center px-4 py-2 border-b border-[var(--border)] bg-[var(--surface)] md:hidden">
          <button className="mobile-menu-btn" onClick={() => useSidebarStore.getState().toggleMobile()}>
            <FaBars size={22} />
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
