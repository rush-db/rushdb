import { useEffect, useState } from 'react'
import { X } from 'lucide-react'
import { Button } from '~/elements/Button'
import { Link } from '~/elements/Link'

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
    <div className="bg-fill outline-stroke fixed bottom-4 right-4 z-50 ml-4 max-w-xs rounded-lg p-4 shadow-lg outline outline-1 outline-offset-0 md:max-w-sm md:p-2">
      <div className="mb-4 flex items-center justify-between md:mb-2">
        <span className="text-lg leading-4">🍪</span>
        <Button onClick={handleReject} size="small" variant="outline" className="bg-fill">
          <X className="h-4 w-4" />
        </Button>
      </div>
      <p className="text-content3 mb-4 text-sm md:mb-2">
        We use cookies to enhance your experience. By clicking "Accept," you agree to{' '}
        <Link href="https://rushdb.com/cookie-policy" target="_blank" size="small">
          our use of cookies
        </Link>
        .
      </p>

      <Button variant="outline" size="small" onClick={handleAccept} className="bg-fill w-full">
        Accept
      </Button>
    </div>
  )
}
