import '~/styles/globals.css'
import type { AppProps } from 'next/app'

import { jetBrainsMono, manrope } from '~/styles/fonts'
import React, { useRef } from 'react'
import cx from 'classnames'
import { CodingLanguageProvider } from '~/contexts/CodingLanguage'
import Script from 'next/script'
import { generateJsonLd } from '~/utils/jsonLd'
import { useRouter } from 'next/router'
import { getAbsoluteURL } from '~/utils'
import Head from 'next/head'

const isProd = process.env.NODE_ENV === 'production'

const gtmScript = `window.dataLayer = window.dataLayer || [];
function gtag(){dataLayer.push(arguments);}
gtag('js', new Date());
gtag('config', 'G-Y678D4CC1J');`

export default function App({ Component, pageProps }: AppProps) {
  const ref = useRef<HTMLElement>(null)

  const router = useRouter()

  const route = router.asPath

  let jsonLdData = [
    generateJsonLd('breadcrumb', {
      items: [
        { name: 'Home', url: 'https://rushdb.com/' },
        ...route
          .split('/')
          .filter((p) => p)
          .map((p, index, arr) => ({
            name: p.charAt(0).toUpperCase() + p.slice(1).replace('-', ' '),
            url: getAbsoluteURL(`/${arr.slice(0, index + 1).join('/')}`)
          }))
      ]
    })
  ]
  if (route === '/') {
    jsonLdData.push(generateJsonLd('homepage', {}))
  } else if (route === '/blog') {
    jsonLdData.push(generateJsonLd('blogRoot', {}))
  } else if (route === '/pricing') {
    jsonLdData.push(generateJsonLd('pricing', {}))
  } else if (route === '/privacy-policy') {
    jsonLdData.push(generateJsonLd('legal', { url: getAbsoluteURL(route), name: 'Privacy Policy' }))
  } else if (route === '/terms-of-service') {
    jsonLdData.push(generateJsonLd('legal', { url: getAbsoluteURL(route), name: 'Terms of Service' }))
  }

  return (
    <>
      <Head>
        <script type="application/ld+json" dangerouslySetInnerHTML={{ __html: JSON.stringify(jsonLdData) }} />
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

      <main className={cx(jetBrainsMono.variable, manrope.variable)} ref={ref}>
        <CodingLanguageProvider>
          <Component {...pageProps} />
        </CodingLanguageProvider>
      </main>
    </>
  )
}
