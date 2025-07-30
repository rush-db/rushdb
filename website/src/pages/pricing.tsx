import { Layout } from '~/components/Layout'
import { Pricing } from '~/sections/Pricing'
import { GetServerSideProps } from 'next'
import { BillingData } from '~/components/pricing-types'

interface PricingPageProps {
  billingData: BillingData | null
}

export default function PricingPage({ billingData }: PricingPageProps) {
  return (
    <Layout
      title="Pricing"
      description="Explore RushDB's flexible pricing plans designed for startups, developers, and enterprises. Get the power of a graph database with instant setup, zero configuration, and best-in-class developer experience. Scale effortlessly with transparent and predictable pricing. Start for free!"
    >
      <Pricing billingData={billingData} />
    </Layout>
  )
}

export const getStaticProps = async () => {
  try {
    const res = await fetch('https://billing.rushdb.com/api/prices')

    let billingData: BillingData | null = null

    if (res.ok) {
      billingData = await res.json()
    } else {
      console.warn('Failed to fetch billing data, using tiered pricing only')
    }

    return {
      props: {
        billingData
      },
      revalidate: 3600 // Optional: ISR, revalidate every hour
    }
  } catch (error) {
    console.error('Failed to fetch billing data:', error)
    return {
      props: {
        billingData: null
      },
      revalidate: 3600 // Optional: ISR
    }
  }
}
