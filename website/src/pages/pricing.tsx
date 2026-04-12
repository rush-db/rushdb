import { LPLayout } from '~/components/lp/LPLayout'
import { LPPricing } from '~/sections/lp/Pricing'
import { BillingData } from '~/components/pricing-types'

interface PricingPageProps {
  billingData: BillingData | null
}

export default function PricingPage({ billingData }: PricingPageProps) {
  return (
    <LPLayout
      title="Pricing"
      description="RushDB pricing is simple: pay for knowledge created. Standard reads are always free. Free plan includes 100K KU/month. Pro starts at $29/month with 10M KU included and overage billing. Scale offers pure usage-based pricing at $2/M KU. Enterprise for custom deployments. Start building free — no credit card required."
    >
      <LPPricing billingData={billingData} showKuDefinition />
    </LPLayout>
  )
}

export const getStaticProps = async () => {
  try {
    const billingServiceUrl = process.env.BILLING_SERVICE_URL || 'https://billing.rushdb.com'
    const res = await fetch(`${billingServiceUrl}/api/prices`)

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
