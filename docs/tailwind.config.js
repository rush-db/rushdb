/** @type {import('tailwindcss').Config} */
module.exports = {
  corePlugins: {
    preflight: false,
    container: false
  },
  darkMode: ['class', '[data-theme="dark"]'],
  content: ['./src/**/*.{jsx,tsx,html}', './docs/**/*.{md,mdx}'],
  theme: {
    extend: {
      borderRadius: {
        sm: '4px'
      },
      screens: {
        sm: '0px',
        lg: '997px'
      },
      colors: {
        accent: {
          DEFAULT: '#3f81ff', // Base color
          contrast: 'hsl(0, 0%, 100%)', // White for contrast on buttons, etc.
          hover: '#3c78ef', // Slightly darker for hover state
          focus: '#346ccd', // Same as hover for consistency
          ring: 'rgba(63, 129, 255, 0.24)' // Transparent version of base for focus ring
        }
      }
    }
  },
  plugins: []
}
