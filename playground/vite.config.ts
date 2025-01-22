import { defineConfig } from 'vite'
import { svelte } from '@sveltejs/vite-plugin-svelte'
import wasm from 'vite-plugin-wasm'

// https://vite.dev/config/
export default defineConfig({
  build: {
    target: 'es2022'
  },
  plugins: [wasm(), svelte()],
})
