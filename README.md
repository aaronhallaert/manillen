# Manillen engine

A chess-engine-style engine for two-player **Manillen** (Belgian trick-taking card game):
play against the computer at three levels, and review every decision afterwards to see
where you blundered. Written in Rust, runs natively and as WebAssembly in a phone browser.

Rules as implemented: [docs/rules-en.md](docs/rules-en.md) (English, authoritative),
original Dutch notes in `rules.md`. Playing tips: `strategies.md`.

## Layout

| Path | What |
|------|------|
| `crates/manillen-core` | Rules, deal state machine, information sets, AI (solver, PIMC, heuristics, analysis). Pure Rust, no threads, no clocks. |
| `crates/manillen-wasm` | `wasm-bindgen` `Session` with a JSON API ([docs/api.md](docs/api.md)). |
| `crates/manillen-cli` | Terminal tool: play, self-play between levels, analyze a record, benchmark. |
| `web/` | Svelte 5 + Vite PWA (mobile-first) that runs the wasm engine in a Web Worker. |
| `docs/` | API contract, fixtures, rules. |

## How the engine works

- The engine only ever receives a `PlayerView`, the information set of the seat it plays:
  own hand, all face-up cards, the played cards, and constraints inferred from the opponent's
  forced plays (voids, "no higher card"). It never looks at hidden cards.
- **Hard** = Perfect Information Monte Carlo: sample the unknown cards (opponent's hand and
  both players' face-down cards) consistently with those constraints, solve each world exactly
  with an alpha-beta solver and transposition table, average the values per move. Exact to the
  end when ≤ 11 tricks remain, horizon search with a material estimate earlier. Perfect card
  counting.
- **Medium** = same search with few samples and a short horizon, and a partial memory: it
  remembers trumps, 10s, aces and the last trick, forgets other low cards.
- **Easy** = rule-based play (unlock own hidden cards, cash masters, give away nothing) with
  some randomness and no memory beyond the last trick.
- **Bidding / contra** are evaluated by sampling too (expected deal score with multipliers).
- **Review**: every decision of the human is re-evaluated from what they could know at the
  time (not hindsight) and labelled best / good / inaccuracy / mistake / blunder by expected
  point loss, with reasons derived only from the player's view (open cards, played cards,
  unseen cards, voids inferred from earlier play).
- **Par** (hindsight, opt-in): with all 32 cards known and both sides playing perfectly, the
  exact points per seat for each of the 5 bids, the correct contra, and so the value of the
  deal. `manillen-cli par record.json`, or "show par" in the web app.

## Build and run

```bash
cargo test --workspace --release            # rules, solver vs brute force, sampler, self-play
cargo run --release -p manillen-cli -- play --level hard
cargo run --release -p manillen-cli -- selfplay --a hard --b easy --games 40
cargo run --release -p manillen-cli -- analyze records/deal-1.json --player 0
cargo run --release -p manillen-cli -- bench
```

WebAssembly + web app:

```bash
rustup target add wasm32-unknown-unknown
cargo install --locked wasm-pack
wasm-pack build crates/manillen-wasm --target web --release   # -> crates/manillen-wasm/pkg
cd web && npm install && npm run dev                           # or: npm run build && npm run preview -- --host
```

Open the preview URL on the phone (same network); the app is installable as a PWA.
Without the wasm pkg the web app falls back to a mock engine (`VITE_ENGINE=mock`).

## Performance (native, release)

Exact solve of a random deal: ~6M nodes / 0.7 s from trick 1, 1M / 0.1 s from trick 3,
negligible from trick 7. ~10M nodes/s native, ~8M nodes/s in wasm. Hard uses a budget of
1.5M nodes per decision by default (`Settings.max_nodes`), i.e. well under a second on a phone.

## Level strength (self-play, 1.5M nodes per move, single deals, seats alternated)

| Match-up | Result |
|----------|--------|
| hard vs easy | 46 – 10 (4 ties) over 60 deals |
| hard vs medium | 39 – 20 (1 tie) over 60 deals |
| medium vs easy | 47 – 28 (5 ties) over 80 deals |
