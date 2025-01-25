import { useState, useEffect } from 'react'

type PaidStartPlanId = 'start'
type PaidPlanId = 'pro'
type PlanPeriod = 'annual' | 'month'

type IncomingPlanData = {
  amount: number
  priceId: string
  productId: string
}

type BillingData = Record<PaidStartPlanId | PaidPlanId, Record<PlanPeriod, IncomingPlanData>>

export function useBillingData() {
  const [data, setData] = useState<BillingData | null>(null)
  const [loading, setLoading] = useState(true)

  useEffect(() => {
    async function fetchBillingData() {
      try {
        setLoading(true)
        const res = await fetch('https://billing.rushdb.com/api/prices')

        if (!res.ok) {
          return
        }

        const json = await res.json()

        setData(json)
      } catch (err) {
        console.error('Failed to fetch billing data:', err)
      } finally {
        setLoading(false)
      }
    }

    fetchBillingData()
  }, [])

  return { data, loading }
}
