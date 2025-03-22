import React, { useState, type ReactNode } from 'react'

import { Logo } from '~/elements/Logo'
import { cn } from '~/lib/utils'

import { motion } from 'framer-motion'

import createVideo from '~/assets/videos/create.mp4'
import deleteVideo from '~/assets/videos/delete.mp4'
import searchVideo from '~/assets/videos/search.mp4'
import { Helmet } from 'react-helmet-async'

const videos = [searchVideo, createVideo, deleteVideo]

export const signupTitle = 'Sign Up – RushDB'
export const signupDescription =
  'Create your RushDB account. Set up a graph database in seconds—no config, no hassle. Perfect for AI, SaaS, and ML projects.'

export const signinTitle = 'Sign In – RushDB'
export const signinDescription =
  'Access your RushDB dashboard. Sign in to manage your graph-powered projects, run queries, and build AI-ready apps instantly.'

export const restoreTitle = 'Reset Password – RushDB'
export const restoreDescription =
  'Forgot your password? Reset it securely and get back to building with RushDB — your zero-config graph database for AI and modern apps.'

export const notFoundTitle = 'Page Not Found – RushDB'
export const notFoundDescription =
  'This page doesn’t exist or has been moved. Head back to RushDB to explore the zero-config graph database for AI and modern apps.'

export const defaultTitle = 'RushDB – Instant Graph Database for AI & Modern Apps'
export const defaultDescription =
  'RushDB is a zero-config, graph-powered database built for AI, SaaS, and ML. Fast queries, seamless scaling, no setup. Try it now!'

const metadataMap = {
  signin: { title: signinTitle, description: signinDescription },
  signup: { title: signupTitle, description: signupDescription },
  recover: { title: restoreTitle, description: restoreDescription },
  '404': { title: notFoundTitle, description: notFoundDescription }
}

export function AuthLayout({
  children,
  className,
  title,
  type,
  ...props
}: TPolymorphicComponentProps<'div', { title?: ReactNode; type: 'signin' | 'signup' | 'recover' | '404' }>) {
  const [videoIdx, setVideoIdx] = useState(0)

  const videoSrc = videos[videoIdx]

  const meta = metadataMap[type] ?? { title: defaultTitle, description: defaultDescription }

  return (
    <div
      className={cn(
        className,
        'mx-auto flex min-h-screen flex-col items-center justify-start gap-3 p-5 sm:justify-center sm:gap-5'
      )}
      {...props}
    >
      <Helmet>
        <title>{meta.title}</title>
        <meta name="description" content={meta.description} />
      </Helmet>

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
        onEnded={() => setVideoIdx((currentIdx) => (currentIdx + 1) % videos.length)}
      />

      <main className="sm:bg-fill/60 relative z-10 flex w-full max-w-xl flex-col items-stretch gap-5 sm:rounded-md sm:border sm:p-5 sm:backdrop-blur-sm">
        <Logo className="mx-auto" />

        {title ?
          <h1 className="mb-5 text-center text-2xl font-bold leading-tight tracking-tight">{title}</h1>
        : null}

        {children}
      </main>
    </div>
  )
}
