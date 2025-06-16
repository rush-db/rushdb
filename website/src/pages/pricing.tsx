import { Layout } from '~/components/Layout'
import { Pricing } from '~/sections/Pricing'

export default function PricingPage() {
  return (
    <Layout
      title="Pricing"
      description="Explore RushDB’s flexible pricing plans designed for startups, developers, and enterprises. Get the power of a graph database with instant setup, zero configuration, and best-in-class developer experience. Scale effortlessly with transparent and predictable pricing. Start for free!"
    >
      <Pricing />
    </Layout>
  )
}
