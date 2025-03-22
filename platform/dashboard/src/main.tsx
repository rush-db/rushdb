import React from 'react'
import ReactDOM from 'react-dom/client'
import 'virtual:svg-icons-register'

import { App } from './App'
import './index.css'
import { HelmetProvider } from 'react-helmet-async'

if (import.meta.env.NODE_ENV === 'development') {
  import('~/lib/logger')
}

ReactDOM.createRoot(document.getElementById('root') as HTMLElement).render(
  <React.StrictMode>
    <HelmetProvider>
      <App />
    </HelmetProvider>
  </React.StrictMode>
)
