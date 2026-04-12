import { PropsWithChildren } from 'react'
import { Footer } from '~/components/Layout/Footer'
import { Meta } from '~/components/Meta'
import { LPHeader } from '~/components/lp/LPHeader'
import CookieNotification from '~/components/CookiesConsent'

export function LPLayout({
  title,
  description,
  image,
  children
}: PropsWithChildren<{ title?: string; description?: string; image?: string }>) {
  return (
    <>
      <Meta title={title} description={description} image={image} />
      <div style={{ background: 'var(--lp-bg)', minHeight: '100vh' }}>
        <LPHeader />
        <div className="pt-16">{children}</div>
        <Footer />
      </div>
      <CookieNotification />
    </>
  )
}
