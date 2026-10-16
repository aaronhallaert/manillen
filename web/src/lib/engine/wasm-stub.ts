// Stand-in for `manillen-wasm` while crates/manillen-wasm/pkg has not been built.
// vite.config.ts aliases `manillen-wasm` to this file when the pkg directory is missing.
export default async function init(): Promise<never> {
  throw new Error('manillen-wasm pkg not built (run: wasm-pack build crates/manillen-wasm --target web --release)');
}
export class Session {
  constructor(_hi: number, _lo: number, _settings: string) {
    throw new Error('manillen-wasm pkg not built');
  }
}
