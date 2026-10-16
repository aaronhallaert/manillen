/// <reference types="svelte" />
/// <reference types="vite/client" />
/// <reference types="vite-plugin-pwa/client" />

interface ImportMetaEnv {
  readonly VITE_ENGINE?: 'mock' | 'wasm';
}

declare module 'manillen-wasm';
