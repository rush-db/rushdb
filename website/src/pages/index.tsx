import { Layout } from '~/components/Layout'
import { Hero } from '~/sections/Hero'

import { Mission } from '~/sections/Mission'
import { UseCases } from '~/sections/UseCases'

export default function Home() {
  return (
    <Layout>
      <Hero />
      <Mission />
      <UseCases />
    </Layout>
  )
}
