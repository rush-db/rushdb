/* eslint-disable import/no-anonymous-default-export */
import type { Config } from 'tailwindcss'

import { RING_ALPHA, RING_COLOR, colors } from './config/colors'

const config = {
  content: ['./index.html', './src/**/*.{js,ts,jsx,tsx}'],
  plugins: [require('tailwindcss-animate')],
  whitelistPatterns: [/^bg-badge-/],
  theme: {
    // colors: {
    //   accent: {
    //     blue: '#3F81FF',
    //     green: '#00A07A',
    //     orange: '#F47500',
    //     pink: '#FF44B4',
    //     red: '#FF586D',
    //     yellow: '#F4B000'
    //   },
    //   background: {
    //     DEFAULT: '#222222',
    //     overlay: '#26323880',
    //     popover: '#222222',
    //     secondary: '#333333'
    //   },
    //   'base-black': '#000',
    //   'base-white': '#FFF',
    //   accent: {
    //     DEFAULT: '#CBEE4C',
    //     contrast: '#000'
    //   },
    //   content: {
    //     DEFAULT: '#FFFFFF',
    //     popover: '#FFFFFF',
    //     secondary: '#D8D8D8',
    //     tertiary: '#FFFFFF1e'
    //   },
    //   currentColor: 'currentColor',
    //   inherit: 'inherit',
    //   primary: {
    //     hover: 'rgba(255,255,255,0.07)'
    //   },
    //   ring: '#3E545F',
    //   stroke: {
    //     DEFAULT: '#4A4A4A',
    //     hover: '#4A4A4A',
    //     tertiary: '#4A4A4A'
    //   },
    //   transparent: 'transparent'
    // },
    container: {
      center: true,
      padding: '2rem',
      screens: {
        '2xl': '1400px'
      }
    },
    extend: {
      zIndex: {
        tooltip: '100'
      },
      cursor: {
        inherit: 'inherit'
      },
      padding: {
        inherit: 'inherit'
      },
      gap: {
        inherit: 'inherit'
      },
      colors: colors,
      ringColor: { DEFAULT: RING_COLOR },
      ringOpacity: { DEFAULT: String(RING_ALPHA) },
      ringWidth: {
        DEFAULT: '4px'
      },
      borderRadius: {
        '2xl': 'calc(var(--radius) + 10px)',
        xl: 'calc(var(--radius) + 4px)',
        lg: `var(--radius)`,
        md: `calc(var(--radius) - 2px)`,
        DEFAULT: 'calc(var(--radius) - 2px)',
        sm: 'calc(var(--radius) - 4px)',
        circle: '100%'
      },
      animation: {
        'accordion-down': 'accordion-down 0.2s ease-out',
        'accordion-up': 'accordion-up 0.2s ease-out'
      },
      fontSize: {
        '2xs': '10px'
      },
      fontFamily: {
        sans: ['Manrope', 'sans-serif'],
        mono: ['JetBrains Mono', 'monospace']
      },
      keyframes: {
        'accordion-down': {
          from: { height: '0' },
          to: { height: 'var(--radix-accordion-content-height)' }
        },
        'accordion-up': {
          from: { height: 'var(--radix-accordion-content-height)' },
          to: { height: '0' }
        }
      }
    }
  }
} satisfies Config

export default config
