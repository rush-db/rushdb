/// <reference types="vite/client" />

interface Window {
  dataLayer: Record<string, unknown>[]
}

interface ImportMetaEnv {
  readonly VITE_BACKEND_BASE_URL: string
  readonly VITE_STRIPE_PUBLIC_KEY: string
  readonly VITE_GTM_ID: string
}

interface ImportMeta {
  readonly env: ImportMetaEnv
}
