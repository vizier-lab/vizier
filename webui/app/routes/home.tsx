import { useEffect } from 'react'
import { useNavigate } from 'react-router'
import { useThemeStore } from '../hooks/themeStore'
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

  // Redirect to first agent when home page loads
  useEffect(() => {
    // Let the layout handle agent loading and redirection
    // User will see this page momentarily before layout redirects
  }, [])

  return (
    <div style={{
      display: 'flex',
      alignItems: 'center',
      justifyContent: 'center',
      height: '100%',
      flexDirection: 'column',
      gap: '1rem',
    }}>
      <img src={`/vizier-logo-${resolvedTheme}.svg`} alt="Vizier" style={{ height: '64px' }} />
      <p style={{ color: 'var(--text-secondary)' }}>
        Select an agent from the sidebar to begin
      </p>
    </div>
  )
}
