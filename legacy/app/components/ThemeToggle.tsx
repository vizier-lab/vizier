import { FaSun, FaMoon } from 'react-icons/fa6'
import { useThemeStore } from '../hooks/themeStore'

interface ThemeToggleProps {
  showLabel?: boolean
}

export default function ThemeToggle({ showLabel = false }: ThemeToggleProps) {
  const { theme, toggleTheme } = useThemeStore()

  return (
    <button
      onClick={toggleTheme}
      className="theme-toggle"
      title={`Switch to ${theme === 'dark' ? 'light' : 'dark'} mode`}
    >
      <FaSun className="theme-icon-light" size={20} />
      <FaMoon className="theme-icon-dark" size={20} />
      {showLabel && <span className="theme-label">Theme</span>}
    </button>
  )
}
