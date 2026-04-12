import Image from 'next/image'

const MAP: Record<string, { src: string; alt: string }> = {
  typescript: { src: '/images/typescript-logo.svg', alt: 'TypeScript' },
  python: { src: '/images/python-logo.svg', alt: 'Python' },
  shell: { src: '/images/bash-logo.svg', alt: 'Shell' }
}

interface LangIconProps {
  lang: string
  size?: number
  className?: string
}

export function LangIcon({ lang, size = 16, className = '' }: LangIconProps) {
  const entry = MAP[lang]
  if (!entry) return null
  return (
    <Image
      src={entry.src}
      alt={entry.alt}
      width={size}
      height={size}
      className={`grayscale ${className}`}
      priority
    />
  )
}
