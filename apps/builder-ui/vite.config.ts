import { defineConfig } from 'vite'
import { svelte } from '@sveltejs/vite-plugin-svelte'

export default defineConfig({
  plugins: [svelte()],
  server: {
    proxy: {
      '/api': {
        target: process.env.VITE_BUILDER_API_TARGET ?? 'http://127.0.0.1:18110',
        changeOrigin: true,
      },
      '/runtime-api': {
        target: process.env.VITE_RUNTIME_API_TARGET ?? 'http://127.0.0.1:18090',
        changeOrigin: true,
      },
    },
  },
})