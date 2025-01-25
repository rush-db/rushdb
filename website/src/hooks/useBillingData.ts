import { useState, useEffect } from 'react'

// Тип данных, которые возвращает `billing.rushdb.com/api/prices`
type BillingData = {
  pro: {
    month: {
      amount: number
      priceId: string
      productId: string
    }
    annual: {
      amount: number
      priceId: string
      productId: string
    }
  }
}

export function useBillingData() {
  const [data, setData] = useState<BillingData | null>(null)

  useEffect(() => {
    async function fetchBillingData() {
      try {
        const res = await fetch('https://billing.rushdb.com/api/prices')

        if (!res.ok) {
          return
        }

        const json = await res.json()

        setData(json)
      } catch (err) {
        console.error('Failed to fetch billing data:', err)
      }
    }

    fetchBillingData()
  }, [])

  return data
}
