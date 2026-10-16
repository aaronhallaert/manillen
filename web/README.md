# Manillen web GUI

Svelte 5 + Vite + TypeScript PWA for the Manillen engine. Contract: `../docs/api.md`.

```sh
npm install
npm run dev                  # uses the wasm engine when ../crates/manillen-wasm/pkg exists, else the fixture mock
VITE_ENGINE=mock npm run dev # force the mock engine
npm test                     # vitest
npm run check                # svelte-check + tsc
npm run build                # static site in dist/ (base './')
npm run preview              # serve dist/ on the LAN (open from a phone)
```

Engine resolution: `vite.config.ts` aliases `manillen-wasm` to `../crates/manillen-wasm/pkg/manillen_wasm.js`
when that file exists (build it with `wasm-pack build crates/manillen-wasm --target web --release`), otherwise to
`src/lib/engine/wasm-stub.ts`, and `createEngine()` falls back to `MockEngine`. Restart `vite` after building the pkg.

Layout: `src/lib/engine` (types, Engine interface, mock, worker + wasm client), `src/lib/state/game.svelte.ts`
(runes store), `src/lib/components` (Card, Table, BidDialog, ContraDialog, DealResult, Review, Lobby).
