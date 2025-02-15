import '~/styles/globals.css'
import type { AppProps } from 'next/app'

import { jetBrainsMono, manrope } from '~/styles/fonts'
import { useRef } from 'react'
import cx from 'classnames'

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

      <main className={cx(jetBrainsMono.variable, manrope.variable)} ref={ref}>
        <Component {...pageProps} />
      </main>
    </>
  )
}
