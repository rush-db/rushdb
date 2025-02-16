import { createContext, ReactNode, useState } from 'react'

export const CodingLanguage = createContext<{ language: string; setLanguage: (value: string) => void }>({
  language: 'typescript',
  setLanguage: (value: string) => {}
})

export const CodingLanguageProvider = ({ children }: { children: ReactNode }) => {
  const [language, setLanguage] = useState<string>('typescript')

  return (
    <CodingLanguage.Provider
      value={{
        language,
        setLanguage
      }}
    >
      {children}
    </CodingLanguage.Provider>
  )
}
