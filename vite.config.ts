import { defineConfig } from 'vite'
import vue from '@vitejs/plugin-vue'
import { fileURLToPath, URL } from 'node:url'

export default defineConfig({
  plugins: [vue()],
  clearScreen: false,
  resolve: {
    alias: { '@': fileURLToPath(new URL('./src', import.meta.url)) },
  },
  server: {
    port: 5173,
    strictPort: true,
    host: process.env.TAURI_DEV_HOST || 'localhost',
    hmr: process.env.TAURI_DEV_HOST
      ? { protocol: 'ws', host: process.env.TAURI_DEV_HOST, port: 5173 }
      : undefined,
  },
  envPrefix: ['VITE_', 'TAURI_ENV_*'],
  build: {
    // R-02: konservatives Ziel wegen WebView-Fragmentierung auf alten Tablets.
    // Android 10 liefert im Minimum Chromium 8x; chrome87 laesst Puffer.
    target: 'chrome87',
    minify: 'esbuild',
    sourcemap: false,
    assetsInlineLimit: 0,
  },
})
