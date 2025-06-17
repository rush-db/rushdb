import { ComponentPropsWithoutRef, PropsWithChildren } from 'react'
import { Footer } from './Footer'
import { Header } from './header'
import { Meta } from '~/components/Meta'
import classNames from 'classnames'
import CookieNotification from '~/components/CookiesConsent'

export function Layout({
  description,
  title,
  image,
  children,
  className
}: PropsWithChildren<
  Pick<ComponentPropsWithoutRef<typeof Meta>, 'title' | 'description' | 'image'> & {
    className?: string
  }
>) {
  return (
    <>
      <Meta title={title} description={description} image={image} />

      <Header />

      <div className={classNames('min-h-screen pt-24 md:pt-0', className)}>{children}</div>

      <Footer />
      <CookieNotification />
    </>
  )
}
