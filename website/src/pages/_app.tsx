import '~/styles/globals.css'
import type { AppProps } from 'next/app'

import { jetBrainsMono, manrope } from '~/styles/fonts'
import React, { useMemo } from 'react'
import cx from 'classnames'
import { CodingLanguageProvider } from '~/contexts/CodingLanguage'
import Script from 'next/script'
import { generateJsonLd } from '~/utils/jsonLd'
import { useRouter } from 'next/router'
import { getAbsoluteURL } from '~/utils'
import Head from 'next/head'
import { metaThemeColor } from '~/config/theme'

const isProd = process.env.NODE_ENV === 'production'

const gtmScript = `window.dataLayer = window.dataLayer || [];
function gtag(){dataLayer.push(arguments);}
gtag('js', new Date());
gtag('config', 'G-Y678D4CC1J');`

export default function App({ Component, pageProps }: AppProps) {
  const router = useRouter()
  const route = router.asPath

  const jsonLdData = useMemo(() => {
    const items = route
      .split('/')
      .filter(Boolean)
      .map((p, index, arr) => ({
        name: p.charAt(0).toUpperCase() + p.slice(1).replace('-', ' '),
        url: getAbsoluteURL(`/${arr.slice(0, index + 1).join('/')}`)
      }))

    const data = [
      generateJsonLd('breadcrumb', { items: [{ name: 'Home', url: 'https://rushdb.com/' }, ...items] })
    ]

    if (route === '/') {
      data.push(generateJsonLd('homepage', {}))
    } else if (route === '/blog') {
      data.push(generateJsonLd('blogRoot', {}))
    } else if (route === '/pricing') {
      data.push(generateJsonLd('pricing', {}))
    } else if (route === '/privacy-policy') {
      data.push(generateJsonLd('legal', { url: getAbsoluteURL(route), name: 'Privacy Policy' }))
    } else if (route === '/terms-of-service') {
      data.push(generateJsonLd('legal', { url: getAbsoluteURL(route), name: 'Terms of Service' }))
    }

    return data
  }, [route])

  return (
    <>
      <Head>
        <link rel="icon" type="image/svg+xml" href="/favicon.svg" />
        <link rel="icon" type="image/png" href="/favicon.png" />
        <link rel="apple-touch-icon" sizes="180x180" href="/apple-touch-icon.png" />
        <link rel="icon" type="image/png" sizes="32x32" href="/favicon-32x32.png" />
        <link rel="icon" type="image/png" sizes="16x16" href="/favicon-16x16.png" />
        <link rel="manifest" href="/site.webmanifest" />
        <meta name="msapplication-TileColor" content={metaThemeColor} />
        <meta name="theme-color" content={metaThemeColor} />
        <link rel="preconnect" href="https://fonts.googleapis.com" crossOrigin="anonymous" />
        <link rel="preconnect" href="https://fonts.gstatic.com" crossOrigin="anonymous" />

        <script type="application/ld+json" dangerouslySetInnerHTML={{ __html: JSON.stringify(jsonLdData) }} />

        <meta name="referrer" content="strict-origin-when-cross-origin" />
      </Head>
      <style jsx global>{`
        html,
        body {
          font-family: ${manrope.style.fontFamily};
        }
      `}</style>
      {isProd && (
        <>
          <Script strategy="lazyOnload" src={`https://www.googletagmanager.com/gtag/js?id=G-Y678D4CC1J`} />
          <Script id="gtm" strategy="lazyOnload">
            {gtmScript}
          </Script>
        </>
      )}

      <main className={cx(jetBrainsMono.variable, manrope.variable)}>
        <CodingLanguageProvider>
          <Component {...pageProps} />
        </CodingLanguageProvider>
      </main>
    </>
  )
}
