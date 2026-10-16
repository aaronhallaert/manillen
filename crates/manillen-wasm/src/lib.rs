//! WebAssembly bindings for the Manillen engine. Every method takes and returns JSON
//! strings; the shapes are documented in docs/api.md.

use manillen_core::ai::{analyze, Ai, AiMove, Level};
use manillen_core::{Deal, GameRecord, Match, MatchSnapshot, Move, Player, PlayerView};
use serde::{Deserialize, Serialize};
use wasm_bindgen::prelude::*;

#[wasm_bindgen(start)]
pub fn start() {
    console_error_panic_hook::set_once();
}

#[derive(Clone, Debug, Serialize, Deserialize)]
struct Settings {
    #[serde(default = "default_level")]
    level: Level,
    #[serde(default = "default_target")]
    target: u16,
    #[serde(default)]
    human: Player,
    #[serde(default = "default_nodes")]
    max_nodes: u64,
    #[serde(default)]
    first_dealer: Option<Player>,
}

fn default_level() -> Level {
    Level::Medium
}
fn default_target() -> u16 {
    101
}
fn default_nodes() -> u64 {
    1_500_000
}

#[derive(Serialize)]
struct ApplyResponse {
    events: Vec<manillen_core::Event>,
    view: PlayerView,
}

#[derive(Serialize, Deserialize)]
struct SessionSnapshot {
    settings: Settings,
    #[serde(rename = "match")]
    match_: MatchSnapshot,
}

fn err(e: impl std::fmt::Display) -> JsValue {
    JsValue::from_str(&e.to_string())
}

fn to_json<T: Serialize>(v: &T) -> Result<String, JsValue> {
    serde_json::to_string(v).map_err(err)
}

/// One match against the engine. The engine never sees more than the view of the seat it plays.
#[wasm_bindgen]
pub struct Session {
    settings: Settings,
    game: Match,
    ai: Ai,
}

#[wasm_bindgen]
impl Session {
    /// Start a new match. `settings` is a Settings JSON object (all fields optional).
    #[wasm_bindgen(constructor)]
    pub fn new(seed_hi: u32, seed_lo: u32, settings: &str) -> Result<Session, JsValue> {
        let settings: Settings = if settings.trim().is_empty() { serde_json::from_str("{}") } else { serde_json::from_str(settings) }.map_err(err)?;
        let seed = ((seed_hi as u64) << 32) | seed_lo as u64;
        let first_dealer = settings.first_dealer.unwrap_or(settings.human.other());
        let game = Match::new(seed, settings.target, first_dealer);
        let ai = Ai::new(seed ^ 0xA11C_E55E, 21);
        Ok(Session { settings, game, ai })
    }

    /// Rebuild a session from `snapshot_json()` output.
    pub fn restore(snapshot: &str) -> Result<Session, JsValue> {
        let snap: SessionSnapshot = serde_json::from_str(snapshot).map_err(err)?;
        let game = Match::restore(&snap.match_).map_err(err)?;
        let seed = ((snap.match_.seed[0] as u64) << 32) | snap.match_.seed[1] as u64;
        let ai = Ai::new(seed ^ 0xA11C_E55E ^ snap.match_.deals.len() as u64, 21);
        Ok(Session { settings: snap.settings, game, ai })
    }

    pub fn version(&self) -> String {
        manillen_core::VERSION.to_string()
    }

    pub fn snapshot_json(&self) -> Result<String, JsValue> {
        to_json(&SessionSnapshot { settings: self.settings.clone(), match_: self.game.snapshot() })
    }

    pub fn settings_json(&self) -> Result<String, JsValue> {
        to_json(&self.settings)
    }

    pub fn match_json(&self) -> Result<String, JsValue> {
        to_json(&self.game.state())
    }

    pub fn view_json(&self, player: u8) -> Result<String, JsValue> {
        to_json(&PlayerView::of(self.game.deal(), Player(player & 1)))
    }

    pub fn legal_moves_json(&self) -> Result<String, JsValue> {
        to_json(&self.game.deal().legal_moves())
    }

    /// Apply a move for the player to move. Returns the events and the human's view.
    pub fn apply_json(&mut self, mv: &str) -> Result<String, JsValue> {
        let mv: Move = serde_json::from_str(mv).map_err(err)?;
        let events = self.game.apply(mv).map_err(err)?;
        to_json(&ApplyResponse { events, view: PlayerView::of(self.game.deal(), self.settings.human) })
    }

