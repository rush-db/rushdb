import React from 'react'
import NextDocument, { Html, Head, Main, NextScript } from 'next/document'

export default class Document extends NextDocument {
  render() {
    return (
      <Html lang="en">
        <Head>
          <meta charSet="UTF-8" />
          <link rel="stylesheet" href="https://cdn.jsdelivr.net/gh/devicons/devicon@latest/devicon.min.css" />
        </Head>
        <body>
          <script
            dangerouslySetInnerHTML={{
              __html: `
                (function() {
                  try {
                    var theme = localStorage.getItem('theme');
                    var prefersDark = window.matchMedia('(prefers-color-scheme: dark)').matches;
                    var shouldBeDark = theme === 'dark' || (!theme && prefersDark);

                    if (shouldBeDark) {
                      document.documentElement.classList.add('dark');
                    } else {
                      document.documentElement.classList.remove('dark');
                    }

                    // Update meta theme color immediately
                    var metaThemeColor = document.querySelector('meta[name="theme-color"]');
                    if (metaThemeColor) {
                      metaThemeColor.setAttribute('content', shouldBeDark ? '#1a1a1a' : '#3f81ff');
                    }
                  } catch (e) {
                    // Fallback to light theme if anything fails
                    document.documentElement.classList.remove('dark');
                  }
                })();
              `
            }}
          />
          <Main />
          <NextScript />
        </body>
      </Html>
    )
  }
}
