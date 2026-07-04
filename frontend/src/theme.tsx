import { createContext, useContext, useEffect, useState, ReactNode } from 'react'

export type ThemeMode = 'light' | 'dark' | 'system'

interface ThemeContextType {
  theme: ThemeMode
  setTheme: (mode: ThemeMode) => void
  effectiveTheme: 'light' | 'dark'
}

const ThemeContext = createContext<ThemeContextType | undefined>(undefined)

export function ThemeProvider({ children }: { children: ReactNode }) {
  const [theme, setThemeState] = useState<ThemeMode>(() => {
    const saved = localStorage.getItem('panorama_theme') as ThemeMode
    if (saved === 'light' || saved === 'dark' || saved === 'system') return saved
    return 'system'
  })

  const [effectiveTheme, setEffectiveTheme] = useState<'light' | 'dark'>('dark')

  const setTheme = (mode: ThemeMode) => {
    setThemeState(mode)
    localStorage.setItem('panorama_theme', mode)
  }

  useEffect(() => {
    const root = document.documentElement

    const apply = () => {
      let active: 'light' | 'dark' = 'dark'
      if (theme === 'light') {
        active = 'light'
      } else if (theme === 'dark') {
        active = 'dark'
      } else {
        // system mode
        const systemPrefersDark = window.matchMedia('(prefers-color-scheme: dark)').matches
        active = systemPrefersDark ? 'dark' : 'light'
      }

      root.setAttribute('data-theme', theme)
      root.setAttribute('data-effective-theme', active)
      setEffectiveTheme(active)
    }

    apply()

    if (theme === 'system') {
      const mediaQuery = window.matchMedia('(prefers-color-scheme: dark)')
      const handleChange = () => apply()
      mediaQuery.addEventListener('change', handleChange)
      return () => mediaQuery.removeEventListener('change', handleChange)
    }
  }, [theme])

  return (
    <ThemeContext.Provider value={{ theme, setTheme, effectiveTheme }}>
      {children}
    </ThemeContext.Provider>
  )
}

export function useTheme() {
  const context = useContext(ThemeContext)
  if (!context) {
    throw new Error('useTheme must be used within a ThemeProvider')
  }
  return context
}

export function ThemeSwitcher() {
  const { theme, setTheme } = useTheme()

  return (
    <div className="theme-switcher-root">
      <div className="theme-switcher-label">Theme</div>
      <div className="theme-switcher-group">
        <button
          type="button"
          className={`theme-btn ${theme === 'light' ? 'active' : ''}`}
          onClick={() => setTheme('light')}
          title="Light Theme"
          aria-label="Light Theme"
        >
          ☀️ <span className="theme-btn-text">Light</span>
        </button>
        <button
          type="button"
          className={`theme-btn ${theme === 'dark' ? 'active' : ''}`}
          onClick={() => setTheme('dark')}
          title="Dark Theme"
          aria-label="Dark Theme"
        >
          🌙 <span className="theme-btn-text">Dark</span>
        </button>
        <button
          type="button"
          className={`theme-btn ${theme === 'system' ? 'active' : ''}`}
          onClick={() => setTheme('system')}
          title="System Theme"
          aria-label="System Theme"
        >
          💻 <span className="theme-btn-text">System</span>
        </button>
      </div>
    </div>
  )
}
