# Manillen engine — JSON API contract

Shared by `manillen-core` (serde types), `manillen-wasm` (`Session`), the CLI and the Svelte GUI.
Every method of the wasm `Session` takes and returns **JSON strings**. The TypeScript mirror lives in
`web/src/lib/engine/types.ts`. Fixtures in `docs/fixtures/` are valid instances of these shapes.

## Primitive encodings

| Type | JSON | Notes |
|------|------|-------|
| `Card` | `"7C"`, `"JD"`, `"AH"`, `"10S"` | rank then suit. Ranks: `7 8 9 J Q K A 10`. Suits: `C D H S`. |
| `Suit` | `"C" \| "D" \| "H" \| "S"` | clubs, diamonds, hearts, spades |
| `Player` | `0 \| 1` | seat index. Player 0 is the human by default (see `Settings.human`). |
| `Phase` | `"bidding" \| "contra" \| "play" \| "done"` | |
| `Level` | `"easy" \| "medium" \| "hard"` | |
| `Label` | `"best" \| "good" \| "inaccuracy" \| "mistake" \| "blunder"` | |

Rank order low → high: `7 8 9 J Q K A 10`. Points: `10=5 A=4 K=3 Q=2 J=1`, others 0.

## Bid, Move

```jsonc
// Bid
{ "type": "trump", "suit": "H" }
{ "type": "no_trump" }

// Move (tagged union)
{ "type": "bid",    "bid": { "type": "trump", "suit": "H" } }
{ "type": "contra", "contra": true }
{ "type": "play",   "card": "10H" }
```

## TableSlot (as seen by any viewer)

```jsonc
{ "top": "7C" | null,   // face-up card currently on the slot, null if the slot is empty
  "covered": true }     // true while a face-down card still lies beneath `top`
```
A slot starts as `{top: X, covered: true}`. Playing `X` flips the hidden card: `{top: Y, covered: false}`.
Playing `Y` empties it: `{top: null, covered: false}`.

## Trick

```jsonc
// completed trick
{ "leader": 0, "cards": ["7C", "AC"], "winner": 1, "points": 4 }
// trick in progress (CurrentTrick)
{ "leader": 0, "cards": ["7C"] }
```
`cards[0]` is the leader's card, `cards[1]` the follower's.

## PlayerView — everything player `me` is allowed to know

```jsonc
{
  "me": 0,
  "dealer": 1,
  "phase": "play",
  "to_move": 0,                 // null when phase == "done"
  "bid": { "type": "trump", "suit": "H" },   // null during bidding
  "contra": false,
  "multiplier": 1,              // 1, 2 or 4
  "hand": ["7C", "KD", "10H"],  // own hand, sorted
  "opp_hand_count": 3,
  "table": [ [TableSlot, TableSlot, TableSlot, TableSlot],   // player 0's row
             [TableSlot, TableSlot, TableSlot, TableSlot] ], // player 1's row
  "current_trick": { "leader": 1, "cards": ["9S"] } | null,
  "last_trick": Trick | null,   // the ONLY trick the UI may show
  "tricks_played": 5,           // 0..16
  "points": [12, 9],            // card points won so far, by player
  "played": ["AS", "9H"],       // all cards played so far (perfect-memory info). UI: do not display.
  "unknown": ["7D", "QD"],      // cards whose location `me` cannot know (opp hand + face-down cards)
  "legal_moves": [Move]         // non-empty only when to_move == me
}
```

## DealResult

```jsonc
{ "dealer": 1, "bid": Bid, "contra": false, "multiplier": 2,
  "points": [38, 22], "winner": 0,   // winner null on 30-30
  "base": 8, "score": 16 }           // base = |points - 30| of winner; score = base * multiplier
```

## MatchState

```jsonc
{ "target": 101, "scores": [16, 0], "dealer": 0,  // dealer of the current/next deal
  "deal_index": 1,                                 // 0-based index of the current deal
  "phase": "play",                                 // phase of the current deal
  "finished": false, "winner": null,
  "deals": [DealResult] }                          // completed deals
```

## GameRecord — one deal, replayable

```jsonc
{ "version": 1,
  "dealer": 1,
  "layout": {
    "hands": [ [8 Cards], [8 Cards] ],
    "table": [ [ {"top": "7C", "under": "AS"}, ... 4 ], [ ... 4 ] ]   // `under` = the face-down card
  },
  "moves": [Move] }                                                   // in play order
```
`moves[0]` is always the dealer's bid, `moves[1]` the non-dealer's contra decision, then 32 plays.

## Evaluation and AI responses

```jsonc
// EvalOption
{ "move": Move, "value": 34.5,       // expected card points for the player to move (0..60);
                                     // for bids/contra: expected deal score (signed, with multiplier)
  "trick_lost": 3 }                  // leads only: sampled layouts in which the opponent wins the trick
                                     // (the reply is forced by the must-beat/must-trump rules); absent otherwise

// EvalResponse
{ "player": 0, "unit": "points" | "score", "options": [EvalOption], "samples": 20, "nodes": 1532000 }

// AiMoveResponse
{ "move": Move, "eval": EvalResponse | null }   // eval null at easy level
```

## AnalysisReport

