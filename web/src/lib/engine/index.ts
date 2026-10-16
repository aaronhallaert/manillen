import type { Engine } from './client';
import { MockEngine } from './mock';
import { WasmEngine } from './wasm';

export type { Engine, Seed } from './client';
export { MockEngine } from './mock';
export { WasmEngine } from './wasm';

/**
 * Picks the real wasm engine when the pkg is built, otherwise falls back to the fixture mock.
 * Force the mock with `VITE_ENGINE=mock`.
 */
export async function createEngine(): Promise<Engine> {
  if (import.meta.env.VITE_ENGINE === 'mock') return new MockEngine();
  try {
    return await WasmEngine.create();
  } catch (e) {
    console.warn('[manillen] wasm engine unavailable, using mock:', e instanceof Error ? e.message : e);
    return new MockEngine();
  }
}
