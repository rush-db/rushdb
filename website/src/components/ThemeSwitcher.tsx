import { Moon, Sun } from 'lucide-react'
import { useTheme } from '~/contexts/ThemeContext'
import { IconButton } from '~/components/IconButton'

export const ThemeSwitcher = () => {
  const { theme, toggleTheme } = useTheme()

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