```jsonc
{ "player": 0,
  "decisions": [
    { "index": 2,                 // index into GameRecord.moves
      "trick": 0,                 // trick number for plays, -1 for bid/contra
      "phase": "play",
      "chosen": Move, "best": Move,
      "options": [EvalOption],    // every legal alternative
      "value_chosen": 31.2, "value_best": 33.0,
      "loss": 1.8, "label": "good", "unit": "points",
      "note": "7D was better than 9S (about 1.8 points lost)." | null,
      "reasons_best":   ["No higher diamonds can be in the opponent's hand or face down: …", "…"],
      "reasons_chosen": ["Higher spades still unseen: 10♠ K♠ …", "…"],   // empty when chosen == best
      "samples": 20            // sampled layouts of the unseen cards the values are averaged over
    }
  ],
  "timeline": [ { "index": 2, "value": 33.0 } ],   // expected points after each decision (eval graph)
  "summary": { "best": 10, "good": 4, "inaccuracy": 2, "mistake": 1, "blunder": 0, "avg_loss": 0.9 } }
```
Label thresholds (card points): best ≤ 0.5, good < 2, inaccuracy < 4, mistake < 7, blunder ≥ 7.
Bid/contra decisions use deal score with thresholds ×1 (same numbers).
`timeline` only contains play decisions (unit = points, 0..60); bid/contra decisions are not on the graph.
`AnalysisReport.decisions[].options` are sorted best first.
`reasons_*` are plain sentences derived **only from the player's view** (own hand, open cards,
played cards, unseen cards); they never use hidden information. The review UI should show the
player's counting context from `view_at_json`: `played` (cards gone) and `unknown` (cards unseen).

## ParReport — perfect-information par (hindsight, for study)

```jsonc
{ "dealer": 1,
  "bids": [ { "bid": Bid, "points": [28, 32], "contra": true, "dealer_score": -4 }, ... 5 ],
  "best_bid": Bid, "par_score": 6,          // signed, for the dealer, with the non-dealer's optimal contra
  "actual": ParBid | null,                  // the bid/contra actually made, solved perfectly
  "nodes": 31000000 }
```
`points` are the exact card points each seat ends with when **both players see all cards and play
perfectly** (non-dealer leads). This uses hidden information: show it only when the player asks
(e.g. a "show par" toggle) or after the deal.

## Settings

```jsonc
{ "level": "hard", "target": 101, "human": 0, "max_nodes": 1500000, "first_dealer": null }
```
All fields are optional (defaults: medium, 101, 0, 1500000, dealer = the opponent of `human`).

## MatchSnapshot / SessionSnapshot — resume after a reload

```jsonc
// SessionSnapshot (from `snapshot_json()`, accepted by `Session.restore`)
{ "settings": Settings,
  "match": { "seed": [hi, lo], "target": 101, "first_dealer": 1, "scores": [16, 0],
             "deals": [DealResult], "record": GameRecord } }   // record = current deal so far
```
Deal layouts are derived deterministically from `seed` and the deal index, so a restored
session continues with exactly the same future deals.

## Events (returned by `apply`)

```jsonc
{ "type": "bid",      "player": 1, "bid": Bid }
{ "type": "contra",   "player": 0, "contra": false }
{ "type": "played",   "player": 0, "card": "7C", "from_table": true }
{ "type": "flipped",  "player": 0, "slot": 2, "card": "AS" }
{ "type": "trick",    "trick": Trick }
{ "type": "deal_over","result": DealResult }
{ "type": "match_over","winner": 0 }
```

## `Session` (wasm-bindgen) — method list

| Method | Args | Returns |
|--------|------|---------|
| `new Session(seed_hi: u32, seed_lo: u32, settings: string)` | Settings JSON (may be `""`) | starts a match and deals deal 0 |
| `Session.restore(snapshot: string)` (static) | SessionSnapshot JSON | rebuilds a session, same future deals |
| `snapshot_json()` | | SessionSnapshot |
| `settings_json()` | | Settings |
| `match_json()` | | MatchState |
| `view_json(player: number)` | | PlayerView |
| `legal_moves_json()` | | `[Move]` for the player to move |
| `apply_json(move: string)` | Move JSON | `{ "events": [Event], "view": PlayerView (for Settings.human) }` — throws on illegal move |
| `next_deal()` | | MatchState (deals the next deal; only valid when phase == "done") |
| `ai_move_json(level: string, max_nodes: number)` | `max_nodes` 0 = settings default; unknown level = settings level | AiMoveResponse for the player to move (engine sees only that player's view). Does **not** apply it. |
| `evaluate_json(player: number, max_nodes: number)` | | EvalResponse from `player`'s info set (hard preset, perfect memory) |
| `record_json()` | | GameRecord of the current deal (moves so far) |
| `analyze_json(record: string, player: number, max_nodes: number)` | GameRecord JSON | AnalysisReport |
| `par_json(max_nodes: number)` | 0 = 60M nodes | ParReport for the current deal (hindsight) |
| `par_of_record_json(record: string, max_nodes: number)` | | ParReport for a record's layout |
| `view_at_json(record: string, move_index: number, player: number)` | | PlayerView just **before** `record.moves[move_index]` is applied (for the review screen) |
| `version()` | | engine version string |

Errors are thrown as JS `Error` with a message string.

## CLI equivalents

`manillen-cli play|selfplay|analyze|bench` use the same JSON types for records and reports.
