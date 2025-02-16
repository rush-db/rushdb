import { Layout } from '~/components/Layout'
import { Hero } from '~/sections/Hero'

import { Mission } from '~/sections/Mission'
import { HowItWorks } from '~/sections/HowItWorks'
import { createContext, useState } from 'react'

export default function Home() {
  return (
    <Layout>
      <Hero />
      <HowItWorks />
      <Mission />
    </Layout>
  )
}
