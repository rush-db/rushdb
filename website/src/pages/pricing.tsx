import { Layout } from '~/components/Layout'
import { Pricing } from '~/sections/Pricing'
import { GetStaticProps } from 'next'

type PaidStartPlanId = 'start'
type PaidPlanId = 'pro'
type PlanPeriod = 'annual' | 'month'

type IncomingPlanData = {
  amount: number
  priceId: string
  productId: string
}

type BillingData = Record<PaidStartPlanId | PaidPlanId, Record<PlanPeriod, IncomingPlanData>>

interface PricingPageProps {
  billingData: BillingData | null
}

export default function PricingPage({ billingData }: PricingPageProps) {
  return (
    <Layout
      title="Pricing"
      description="Explore RushDB's flexible pricing plans designed for startups, developers, and enterprises. Get the power of a graph database with instant setup, zero configuration, and best-in-class developer experience. Scale effortlessly with transparent and predictable pricing. Start for free!"
    >
      <Pricing initialBillingData={billingData} />
    </Layout>
  )
}

export const getStaticProps: GetStaticProps<PricingPageProps> = async () => {
  try {
    const res = await fetch('https://billing.rushdb.com/api/prices')

    if (!res.ok) {
      return {
        props: {
          billingData: null
        },
        revalidate: 300 // Revalidate every 5 minutes on error
      }
    }

    const billingData = await res.json()

    return {
      props: {
        billingData
      },
      revalidate: 3600 // Revalidate every hour
    }
  } catch (error) {
    console.error('Failed to fetch billing data:', error)
    return {
      props: {
        billingData: null
      },
      revalidate: 300 // Revalidate every 5 minutes on error
    }
  }
}
