import React from 'react'
import ThemeSwitch from '@site/src/components/ThemeSwitch'
import type { Props } from '@theme/ColorModeToggle'

export default function ColorModeToggle({ value, onChange, className }: Props) {
  const isDark = value === 'dark'

  return (
    <ThemeSwitch className={className} isDark={isDark} onToggle={() => onChange(isDark ? 'light' : 'dark')} />
  )
}