    /// Deal the next deal (only when the current one is done). Returns the MatchState.
    pub fn next_deal(&mut self) -> Result<String, JsValue> {
        self.game.next_deal().map_err(err)?;
        self.match_json()
    }

    /// Let the engine pick a move for the player to move, using only that player's view.
    /// The move is NOT applied. `level` = "easy" | "medium" | "hard"; `max_nodes` 0 = settings default.
    pub fn ai_move_json(&mut self, level: &str, max_nodes: f64) -> Result<String, JsValue> {
        let level = Level::parse(level).unwrap_or(self.settings.level);
        let deal: &Deal = self.game.deal();
        let p = deal.to_move().ok_or_else(|| err("deal is over"))?;
        let view = PlayerView::of(deal, p);
        let nodes = if max_nodes > 0.0 { max_nodes as u64 } else { self.settings.max_nodes };
        let m: AiMove = self.ai.choose_move(&view, level, nodes).ok_or_else(|| err("no move available"))?;
        to_json(&m)
    }

    /// Evaluate every legal move of `player` (who must be to move) from their information set.
    pub fn evaluate_json(&mut self, player: u8, max_nodes: f64) -> Result<String, JsValue> {
        let view = PlayerView::of(self.game.deal(), Player(player & 1));
        let nodes = if max_nodes > 0.0 { max_nodes as u64 } else { self.settings.max_nodes };
        let e = self.ai.evaluate(&view, Level::Hard, nodes).ok_or_else(|| err("not this player's turn or no legal moves"))?;
        to_json(&e)
    }

    pub fn record_json(&self) -> Result<String, JsValue> {
        to_json(&GameRecord::of(self.game.deal()))
    }

    /// Review every decision of `player` in `record` (a GameRecord JSON). `max_nodes` per decision.
    pub fn analyze_json(&mut self, record: &str, player: u8, max_nodes: f64) -> Result<String, JsValue> {
        let record: GameRecord = serde_json::from_str(record).map_err(err)?;
        let nodes = if max_nodes > 0.0 { max_nodes as u64 } else { self.settings.max_nodes };
        let report = analyze(&mut self.ai, &record, Player(player & 1), nodes, |_, _| {}).map_err(err)?;
        to_json(&report)
    }

    /// Perfect-information par of the current deal (all cards known, both sides perfect).
    /// Uses hidden information: for study only. `max_nodes` total budget (0 = 60M).
    pub fn par_json(&mut self, max_nodes: f64) -> Result<String, JsValue> {
        let deal = self.game.deal();
        let nodes = if max_nodes > 0.0 { max_nodes as u64 } else { 60_000_000 };
        let r = manillen_core::ai::par_analysis(&mut self.ai.solver, deal.layout(), deal.dealer(), deal.bid(), if deal.bid().is_some() && deal.phase() != manillen_core::Phase::Contra { Some(deal.contra()) } else { None }, manillen_core::ai::Budget::nodes(nodes))
            .ok_or_else(|| err("par analysis exceeded the node budget"))?;
        to_json(&r)
    }

    /// Par of a GameRecord's layout (same as `par_json` but for any record, e.g. in the review).
    pub fn par_of_record_json(&mut self, record: &str, max_nodes: f64) -> Result<String, JsValue> {
        let record: GameRecord = serde_json::from_str(record).map_err(err)?;
        let nodes = if max_nodes > 0.0 { max_nodes as u64 } else { 60_000_000 };
        let bid = record.moves.first().and_then(|m| match m { Move::Bid { bid } => Some(*bid), _ => None });
        let contra = record.moves.get(1).and_then(|m| match m { Move::Contra { contra } => Some(*contra), _ => None });
        let r = manillen_core::ai::par_analysis(&mut self.ai.solver, &record.layout, record.dealer, bid, contra, manillen_core::ai::Budget::nodes(nodes))
            .ok_or_else(|| err("par analysis exceeded the node budget"))?;
        to_json(&r)
    }

    /// The view of `player` just before `record.moves[move_index]`.
    pub fn view_at_json(&self, record: &str, move_index: u32, player: u8) -> Result<String, JsValue> {
        let record: GameRecord = serde_json::from_str(record).map_err(err)?;
        let v = record.view_at(move_index as usize, Player(player & 1)).map_err(err)?;
        to_json(&v)
    }
}
