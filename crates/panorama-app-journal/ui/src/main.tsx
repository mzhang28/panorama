import React from 'react'
import ReactDOM from 'react-dom/client'
import { QueryClient, QueryClientProvider } from '@tanstack/react-query'

// Standalone dev entry — mounts the Journal app independently.
// In production, this component is loaded by the Panorama host via Module Federation.
import App from './App'

const queryClient = new QueryClient()

ReactDOM.createRoot(document.getElementById('root')!).render(
  <React.StrictMode>
    <QueryClientProvider client={queryClient}>
      <App pluginId="com.panorama.journal" />
    </QueryClientProvider>
  </React.StrictMode>,
)
