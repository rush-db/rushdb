import React from 'react'
import ReactDOM from 'react-dom/client'
import 'virtual:svg-icons-register'

import { App } from './App'
import './index.css'

if (import.meta.env.DEV) {
  import('~/lib/logger')
} else if (import.meta.env.SENTRY_DSN) {
  import('@sentry/react').then(({ init, BrowserTracing, Replay }) => {
    init({
      dsn: import.meta.env.SENTRY_DSN,
      integrations: [new BrowserTracing(), new Replay()],
      // Performance Monitoring
      tracesSampleRate: 1.0, // Capture 100% of the transactions
      // Session Replay
      replaysSessionSampleRate: 0.1, // This sets the sample rate at 10%. You may want to change it to 100% while in development and then sample at a lower rate in production.
      replaysOnErrorSampleRate: 1.0 // If you're not already sampling the entire session, change the sample rate to 100% when sampling sessions where errors occur.
    })
  })
}

ReactDOM.createRoot(document.getElementById('root') as HTMLElement).render(
  <React.StrictMode>
    <App />
  </React.StrictMode>
)
