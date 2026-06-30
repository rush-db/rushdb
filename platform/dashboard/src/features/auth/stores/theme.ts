import { persistentAtom } from '@nanostores/persistent'
import { atom } from 'nanostores'

export type ThemePreference = 'dark' | 'light' | 'system'
export type ResolvedTheme = 'dark' | 'light'

/** User-chosen preference. `system` follows the OS `prefers-color-scheme`. */
export const $themePreference = persistentAtom<ThemePreference>('theme', 'system')

const prefersDark =
  typeof window !== 'undefined' && typeof window.matchMedia === 'function' ?
    window.matchMedia('(prefers-color-scheme: dark)')
  : null

const systemTheme = (): ResolvedTheme => (prefersDark?.matches ? 'dark' : 'light')

const resolve = (preference: ThemePreference): ResolvedTheme =>
  preference === 'system' ? systemTheme() : preference

/** The theme actually applied to the document (`system` collapsed to dark/light). */
export const $resolvedTheme = atom<ResolvedTheme>(resolve($themePreference.get()))

const recompute = () => $resolvedTheme.set(resolve($themePreference.get()))

$themePreference.subscribe(recompute)
prefersDark?.addEventListener('change', recompute)

$resolvedTheme.subscribe((theme) => {
  if (typeof document !== 'undefined') {
    document.documentElement.dataset.theme = theme
  }
})
