import { Layout } from '~/components/Layout'
import { Hero } from '~/sections/Hero'

import { Mission } from '~/sections/Mission'
import { UseCases } from '~/sections/UseCases'
import { HowItWorks } from '~/sections/HowItWorks'

export default function Home() {
  return (
    <Layout>
      <Hero />
      <HowItWorks />
      <Mission />
    </Layout>
  )
}
