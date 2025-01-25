import { useEffect, useState } from 'react'
import { X } from 'lucide-react'
import { Button } from '~/components/Button'
import Link from 'next/link'

export default function CookieNotification() {
  const [isVisible, setIsVisible] = useState(false)

  useEffect(() => {
    const hasConsented = localStorage.getItem('cookieConsent')
    if (hasConsented === null) {
      setIsVisible(true)
    }
  }, [])

  const handleAccept = () => {
    localStorage.setItem('cookieConsent', 'true')
    setIsVisible(false)
  }

  const handleReject = () => {
    localStorage.setItem('cookieConsent', 'false')
    setIsVisible(false)
  }

  if (!isVisible) return null

  return (
    <div className="fixed bottom-4 right-4 z-10 ml-4 max-w-xs rounded-lg border border-gray-200 bg-white p-4 shadow-lg md:max-w-sm md:p-2">
      <div className="mb-4 flex items-center justify-between md:mb-2">
        <span className="text-lg leading-4">🍪</span>
        <Button onClick={handleReject} size="small" variant="secondary">
          <X className="h-4 w-4" />
        </Button>
      </div>
      <p className="text-content3 mb-4 text-sm md:mb-2">
        We use cookies to enhance your experience. By clicking "Accept," you agree to{' '}
        <Link href="/cookie-policy" target="_blank" className="text-blue-500 underline hover:text-blue-700">
          our use of cookies
        </Link>
        .
      </p>

      <Button variant="outline" size="small" onClick={handleAccept} className="w-full">
        Accept
      </Button>
    </div>
  )
}
