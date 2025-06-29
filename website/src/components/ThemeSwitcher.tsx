import { Moon, Sun } from 'lucide-react'
import { useTheme } from '~/contexts/ThemeContext'
import { IconButton } from '~/components/IconButton'
import { useEffect, useState } from 'react'

export const ThemeSwitcher = () => {
  const { theme, toggleTheme, isInitialized } = useTheme()
  const [mounted, setMounted] = useState(false)

  // Only render on client side after hydration
  useEffect(() => {
    setMounted(true)
  }, [])

  // Don't render anything during SSR or before theme is initialized
  if (!mounted || !isInitialized) {
    return (
      <IconButton variant="custom" size="small" aria-label="Loading theme switcher..." disabled>
        <div className="h-5 w-5" />
      </IconButton>
    )
  }

  return (
    <IconButton
      variant="custom"
      size="small"
      onClick={toggleTheme}
      aria-label={`Switch to ${theme === 'light' ? 'dark' : 'light'} mode`}
      title={`Switch to ${theme === 'light' ? 'dark' : 'light'} mode`}
    >
      {theme === 'light' ?
        <Moon className="h-5 w-5" />
      : <Sun className="h-5 w-5" />}
    </IconButton>
  )
}
