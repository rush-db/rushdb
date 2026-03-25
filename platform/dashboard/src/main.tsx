import React from 'react'
import ReactDOM from 'react-dom/client'
import 'virtual:svg-icons-register'

import { QueryClientProvider } from '@tanstack/react-query'

import { App } from './App'
import './index.css'
import { HelmetProvider } from 'react-helmet-async'
import { queryClient } from '~/lib/queryClient'

if (import.meta.env.NODE_ENV === 'development') {
  import('~/lib/logger')
}

ReactDOM.createRoot(document.getElementById('root') as HTMLElement).render(
  <React.StrictMode>
    <HelmetProvider>
      <QueryClientProvider client={queryClient}>
        <App />
      </QueryClientProvider>
    </HelmetProvider>
  </React.StrictMode>
)
