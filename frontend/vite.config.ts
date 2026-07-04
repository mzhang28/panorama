import { defineConfig } from 'vite'
import react from '@vitejs/plugin-react'
import { federation } from '@module-federation/vite'
import path from 'path'

const BACKEND_PORT = process.env.VITE_BACKEND_PORT || '3000'
const FRONTEND_PORT = Number(process.env.VITE_PORT) || 5173

const resolvePlugin = (name: string) =>
  path.resolve(__dirname, `../crates/panorama-app-${name}/ui/src/App.tsx`)

export default defineConfig(({ mode }) => ({
  plugins: [
    react(),
    ...(mode === 'production'
      ? [federation({
          name: 'panorama_host',
          shared: {
            react: { singleton: true, requiredVersion: '^19.0.0' },
            'react-dom': { singleton: true, requiredVersion: '^19.0.0' },
            '@tanstack/react-query': { singleton: true, requiredVersion: '^5.60.0' },
          },
        })]
      : []),
  ],
  resolve: {
    // Rollup needs these aliases in all modes because both dev.tsx and
    // prod.tsx are parsed, even though only one is loaded at runtime.
    alias: {
      'panorama-plugin-journal-ui':  resolvePlugin('journal'),
      'panorama-plugin-grafana-ui':  resolvePlugin('grafana'),
      'panorama-plugin-wakatime-ui': resolvePlugin('wakatime'),
      'panorama-plugin-trips-ui':    resolvePlugin('trips'),
      'panorama-plugin-beli-ui':     resolvePlugin('beli'),
      'panorama-plugin-subsonic-ui': resolvePlugin('subsonic'),
      'panorama-plugin-files-ui':    resolvePlugin('files'),
    },
  },
  server: {
    port: FRONTEND_PORT,
    proxy: {
      '/api': `http://127.0.0.1:${BACKEND_PORT}`,
      '/plugin': `http://127.0.0.1:${BACKEND_PORT}`,
    },
    allowedHosts: ['ephemeral'],
  },
}))
