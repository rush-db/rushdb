/** @type {import('tailwindcss').Config} */
module.exports = {
  content: ['./src/**/*.{js,ts,jsx,tsx}'],
  theme: {
    screens: {
      xl: { max: '1920px' },
      lg: { max: '1680px' },
      md: { max: '1079px' },
      sm: { max: '759px' }
    },
    container: {
      center: true,
      screens: {
        sm: '600px',
        md: '728px',
        lg: '984px',
        xl: '1240px',
        '2xl': '1496px'
      },
      padding: {
        DEFAULT: '1rem',
        sm: '1rem',
        lg: '1rem',
        xl: '1rem',
        '2xl': '1rem'
      }
    },
    fontSize: {
      xs: '12px',
      sm: '16px',
      base: '18px',
      md: '20px',
      lg: '24px',
      xl: '30px',
      '2xl': '40px',
      '3xl': '48px',
      '4xl': '64px'
    },
    extend: {
      fontFamily: {
        sans: ['var(--font-manrope)'],
        special: ['var(--font-dm-serif-display)'],
        mono: ['var(--font-jet-brains-mono)']
      },
      keyframes: {
        zoomOut: {
          '0%': { transform: 'scale(1.3)' },
          '100%': { transform: 'scale(1)' }
        },
        fadeIn: {
          '0%': { opacity: '0' },
          '100%': { opacity: '1' }
        }
      },
      animation: {
        zoomOut: 'zoomOut 1s ease-out',
        fadeIn: 'fadeIn 0.3s ease-in'
      },
      colors: {
        'accent-brand': '#3f81ff', // Switched from white to black for light mode.
        'accent-yellow': '#E2C044',
        'accent-orange': '#F58B3B',
        'accent-green': '#409E6F',
        'accent-red': '#D14040',
        'accent-pink': '#E27C9A',
        'accent-blue': '#3A99C8',
        'accent-purple': '#A370C1',

        disabled: {
          DEFAULT: 'hsl(0, 0%, 92%)',
          contrast: 'hsl(0, 0%, 55%)',
          hover: 'hsl(0, 0%, 87%)',
          focus: 'hsl(0, 0%, 87%)',
          ring: 'hsla(0, 0%, 92%, 0.5)'
        },
        fill: {
          DEFAULT: 'hsl(0,0%,100%)',
          contrast: 'hsl(0, 0%, 20%)',
          hover: 'hsl(0, 0%, 92%)',
          focus: 'hsl(0, 0%, 92%)',
          ring: 'hsla(0, 0%, 97%, 0.5)'
        },
        fill2: {
          DEFAULT: 'hsl(0, 0%, 95%)',
          contrast: 'hsl(0, 0%, 25%)',
          hover: 'hsl(0, 0%, 90%)',
          focus: 'hsl(0, 0%, 90%)',
          ring: 'hsla(0, 0%, 95%, 0.5)'
        },
        fill3: {
          DEFAULT: 'hsl(0, 0%, 90%)',
          contrast: 'hsl(0, 0%, 30%)',
          hover: 'hsl(0, 0%, 85%)',
          focus: 'hsl(0, 0%, 85%)',
          ring: 'hsla(0, 0%, 90%, 0.5)'
        },
        content: {
          DEFAULT: 'hsl(0, 0%, 15%)',
          contrast: 'hsl(0, 0%, 100%)',
          hover: 'hsl(0, 0%, 25%)',
          focus: 'hsl(0, 0%, 25%)',
          ring: 'hsla(0, 0%, 15%, 0.5)'
        },
        content2: {
          DEFAULT: 'hsl(0, 0%, 30%)',
          contrast: 'hsl(0, 0%, 100%)',
          hover: 'hsl(0, 0%, 40%)',
          focus: 'hsl(0, 0%, 40%)',
          ring: 'hsla(0, 0%, 30%, 0.5)'
        },
        content3: {
          DEFAULT: 'hsl(0, 0%, 50%)',
          contrast: 'hsl(0, 0%, 100%)',
          hover: 'hsl(0, 0%, 60%)',
          focus: 'hsl(0, 0%, 60%)',
          ring: 'hsla(0, 0%, 50%, 0.5)'
        },
        stroke: {
          DEFAULT: '#EBEBEB',
          contrast: 'hsl(0, 0%, 80%)',
          hover: 'hsla(0, 0%, 20%, 0.2)',
          focus: 'hsla(0, 0%, 20%, 0.2)',
          ring: 'hsla(0, 0%, 20%, 0.5)'
        },
        accent: {
          DEFAULT: '#3f81ff', // Base color
          contrast: 'hsl(0, 0%, 100%)', // White for contrast on buttons, etc.
          hover: '#3c78ef', // Slightly darker for hover state
          focus: '#346ccd', // Same as hover for consistency
          ring: 'rgba(63, 129, 255, 0.24)' // Transparent version of base for focus ring
        },
        primary: {
          DEFAULT: 'hsl(0, 0%, 100%)',
          contrast: 'hsl(0, 0%, 10%)',
          hover: 'hsl(0, 0%, 95%)',
          focus: 'hsl(0, 0%, 95%)',
          ring: 'hsla(0, 0%, 100%, 0.5)'
        },
        secondary: {
          DEFAULT: 'hsla(0, 0%, 100%, 0.9)',
          contrast: 'hsl(0, 0%, 20%)',
          hover: 'hsla(0,0%,96%,0.8)',
          focus: 'hsla(0, 0%, 100%, 0.8)',
          ring: 'hsla(0, 0%, 100%, 0.5)'
        }
      }
    }
  }
}
