import { Layout } from '~/components/Layout'
import { Hero } from '~/sections/Hero'

import { Mission } from '~/sections/Mission'
import { HowItWorks } from '~/sections/HowItWorks'
import { createContext, useState } from 'react'

export const CodingLanguage = createContext<{ language: string; setLanguage: (value: string) => void }>({
  language: 'typescript',
  setLanguage: (value: string) => {}
})

export default function Home() {
  const [language, setLanguage] = useState<string>('typescript')
  return (
    <CodingLanguage.Provider
      value={{
        language,
        setLanguage
      }}
    >
      <Layout>
        <Hero />
        <HowItWorks />
        <Mission />
      </Layout>
    </CodingLanguage.Provider>
  )
}
