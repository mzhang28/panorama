import { defineConfig } from 'vite'
import react from '@vitejs/plugin-react'
import { federation } from '@module-federation/vite'

const BACKEND_PORT = process.env.VITE_BACKEND_PORT || '3000'
const FRONTEND_PORT = Number(process.env.VITE_PORT) || 5173

export default defineConfig({
  plugins: [
    react(),
    federation({
      name: 'panorama_host',
      // No static remotes — all loaded dynamically at runtime
      shared: {
        react: { singleton: true, requiredVersion: '^19.0.0' },
        'react-dom': { singleton: true, requiredVersion: '^19.0.0' },
        '@tanstack/react-query': { singleton: true, requiredVersion: '^5.60.0' },
      },
    }),
  ],
  server: {
    port: FRONTEND_PORT,
    proxy: {
      '/api': `http://127.0.0.1:${BACKEND_PORT}`,
      '/plugin': `http://127.0.0.1:${BACKEND_PORT}`,
    },
    allowedHosts: ['ephemeral']
  },
})
