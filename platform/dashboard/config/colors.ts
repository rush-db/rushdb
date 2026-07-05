import type { Oklch } from 'culori'

import { converter, formatHex, oklch, wcagContrast } from 'culori'

import { isAnyObject } from '../src/types'

type Color = Oklch

type ColorState = {
  DEFAULT: Color
  contrast?: Color
  focus?: Color
  hover?: Color
  ring?: Color
}

type ColorOrColorState = Color | ColorState

type ColorTree = { [key: string]: Color | ColorState | ColorTree }

const isColorState = (predicate: ColorOrColorState): predicate is ColorState =>
  isAnyObject(predicate) && 'DEFAULT' in predicate

const toHsl = converter('hsl')

const parseColor = (color: string) => oklch(color) as Color

/**
 * Format a color as space-separated HSL channels ("H S% L%") without the
 * `hsl()` wrapper — theme tokens wrap the variable as `hsl(var(--x))` and
 * Tailwind v4 layers opacity modifiers on top with `color-mix()`.
 */
const clamp01 = (n: number): number => Math.min(1, Math.max(0, Number.isFinite(n) ? n : 0))

const formatHslChannels = (color: Color): string => {
  const a = color.alpha ?? 1

  // Round-trip through sRGB hex first: OKLCH colors can sit slightly outside the
  // sRGB gamut, which makes the direct HSL conversion return out-of-range
  // saturation/luminance (e.g. negative or >100%) and distorts the color.
  const { h = 0, l = 0, s = 0 } = toHsl(formatHex(color)) ?? {}

  const hue = Number((h ?? 0).toFixed(2))
  const sat = `${Number((clamp01(s ?? 0) * 100).toFixed(2))}%`
  const lum = `${Number((clamp01(l ?? 0) * 100).toFixed(2))}%`

  // Alpha-baked tokens carry their (per-theme) alpha in the channel value.
  return a < 1 ? `${hue} ${sat} ${lum} / ${Number(a.toFixed(3))}` : `${hue} ${sat} ${lum}`
}

const shiftColorLuminance = (_color: ColorOrColorState, shiftAmount: Color['l']): Color => {
  const color = isColorState(_color) ? _color.DEFAULT : _color

  return { ...color, l: color.l + shiftAmount }
}

const shiftColorAlpha = (_color: ColorOrColorState, alpha: Color['l']): Color => {
  const color = isColorState(_color) ? _color.DEFAULT : _color

  return { ...color, alpha }
}

export const RING_ALPHA = 0.24

const smartShiftLuminance = (color: Color) => {
  const step = color.l > 0.6 ? -0.1 : 0.1

  return shiftColorLuminance(color, step)
}

const smartShiftAlpha = (color: Color) => {
  const step = color.alpha && color.alpha > 0.8 ? -0.1 : 0.1
  return shiftColorAlpha(color, (color?.alpha ?? 0) + step)
}

const getContrastColor = (color: Color): Color => {
  // contrast
  const isLightColor = wcagContrast(color, 'black') > 4.5

  // should be done based on main text
  return isLightColor ? parseColor('black') : parseColor('white')
}

const generateColorState = (_color: Color | string): ColorState => {
  const color = typeof _color === 'string' ? parseColor(_color) : _color

  const isTransparent = color.alpha && color.alpha !== 1

  const shiftFn = isTransparent ? smartShiftAlpha : smartShiftLuminance

  return {
    DEFAULT: color,
    contrast: getContrastColor(color),
    hover: shiftFn(color),
    focus: shiftFn(color),
    ring: shiftColorAlpha(color, RING_ALPHA)
  }
}

type SuggestedKeys =
  | 'accent'
  | 'content'
  | 'danger'
  | 'disabled'
  | 'fill'
  | 'menu'
  | 'primary'
  | 'secondary'
  | 'stroke'

type PaletteEntries = [key: SuggestedKeys | (string & {}), baseColor: string][]

/**
 * The neutral surface/content tokens are the only colors that differ between
 * dark and light. Brand and semantic colors (accent, danger, success, warning,
 * badges) are shared so the brand stays consistent and contrast text is still
 * derived by `getContrastColor`.
 */
const brandEntries: PaletteEntries = [
  ['danger', 'oklch(54% 0.201 22.76)'],
  ['success', 'oklch(69.17% 0.217 142.4)'],
  ['warning', 'oklch(86.3% 0.173 90.5)']
]

const darkNeutralEntries: PaletteEntries = [
  // The brand accent is intentionally neutralized: `accent` mirrors `primary`
  // so no brand color renders anywhere. CTAs use `primary` directly.
  ['accent', 'oklch(100% 0 0)'],
  ['primary', 'oklch(100% 0 0)'],
  ['secondary', 'oklch(100% 0 0 / 10%)'],
  ['disabled', 'oklch(30% 0 0)'],
  // fill
  ['fill', 'oklch(25.2% 0 0)'],
  ['fill2', 'oklch(23.2% 0 0)'],
  ['fill3', 'oklch(20.2% 0 0)'],
  ['menu', 'oklch(20.2% 0 0)'],
  // content
  ['content', 'oklch(100% 0 0)'],
  ['content2', 'oklch(80% 0 0)'],
  ['content3', 'oklch(65% 0 0)'],
  // stroke
  ['stroke', 'oklch(100% 0 0 / 7%)']
]

