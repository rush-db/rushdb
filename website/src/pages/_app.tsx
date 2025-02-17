import '~/styles/globals.css'
import type { AppProps } from 'next/app'

import { jetBrainsMono, manrope } from '~/styles/fonts'
import React, { useRef } from 'react'
import cx from 'classnames'
import { CodingLanguageProvider } from '~/contexts/CodingLanguage'
import Script from 'next/script'

const isProd = process.env.NODE_ENV === 'production'

const gtmScript = `window.dataLayer = window.dataLayer || [];
function gtag(){dataLayer.push(arguments);}
gtag('js', new Date());
gtag('config', 'G-Y678D4CC1J');`

export default function App({ Component, pageProps }: AppProps) {
  const ref = useRef<HTMLElement>(null)

  return (
    <>
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
