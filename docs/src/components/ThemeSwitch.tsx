import React from 'react'
import { Moon, Sun } from 'lucide-react'

type Props = {
  className?: string
  disabled?: boolean
  isDark: boolean
  onToggle: () => void
}

export default function ThemeSwitch({ className, disabled, isDark, onToggle }: Props) {
  return (
    <button
      type="button"
      role="switch"
      aria-checked={isDark}
      aria-label={isDark ? 'Switch to light mode' : 'Switch to dark mode'}
      title={isDark ? 'Switch to light mode' : 'Switch to dark mode'}
      disabled={disabled}
      className={['rushdb-theme-switch', className].filter(Boolean).join(' ')}
      data-dark={isDark ? 'true' : 'false'}
      onClick={onToggle}
    >
      <span className="rushdb-theme-switch__icons" aria-hidden="true">
        <Sun size={12} strokeWidth={2.5} className="rushdb-theme-switch__sun" />
        <Moon size={12} strokeWidth={2.5} className="rushdb-theme-switch__moon" />
      </span>
      <span className="rushdb-theme-switch__thumb" aria-hidden="true">
        {isDark ?
          <Moon size={12} strokeWidth={2.5} className="rushdb-theme-switch__thumb-icon" />
        : <Sun size={12} strokeWidth={2.5} className="rushdb-theme-switch__thumb-icon" />}
      </span>
    </button>
  )
}
