import { svelte } from '@sveltejs/vite-plugin-svelte';
import { VitePWA } from 'vite-plugin-pwa';
import { defineConfig } from 'vitest/config';
import { existsSync } from 'node:fs';
import { fileURLToPath } from 'node:url';

// `manillen-wasm` resolves to the wasm-pack output when it has been built,
// and to a throwing stub otherwise (the app then falls back to the mock engine).
const pkgDir = fileURLToPath(new URL('../crates/manillen-wasm/pkg', import.meta.url));
const pkgEntry = `${pkgDir}/manillen_wasm.js`;
const wasmAlias = existsSync(pkgEntry) ? pkgEntry : fileURLToPath(new URL('./src/lib/engine/wasm-stub.ts', import.meta.url));

export default defineConfig({
  base: './',
  plugins: [
    svelte(),
    VitePWA({
      registerType: 'autoUpdate',
      includeAssets: ['icon.svg'],
      manifest: {
        name: 'Manillen',
        short_name: 'Manillen',
        description: 'Two-player Manillen against an engine, with post-game review.',
        start_url: './',
        scope: './',
        display: 'standalone',
        orientation: 'portrait',
        theme_color: '#1f5f3a',
        background_color: '#0f2a1b',
        icons: [
          { src: 'icon.svg', sizes: 'any', type: 'image/svg+xml', purpose: 'any' },
          { src: 'icon.svg', sizes: '512x512', type: 'image/svg+xml', purpose: 'maskable' },
        ],
      },
      workbox: { globPatterns: ['**/*.{js,css,html,svg,wasm,json}'] },
    }),
  ],
  resolve: {
    alias: { 'manillen-wasm': wasmAlias },
  },
  server: { fs: { allow: ['..'] } },
  worker: { format: 'es' },
  test: {
    environment: 'jsdom',
    include: ['src/**/*.test.ts'],
  },
});
