import React, { useEffect, useState } from 'react'
import SearchModal from '@site/src/components/SearchModal'

export default function Root({ children }: { children: React.ReactNode }) {
  const [searchOpen, setSearchOpen] = useState(false)

  useEffect(() => {
    const handleOpen = () => setSearchOpen(true)
    const handleKey = (e: KeyboardEvent) => {
      if ((e.metaKey || e.ctrlKey) && e.key === 'k') {
        e.preventDefault()
        setSearchOpen(true)
      }
    }
    document.addEventListener('rushdb:open-search', handleOpen)
    document.addEventListener('keydown', handleKey)
    return () => {
      document.removeEventListener('rushdb:open-search', handleOpen)
      document.removeEventListener('keydown', handleKey)
    }
  }, [])

  return (
    <>
      {children}
      <SearchModal isOpen={searchOpen} onClose={() => setSearchOpen(false)} />
    </>
  )
}
