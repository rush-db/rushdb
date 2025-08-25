import { QueryClient } from '@tanstack/react-query'

// Singleton QueryClient for non-hook based query usage (bridging to nanostores)
export const queryClient = new QueryClient()
