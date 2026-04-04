import { create } from 'zustand'
import { persist } from 'zustand/middleware'

type Theme = 'light' | 'dark' | 'system'

interface ThemeState {
  theme: Theme
  resolvedTheme: 'light' | 'dark'
  setTheme: (theme: Theme) => void
  toggleTheme: () => void
}

export const useThemeStore = create<ThemeState>()(
  persist(
    (set, get) => {
      const getResolvedTheme = (theme: Theme): 'light' | 'dark' => {
        if (theme === 'light') return 'light'
        if (theme === 'dark') return 'dark'
        return window.matchMedia('(prefers-color-scheme: dark)').matches ? 'dark' : 'light'
      }

      const applyTheme = (theme: Theme) => {
        const resolved = getResolvedTheme(theme)
        const root = document.documentElement
        root.classList.remove('light', 'dark')
        root.classList.add(resolved)
      }

      // Initialize theme on app load
      const storedTheme = JSON.parse(localStorage.getItem('theme') || '"system"') as Theme
      applyTheme(storedTheme)

      // Listen for system theme changes
      const mediaQuery = window.matchMedia('(prefers-color-scheme: dark)')
      const handleChange = () => {
        if (get().theme === 'system') {
          applyTheme('system')
          set({ resolvedTheme: getResolvedTheme('system') })
        }
      }
      mediaQuery.addEventListener('change', handleChange)

      return {
        theme: storedTheme,
        resolvedTheme: getResolvedTheme(storedTheme),
        setTheme: (theme: Theme) => {
          applyTheme(theme)
          set({ theme, resolvedTheme: getResolvedTheme(theme) })
        },
        toggleTheme: () => {
          const { theme } = get()
          const newTheme: Theme = theme === 'dark' ? 'light' : 'dark'
          applyTheme(newTheme)
          set({ theme: newTheme, resolvedTheme: getResolvedTheme(newTheme) })
        },
      }
    },
    {
      name: 'theme-storage',
      partialize: (state) => ({ theme: state.theme }),
    }
  )
)
