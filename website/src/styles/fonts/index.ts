import localFont from 'next/font/local'
import { Manrope } from 'next/font/google'

export const jetBrainsMono = localFont({
  src: [
    {
      path: './jet-brains-mono/JetBrainsMono-Regular.woff2',
      weight: '400',
      style: 'regular'
    },
    {
      path: './jet-brains-mono/JetBrainsMono-Bold.woff2',
      weight: '600',
      style: 'bold'
    }
  ],
  variable: '--font-jet-brains-mono'
})

export const manrope = Manrope({
  subsets: ['latin'],
  weight: ['200', '400', '500', '600', '700', '800'],
  variable: '--font-manrope',
  display: 'block',
  adjustFontFallback: false
})
