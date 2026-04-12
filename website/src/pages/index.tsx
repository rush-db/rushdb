/*
 * ──────────────────────────────────────────────────
 * OLD HOME PAGE (commented out — preserved for reference)
 * ──────────────────────────────────────────────────
import { Layout } from '~/components/Layout'
import { Hero } from '~/sections/Hero'
import { SocialProof } from '~/sections/SocialProof'
import { TheProblem } from '~/sections/TheProblem'
import { Mission } from '~/sections/Mission'
import { HowItWorks } from '~/sections/HowItWorks'
import { ChooseInterface } from '~/sections/ChooseInterface'
import { Faq } from '~/components/Faq'
import { BlogSection } from '~/sections/BlogSection'

export default function Home() {
  return (
    <Layout>
      <Hero />
      <SocialProof />
      <TheProblem />
      <HowItWorks />
      <ChooseInterface />
      <BlogSection />

      <section className="container mx-auto max-w-3xl">
        <Faq />
      </section>
      <Mission />
    </Layout>
  )
}
 * ──────────────────────────────────────────────────
 */

import { useEffect } from 'react'
import Head from 'next/head'
import { Meta } from '~/components/Meta'
import { LPHeader } from '~/components/lp/LPHeader'

import { LPHero } from '~/sections/lp/Hero'
import { LPProblemStrip } from '~/sections/lp/ProblemStrip'
import { LPHowItWorks } from '~/sections/lp/HowItWorks'
import { LPCodeSection } from '~/sections/lp/CodeSection'
import { LPUseCases } from '~/sections/lp/UseCases'
import { LPOntologySection } from '~/sections/lp/OntologySection'
import { LPFeatureProof } from '~/sections/lp/FeatureProof'
import { LPSocialProof } from '~/sections/lp/SocialProof'
import { LPPricing } from '~/sections/lp/Pricing'
import { LPFooterCTA } from '~/sections/lp/FooterCTA'
import { Footer } from '~/components/Layout/Footer'

export default function Home() {
  // Force dark mode for the LP
  useEffect(() => {
    const html = document.documentElement
    const hadDark = html.classList.contains('dark')
    html.classList.add('dark')
    return () => {
      if (!hadDark) html.classList.remove('dark')
    }
  }, [])

  return (
    <>
      <Meta
        title="RushDB — Memory for Agents and Apps"
        description="Push any JSON. Get graph relationships and vector search instantly. The memory layer for AI agents and modern apps. No schema, no pipeline."
      />
      <Head>
        <meta property="og:title" content="RushDB 2.0 — Memory for Agents and Apps" />
        <meta
          property="og:description"
          content="Vector + graph from a single write. Instant memory for AI agents."
        />
      </Head>

      <div>
        <LPHeader />
        <div className="flex min-h-screen flex-col pt-16">
          <LPHero />
          <LPProblemStrip />
        </div>
        <LPHowItWorks />
        <LPCodeSection />
        <LPUseCases />
        <LPOntologySection />
        <LPFeatureProof />
        <LPSocialProof />
        <LPPricing showEnterprise={false} />
        <LPFooterCTA />
        <Footer />
      </div>
    </>
  )
}
