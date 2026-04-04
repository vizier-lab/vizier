import { FiSun, FiMoon } from 'react-icons/fi'
import { useThemeStore } from '../hooks/themeStore'

export default function ThemeToggle() {
  const { theme, setTheme, toggleTheme } = useThemeStore()

  return (
    <button
      onClick={toggleTheme}
      className="theme-toggle"
      title={`Switch to ${theme === 'dark' ? 'light' : 'dark'} mode`}
    >
      <FiSun className="theme-icon-light" size={18} />
      <FiMoon className="theme-icon-dark" size={18} />
    </button>
  )
}
