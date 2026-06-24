import { type ReactNode } from 'react'

import { Code2, Network, Zap } from 'lucide-react'

import { Logo } from '~/elements/Logo'
import { cn } from '~/lib/utils'
import CookieNotification from '~/components/CookiesConsent'
import { PolicyLinks } from '~/components/PolicyLinks'
import { GraphCanvas } from '~/features/auth/components/GraphCanvas'

import { Helmet } from 'react-helmet-async'

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

const features = [
  { icon: Network, title: 'Connected memory', description: 'facts, embeddings & relationships in one' },
  { icon: Zap, title: 'Graph-aware retrieval', description: 'semantic search + traversal in one query' },
  { icon: Code2, title: 'Live schema', description: 'agents query real data, not guesses' }
]

function BrandPanel() {
  return (
    <aside className="bg-fill3 relative hidden overflow-hidden lg:flex lg:w-1/2 xl:w-[55%]">
      <GraphCanvas className="absolute inset-0" />

      <div className="relative z-10 flex flex-col justify-center gap-10 p-12 xl:p-16">
        <div className="text-4xl font-bold tracking-tight xl:text-5xl">
          Rush<span className="text-accent">DB</span>
        </div>

        <div className="flex flex-col gap-4">
          <h2 className="max-w-lg text-4xl font-bold leading-[1.1] tracking-tight xl:text-5xl">
            Instant graph memory
            <br />
            <span className="text-accent">for AI agents</span>
          </h2>
          <p className="text-content2 max-w-md text-base">
            Push JSON. Get semantic recall, graph traversal, and live schema over one memory.
          </p>
        </div>

        <ul className="flex flex-col gap-5">
          {features.map(({ icon: Icon, title, description }) => (
            <li className="flex items-center gap-4" key={title}>
              <span className="bg-secondary border-stroke flex h-11 w-11 shrink-0 items-center justify-center rounded-full border">
                <Icon className="text-accent h-5 w-5" />
              </span>
              <span className="flex flex-col">
                <span className="font-semibold">{title}</span>
                <span className="text-content2 text-sm">{description}</span>
              </span>
            </li>
          ))}
        </ul>
      </div>
    </aside>
  )
}

export function AuthLayout({
  children,
  className,
  title,
  subtitle,
  type,
  ...props
}: TPolymorphicComponentProps<
  'div',
  {
    title?: ReactNode
    subtitle?: ReactNode
    type: 'signin' | 'signup' | 'recover' | '404'
  }
>) {
  const meta = metadataMap[type] ?? { title: defaultTitle, description: defaultDescription }

  return (
    <div className={cn('flex min-h-screen w-full', className)} {...props}>
      <Helmet>
        <title>{meta.title}</title>
        <meta name="description" content={meta.description} />
      </Helmet>

      <BrandPanel />

      <div className="relative flex w-full flex-col items-center justify-center p-5 lg:w-1/2 xl:w-[45%]">
        <main className="bg-fill3/60 z-10 flex w-full max-w-md flex-col items-stretch gap-5 rounded-2xl border p-6 backdrop-blur-sm sm:p-8">
          <Logo className="mx-auto" height={40} width={40} />

          {title ?
            <div className="flex flex-col gap-1 text-center">
              <h1 className="text-2xl font-bold leading-tight tracking-tight">{title}</h1>
              {subtitle ?
                <p className="text-content2 text-sm">{subtitle}</p>
              : null}
            </div>
          : null}

          {children}

          <PolicyLinks />
        </main>

        <CookieNotification />
      </div>
    </div>
  )
}
