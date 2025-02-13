import React from 'react'
import NextDocument, { Html, Head, Main, NextScript } from 'next/document'
import { metaThemeColor } from '~/config/theme'
import Script from 'next/script'

const isProd = process.env.NODE_ENV === 'production'

const gtmScript = `window.dataLayer = window.dataLayer || [];
function gtag(){dataLayer.push(arguments);}
gtag('js', new Date());
gtag('config', 'G-Y678D4CC1J');`

const rb2bScript = `!function () {var reb2b = window.reb2b = window.reb2b || [];
if (reb2b.invoked) return;reb2b.invoked = true;reb2b.methods = ["identify", "collect"];
reb2b.factory = function (method) {return function () {var args = Array.prototype.slice.call(arguments);
args.unshift(method);reb2b.push(args);return reb2b;};};
for (var i = 0; i < reb2b.methods.length; i++) {var key = reb2b.methods[i];reb2b[key] = reb2b.factory(key);}
reb2b.load = function (key) {var script = document.createElement("script");script.type = "text/javascript";script.async = true;
script.src = "https://s3-us-west-2.amazonaws.com/b2bjsstore/b/" + key + "/Z6PVLH54JP6R.js.gz";
var first = document.getElementsByTagName("script")[0];
first.parentNode.insertBefore(script, first);};
reb2b.SNIPPET_VERSION = "1.0.1";reb2b.load("Z6PVLH54JP6R");}();`

export default class Document extends NextDocument {
  render() {
    return (
      <Html lang="en">
        <Head>
          <link rel="icon" type="image/svg+xml" href="/favicon.svg" />
          <link rel="icon" type="image/png" href="/favicon.png" />
          <link rel="apple-touch-icon" sizes="180x180" href="/apple-touch-icon.png" />
          <link rel="icon" type="image/png" sizes="32x32" href="/favicon-32x32.png" />
          <link rel="icon" type="image/png" sizes="16x16" href="/favicon-16x16.png" />
          <link rel="manifest" href="/site.webmanifest" />
          <meta name="msapplication-TileColor" content={metaThemeColor} />
          <meta name="theme-color" content={metaThemeColor} />
          <link rel="preconnect" href="https://fonts.googleapis.com" />
          <link rel="preconnect" href="https://fonts.gstatic.com" crossOrigin="" />
          <link rel="stylesheet" href="https://cdn.jsdelivr.net/gh/devicons/devicon@latest/devicon.min.css" />
          {isProd && (
            <>
              <Script
                strategy="lazyOnload"
                src={`https://www.googletagmanager.com/gtag/js?id=G-Y678D4CC1J`}
              />
              <Script id="gtm" strategy="lazyOnload">
                {gtmScript}
              </Script>
              <Script id="rb2b" strategy="lazyOnload">
                {rb2bScript}
              </Script>
            </>
          )}
        </Head>
        <body>
          <Main />
          <NextScript />
        </body>
      </Html>
    )
  }
}
