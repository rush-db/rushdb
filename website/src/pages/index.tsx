import { Layout } from '~/components/Layout'
import { Hero } from '~/sections/Hero'
import { SocialProof } from '~/sections/SocialProof'
import { Mission } from '~/sections/Mission'
import { HowItWorks } from '~/sections/HowItWorks'
import { Faq } from '~/components/Faq'
import { BlogSection } from '~/sections/BlogSection'

export default function Home() {
  return (
    <Layout>
      <Hero />
      <SocialProof />
      <HowItWorks />
      <BlogSection />

      <section className="container mx-auto max-w-3xl">
        <Faq />
      </section>
      <Mission />
    </Layout>
  )
}
