import { defineConfig } from 'vite'
import react from '@vitejs/plugin-react'
import path from 'path'

// https://v2.tauri.app/start/frontend/vite/
export default defineConfig({
  plugins: [react()],
  resolve: {
    alias: {
      '@': path.resolve(__dirname, './src'),
    },
  },
  // Tauri expects a fixed port for dev server
  server: {
    port: 5173,
    strictPort: true,
  },
  // Env variables for Tauri
  envPrefix: ['VITE_', 'TAURI_'],
  build: {
    outDir: 'dist',
    emptyOutDir: true,
    // Tauri uses Chromium on Windows and WebKit on macOS/Linux
    target: process.env.TAURI_ENV_PLATFORM === 'windows'
      ? 'chrome105'
      : 'safari14',
    // Don't minify for faster builds during development
    minify: process.env.TAURI_ENV_DEBUG ? false : 'esbuild',
    // Produce sourcemaps for debug builds
    sourcemap: !!process.env.TAURI_ENV_DEBUG,
  },
  // Prevent vite from obscuring rust errors
  clearScreen: false,
})
