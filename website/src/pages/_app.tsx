import "~/styles/globals.css"
import type { AppProps } from "next/app"
import Script from "next/script"

import { jetBrainsMono, manrope, dmSerifDisplay } from "~/styles/fonts"
import { useRef } from "react"
import cx from "classnames"

const isProd = process.env.NODE_ENV === "production"

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
          <Script
            strategy="lazyOnload"
            src={`https://www.googletagmanager.com/gtag/js?id=G-CLCR2SYDC6`}
          />
          {/*<Script src="https://cdn.jsdelivr.net/npm/pathseg@1.2.1/pathseg.min.js"></Script>*/}
          <Script id="gtm" strategy="lazyOnload">
            {`
        window.dataLayer = window.dataLayer || [];
        function gtag(){dataLayer.push(arguments);}
        gtag('js', new Date());
        
        gtag('config', 'G-CLCR2SYDC6');
      `}
          </Script>
        </>
      )}

      <main
        className={cx(
          jetBrainsMono.variable,
          dmSerifDisplay.variable,
          manrope.variable,
        )}
        ref={ref}
      >
        <Component {...pageProps} />
      </main>
    </>
  )
}
