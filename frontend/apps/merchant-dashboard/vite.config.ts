import { fileURLToPath, URL } from 'node:url'

import { defineConfig } from 'vite'
import vue from '@vitejs/plugin-vue'
import vueDevTools from 'vite-plugin-vue-devtools'
import tailwindcss from '@tailwindcss/vite'
import svgLoader from 'vite-svg-loader'
import { VitePWA } from 'vite-plugin-pwa'

// https://vite.dev/config/
export default defineConfig({
  plugins: [
    vue(),
    vueDevTools(),
    tailwindcss(),
    svgLoader({ svgoConfig: {} }) as any,
    VitePWA({
      registerType: 'autoUpdate',
      includeAssets: ['brand/favicon.svg', 'apple-touch-icon.png'],
      manifest: {
        name: 'IronixPay Merchant Dashboard',
        short_name: 'IronixPay',
        description: 'Manage crypto payments, sessions, billing, and webhooks.',
        theme_color: '#2563eb',
        background_color: '#0f172a',
        display: 'standalone',
        icons: [
          {
            src: 'brand/pwa-192x192.png',
            sizes: '192x192',
            type: 'image/png',
          },
          {
            src: 'brand/pwa-512x512.png',
            sizes: '512x512',
            type: 'image/png',
          },
          {
            src: 'brand/pwa-maskable-512x512.png',
            sizes: '512x512',
            type: 'image/png',
            purpose: 'maskable',
          },
        ],
      },
      workbox: {
        // Precache all static assets (App Shell)
        globPatterns: ['**/*.{js,css,html,ico,png,svg,woff2}'],
        // SPA: serve index.html for navigation requests
        navigateFallback: 'index.html',
        // Exclude API routes from navigation fallback — prevents SW from
        // returning index.html when someone navigates to /api/* or /v1/* directly
        navigateFallbackDenylist: [/^\/api\//, /^\/v1\//],
        // No runtimeCaching — financial data must never be served from cache
      },
    }),
  ],
  server: {
    port: 5173,
    proxy: {
      '/api': {
        target: 'http://localhost:3000',
        changeOrigin: true,
      },
      '/v1': {
        target: 'http://localhost:3000',
        changeOrigin: true,
      },
    },
  },
  resolve: {
    dedupe: ['vue'],
    alias: {
      '@': fileURLToPath(new URL('./src', import.meta.url)),
    },
  },
})
