import { useState, type ReactNode } from 'react'

import { Logo } from '~/elements/Logo'
import { cn } from '~/lib/utils'

import { motion } from 'framer-motion'

import createVideo from '~/assets/videos/create.mp4'
import deleteVideo from '~/assets/videos/delete.mp4'
import searchVideo from '~/assets/videos/search.mp4'

const videos = [searchVideo, createVideo, deleteVideo]

export function AuthLayout({
  children,
  className,
  title,
  ...props
}: TPolymorphicComponentProps<'div', { title?: ReactNode }>) {
  const [videoIdx, setVideoIdx] = useState(0)

  const videoSrc = videos[videoIdx]

  return (
    <div
      className={cn(
        className,
        'mx-auto flex min-h-screen flex-col items-center justify-start gap-3 p-5 sm:justify-center sm:gap-5'
      )}
      {...props}
    >
      <motion.video
        src={videoSrc}
        autoPlay
        muted
        playsInline
        aria-hidden
        className="fixed inset-0 z-0 hidden h-full w-full object-cover opacity-20 sm:block"
        initial={{ opacity: 0 }}
        animate={{ opacity: 0.2 }}
        transition={{ duration: 0.5 }}
        onEnded={() =>
          setVideoIdx((currentIdx) => (currentIdx + 1) % videos.length)
        }
      />

      <main className="relative z-10 flex w-full max-w-xl flex-col items-stretch gap-5 sm:rounded-md sm:border sm:bg-fill/60 sm:p-5 sm:backdrop-blur-sm">
        <Logo className="mx-auto" />

        {title ? (
          <h1 className="mb-5 text-center text-2xl font-bold leading-tight tracking-tight">
            {title}
          </h1>
        ) : null}

        {children}
      </main>
    </div>
  )
}
