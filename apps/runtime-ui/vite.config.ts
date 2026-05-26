import { defineConfig } from 'vite'
import { svelte } from '@sveltejs/vite-plugin-svelte'

// https://vite.dev/config/
export default defineConfig({
  plugins: [svelte()],
  server: {
    proxy: {
      '/api': {
        target: process.env.VITE_RUNTIME_API_TARGET ?? 'http://127.0.0.1:18090',
        changeOrigin: true,
      },
    },
  },
})
