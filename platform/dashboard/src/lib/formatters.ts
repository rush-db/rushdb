import type { DatetimeObject } from '@rushdb/javascript-sdk'

import type { ISO8601 } from '~/types'

export const numberCompact = Intl.NumberFormat('en', { notation: 'compact' })

export const percentFormatter = new Intl.NumberFormat('en-US', {
  maximumSignificantDigits: 2,
  style: 'percent'
})

export const currencyFormatters = {
  usd: new Intl.NumberFormat('en-US', {
    maximumSignificantDigits: 3,
    style: 'currency',
    currency: 'USD'
  })
}

export const collectDateToString = (collectDate: DatetimeObject) => {
  return `${collectDate.$day}/${collectDate.$month}/${collectDate.$year}`
}

export const formatIsoToLocal = (iso: ISO8601) => new Date(iso).toLocaleDateString()
