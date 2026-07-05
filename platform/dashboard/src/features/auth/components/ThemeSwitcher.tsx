import { useStore } from '@nanostores/react'
import { Monitor, Moon, Sun } from 'lucide-react'

import type { ThemePreference } from '~/features/auth/stores/theme'
import { $themePreference } from '~/features/auth/stores/theme'
import { cn } from '~/lib/utils'

const THEME_OPTIONS: Array<{ icon: React.ReactNode; label: string; value: ThemePreference }> = [
  { value: 'system', label: 'System', icon: <Monitor className="h-4 w-4" /> },
  { value: 'light', label: 'Light', icon: <Sun className="h-4 w-4" /> },
  { value: 'dark', label: 'Dark', icon: <Moon className="h-4 w-4" /> }
]

export function ThemeSwitcher({ className, compact }: { className?: string; compact?: boolean }) {
  const preference = useStore($themePreference)

  return (
    <div
      className={cn('inline-flex gap-1 rounded-md border border-stroke p-1', className)}
      role="radiogroup"
      aria-label="Theme"
    >
      {THEME_OPTIONS.map((option) => {
        const active = preference === option.value
        return (
          <button
            aria-checked={active}
            aria-label={option.label}
            title={compact ? option.label : undefined}
            className={cn(
              'flex items-center gap-2 rounded-md text-sm font-medium transition-colors',
              compact ? 'p-1.5' : 'px-3 py-1.5',
              active ? 'bg-secondary text-content' : (
                'text-content2 hover:bg-secondary-hover hover:text-content'
              )
            )}
            key={option.value}
            onClick={() => $themePreference.set(option.value)}
            role="radio"
            type="button"
          >
            {option.icon}
            {compact ? null : option.label}
          </button>
        )
      })}
    </div>
  )
}