const lightNeutralEntries: PaletteEntries = [
  // Neutralized brand accent — mirrors `primary` (see dark palette note).
  ['accent', 'oklch(22% 0 0)'],
  ['primary', 'oklch(22% 0 0)'],
  ['secondary', 'oklch(0% 0 0 / 8%)'],
  ['disabled', 'oklch(90% 0 0)'],
  // fill (lightest = main background)
  ['fill', 'oklch(99% 0 0)'],
  ['fill2', 'oklch(97% 0 0)'],
  ['fill3', 'oklch(94% 0 0)'],
  ['menu', 'oklch(97% 0 0)'],
  // content
  ['content', 'oklch(22% 0 0)'],
  ['content2', 'oklch(40% 0 0)'],
  ['content3', 'oklch(55% 0 0)'],
  // stroke
  ['stroke', 'oklch(0% 0 0 / 10%)']
]

const buildStates = (entries: PaletteEntries): Record<string, ColorState> =>
  Object.fromEntries(entries.map(([key, baseColor]) => [key, generateColorState(baseColor)]))

// `focusChannels` is the inner OKLCH channel string (e.g. "100% 0 0"); the focus
// and ring states reuse it at fixed alphas. Kept neutral so no brand color shows.
const interactionState = (defaultColor: string, focusChannels: string): Record<string, Color> => ({
  DEFAULT: parseColor(defaultColor),
  hover: parseColor(defaultColor),
  focus: parseColor(`oklch(${focusChannels} / 88%)`),
  ring: parseColor(`oklch(${focusChannels} / ${RING_ALPHA * 100}%)`)
})

const badgeTree: ColorTree = {
  blue: { DEFAULT: parseColor('oklch(62.86% 0.199 261.73)'), contrast: parseColor('oklch(100% 0 0)') },
  red: { DEFAULT: parseColor('oklch(54% 0.201 22.76)'), contrast: parseColor('oklch(100% 0 0)') },
  yellow: { DEFAULT: parseColor('oklch(86.3% 0.173 90.5)'), contrast: parseColor('oklch(0% 0 0)') },
  green: { DEFAULT: parseColor('oklch(69.17% 0.217 142.4)'), contrast: parseColor('oklch(100% 0 0)') },
  pink: { DEFAULT: parseColor('oklch(69.17% 0.217 12.6)'), contrast: parseColor('oklch(100% 0 0)') },
  orange: { DEFAULT: parseColor('oklch(69.17% 0.217 31)'), contrast: parseColor('oklch(100% 0 0)') }
}

/** Build the raw (unformatted) color tree for one theme. */
const buildPalette = (neutralEntries: PaletteEntries): ColorTree => ({
  ...buildStates(brandEntries),
  ...buildStates(neutralEntries),
  interaction: interactionState('oklch(100% 0 0 / 24%)', '100% 0 0'),
  badge: badgeTree
})

const darkPalette = buildPalette(darkNeutralEntries)
const lightPalette = buildPalette(lightNeutralEntries)
// The interaction default differs between themes (white overlay vs black overlay).
lightPalette.interaction = interactionState('oklch(0% 0 0 / 16%)', '22% 0 0')

const isColor = (value: Color | ColorState | ColorTree): value is Color =>
  isAnyObject(value) && 'mode' in value

const varName = (path: string[]): string => `--${path.join('-').replace(/-DEFAULT$/, '')}`

/** Walk a palette tree, collecting CSS-variable channel values keyed by var name. */
function collectVars(tree: ColorTree, path: string[] = [], acc: Record<string, string> = {}) {
  for (const [key, value] of Object.entries(tree)) {
    const nextPath = [...path, key]
    if (isColor(value)) {
      acc[varName(nextPath)] = formatHslChannels(value)
    } else {
      collectVars(value as ColorTree, nextPath, acc)
    }
  }
  return acc
}

/**
 * Walk the (dark) palette tree to build the Tailwind v4 `@theme inline` token
 * map. Each leaf becomes a `--color-*` token wrapping its runtime CSS variable
 * (`hsl(var(--x))`), so utilities follow the active theme. `/opacity` modifiers
 * work on both opaque and alpha-baked tokens — v4 applies them via `color-mix()`.
 */
function collectThemeTokens(tree: ColorTree, path: string[] = [], acc: Record<string, string> = {}) {
  for (const [key, value] of Object.entries(tree)) {
    const nextPath = [...path, key]
    if (isColor(value)) {
      const channelVar = varName(nextPath)
      acc[`--color${channelVar.slice(1)}`] = `hsl(var(${channelVar}))`
    } else {
      collectThemeTokens(value as ColorTree, nextPath, acc)
    }
  }
  return acc
}

export const darkVars = collectVars(darkPalette)
export const lightVars = collectVars(lightPalette)

export const themeTokens = collectThemeTokens(darkPalette)
