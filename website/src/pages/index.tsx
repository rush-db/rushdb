import { Layout } from '~/components/Layout'
import { Hero } from '~/sections/Hero'
import { SocialProof } from '~/sections/SocialProof'
import { Mission } from '~/sections/Mission'
import { HowItWorks } from '~/sections/HowItWorks'

export default function Home() {
  return (
    <Layout>
      <Hero />
      <SocialProof />
      <HowItWorks />
      <Mission />
    </Layout>
  )
}
