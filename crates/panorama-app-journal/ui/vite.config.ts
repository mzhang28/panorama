import { defineConfig } from 'vite'
import react from '@vitejs/plugin-react'
import { federation } from '@module-federation/vite'

export default defineConfig({
  base: './',
  plugins: [
    react(),
    federation({
      name: 'io_mzhang_panorama_journal',
      filename: 'remoteEntry.js',
      dts: false,
      exposes: { './App': './src/App.tsx' },
      shared: {
        react: { singleton: true, requiredVersion: '^19.0.0' },
        'react-dom': { singleton: true, requiredVersion: '^19.0.0' },
        '@tanstack/react-query': { singleton: true, requiredVersion: '^5.60.0' },
      },
    }),
  ],
  build: { target: 'es2022' },
})
