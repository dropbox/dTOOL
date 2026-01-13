import { defineConfig } from 'vite'
import react from '@vitejs/plugin-react'

// https://vitejs.dev/config/
export default defineConfig({
  plugins: [react()],
  server: {
    port: 5173,
    host: true,
    // Proxy API and WebSocket requests to the websocket server
    proxy: {
      '/ws': {
        target: 'ws://localhost:3002',
        ws: true,
        changeOrigin: true,
      },
      '/health': {
        target: 'http://localhost:3002',
        changeOrigin: true,
      },
      '/version': {
        target: 'http://localhost:3002',
        changeOrigin: true,
      },
      '/metrics': {
        target: 'http://localhost:3002',
        changeOrigin: true,
      },
      '/api': {
        target: 'http://localhost:3002',
        changeOrigin: true,
      },
    },
  },
  preview: {
    port: 5173,
    host: true,
    // Note: vite preview doesn't support proxy - use nginx or run dev server
  },
})
