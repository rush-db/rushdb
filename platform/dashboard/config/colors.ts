import type { Oklch } from 'culori'

import { formatHsl, formatCss, oklch, wcagContrast } from 'culori'

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

const isColorState = (predicate: ColorOrColorState): predicate is ColorState =>
  isAnyObject(predicate) && 'default' in predicate

const formatColor = (color: Color | string | undefined) =>
  // formatCss(color) as string
  formatHsl(color) as string

const parseColor = (color: string) => oklch(color) as Color

const shiftColorLuminance = (
  _color: ColorOrColorState,
  shiftAmount: Color['l']
): Color => {
  const color = isColorState(_color) ? _color.DEFAULT : _color

  return { ...color, l: color.l + shiftAmount }
}

const shiftColorAlpha = (
  _color: ColorOrColorState,
  alpha: Color['l']
): Color => {
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
    // ring: {
    //   h: color.h,
    //   l: 0.69,
    //   c: 0.17,
    //   alpha: RING_ALPHA,
    //   mode: color.mode
    // }
  }
}

function deepFormat<O extends Record<string, Color | string>>(obj: O) {
  return Object.fromEntries(
    Object.entries(obj).map(([key, val]) => [key, formatColor(val)])
  ) as Record<keyof O, ReturnType<typeof formatColor>>
}

/**
 * // any color has contrast color
 * accent, danger, warning, primary, secondary, disabled {
 *  DEFAULT // green oklch(89.49% 0.186 120.89)
 *  contrast // white
 *  hover
 *  focus
 * }
 *
 * fill, fill2, menu { //bg-surface
 * }
 *
 * content, content2, note {
 * }
 *
 * stroke {}
 *
 */
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

function createColors<
  // eslint-disable-next-line @typescript-eslint/ban-types
  E extends [key: SuggestedKeys | (string & {}), baseColor: string][]
>({
  entries
}: {
  entries: E
}): Record<E[number][0], ReturnType<typeof deepFormat>> {
  const colors = entries.map(([key, baseColor]) => {
    return [key, deepFormat(generateColorState(baseColor))]
  })

  return Object.fromEntries(colors)
}

const setColorHue = (_color: Color | string, hue: Color['h']): Color => {
  const color = isAnyObject(_color) ? _color : parseColor(_color)

  return { ...color, h: hue }
}

export const colors = {
  ...createColors({
    entries: [
      ['accent' as const, 'oklch(89.49% 0.186 120.89)'],
      ['danger' as const, 'oklch(54% 0.201 22.76)'],
      ['success' as const, 'oklch(69.17% 0.217 142.4)'],
      ['warning' as const, 'oklch(86.3% 0.173 90.5)'],
      ['primary' as const, 'oklch(100% 0 0)'],
      ['secondary' as const, 'oklch(100% 0 0 / 10%)'],
      ['disabled' as const, 'oklch(30% 0 0)'],
      // fill
      ['fill' as const, `oklch(25.2% 0 0)`],
      ['fill2' as const, `oklch(23.2% 0 0)`],
      ['fill3' as const, `oklch(20.2% 0 0)`],
      ['menu' as const, `oklch(20.2% 0 0)`],
      // content
      ['content' as const, `oklch(100% 0 0)`],
      ['content2' as const, `oklch(80% 0 0)`],
      ['content3' as const, `oklch(65% 0 0)`],
      // stroke
      ['stroke' as const, 'oklch(100% 0 0 / 7%)']
    ]
  }),
  // interaction: deepFormat({
  //   DEFAULT: 'oklch(100% 0 0)',
  //   hover: 'oklch(100% 0 0 / 24%)',
  //   focus: 'oklch(100% 0 0 / 51%)'
  // })
  interaction: deepFormat({
    DEFAULT: 'oklch(100% 0 0 / 24%)',
    hover: 'oklch(100% 0 0 / 24%)',
    focus: 'oklch(89.49% 0.186 120.89 / 88%)',
    ring: `oklch(89.49% 0.186 120.89 / ${RING_ALPHA * 100}%)`
  }),
  badge: {
    //     blue: '#3F81FF',
    //     green: '#00A07A',
    //     orange: '#F47500',
    //     pink: '#FF44B4',
    //     red: '#FF586D',
    //     yellow: '#F4B000'
    blue: deepFormat({
      DEFAULT: 'oklch(62.86% 0.199 261.73)',
      contrast: 'oklch(100% 0 0)'
    }),
    red: deepFormat({
      DEFAULT: 'oklch(54% 0.201 22.76)',
      contrast: 'oklch(100% 0 0)'
    }),
    yellow: deepFormat({
      DEFAULT: 'oklch(86.3% 0.173 90.5)',
      contrast: 'oklch(0% 0 0)'
    }),
    green: deepFormat({
      DEFAULT: 'oklch(69.17% 0.217 142.4)',
      contrast: 'oklch(100% 0 0)'
    }),
    pink: deepFormat({
      DEFAULT: 'oklch(69.17% 0.217 12.6)',
      contrast: 'oklch(100% 0 0)'
    }),
    orange: deepFormat({
      DEFAULT: 'oklch(69.17% 0.217 31)',
      contrast: 'oklch(100% 0 0)'
    })
  }
}

export const RING_COLOR = formatColor(colors['content']['ring']) //formatColor(colors.surface.ring.DEFAULT)

export const cssVars = Object.entries(colors).reduce((acc, [group, val]) => {
  if ('DEFAULT' in val) {
    Object.entries(val).forEach(([state, color]) => {
      let colorName = `--${group}-${state}`.replace('DEFAULT', '')
      // @ts-ignore
      acc[colorName] = color
    })
  }

  return acc
}, {})
