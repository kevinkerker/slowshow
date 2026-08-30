import { defineConfig } from 'vitest/config'
import vue from '@vitejs/plugin-vue'
import { fileURLToPath, URL } from 'node:url'

export default defineConfig({
  plugins: [vue()],
  resolve: {
    alias: { '@': fileURLToPath(new URL('./src', import.meta.url)) },
  },
  test: {
    environment: 'jsdom',
    setupFiles: ['./tests/setup.ts'],
    // .mjs ebenfalls: die Build-Skripte unter scripts/ sind ESM ohne Typen,
    // und `vue-tsc` schaut nicht hinein. Ein Test in .ts muesste sie erst
    // typisieren, nur um eine reine Funktion zu pruefen.
    include: ['src/**/*.test.ts', 'tests/**/*.test.ts', 'tests/**/*.test.mjs'],
    coverage: {
      provider: 'v8',
      include: ['src/lib/**/*.ts', 'src/stores/**/*.ts'],
      reporter: ['text', 'html'],
    },
  },
})
