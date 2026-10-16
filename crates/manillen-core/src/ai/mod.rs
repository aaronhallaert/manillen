//! AI: solver, determinization, PIMC, heuristics, bidding and analysis.

pub mod analysis;
pub mod bidding;
pub mod determinize;
pub mod heuristic;
pub mod par;
pub mod pimc;
pub mod solver;

pub use analysis::{analyze, AnalysisReport, Decision, Label, Summary};
pub use par::{par_analysis, ParBid, ParReport};
pub use pimc::{Eval, EvalOption, Unit};
pub use solver::{Budget, Horizon, Solver};

use crate::deal::Phase;
use crate::rules::Move;
use crate::view::{MemoryModel, PlayerView};
use rand::rngs::SmallRng;
use rand::{Rng, SeedableRng};
use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, PartialEq, Eq, Debug, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Level {
    Easy,
    Medium,
    Hard,
}

impl Level {
    pub fn parse(s: &str) -> Option<Level> {
        match s.to_ascii_lowercase().as_str() {
            "easy" => Some(Level::Easy),
            "medium" => Some(Level::Medium),
            "hard" => Some(Level::Hard),
            _ => None,
        }
    }
}

/// Tunable parameters of a level.
#[derive(Clone, Copy, Debug)]
pub struct LevelConfig {
    pub memory: MemoryModel,
    /// `None` = no search, heuristics only.
    pub search: Option<SearchConfig>,
    /// Probability of a random legal move (easy only).
    pub randomness: f32,
}

#[derive(Clone, Copy, Debug)]
pub struct SearchConfig {
    pub play_samples: u32,
    pub play_horizon: Horizon,
    pub bid_samples: u32,
    pub bid_horizon: Horizon,
}

impl LevelConfig {
    pub fn for_level(level: Level) -> LevelConfig {
        match level {
            Level::Easy => LevelConfig { memory: MemoryModel::LastTrickOnly, search: None, randomness: 0.2 },
            Level::Medium => LevelConfig {
                memory: MemoryModel::Partial,
                search: Some(SearchConfig {
                    play_samples: 4,
                    play_horizon: Horizon { exact_when_remaining: 7, depth: 3 },
                    bid_samples: 3,
                    bid_horizon: Horizon { exact_when_remaining: 5, depth: 3 },
                }),
                randomness: 0.0,
            },
            Level::Hard => LevelConfig {
                memory: MemoryModel::Perfect,
                search: Some(SearchConfig {
                    play_samples: 24,
                    play_horizon: Horizon { exact_when_remaining: 12, depth: 5 },
                    bid_samples: 12,
                    bid_horizon: Horizon { exact_when_remaining: 10, depth: 5 },
                }),
                randomness: 0.0,
            },
        }
    }
}

/// Cheapest search used when the budget cannot afford a level's preset.
const SHALLOW: SearchConfig = SearchConfig {
    play_samples: 4,
    play_horizon: Horizon { exact_when_remaining: 6, depth: 2 },
    bid_samples: 3,
    bid_horizon: Horizon { exact_when_remaining: 5, depth: 2 },
};

/// A move chosen by the engine, with the evaluation that produced it (if any).
#[derive(Clone, PartialEq, Debug, Serialize, Deserialize)]
pub struct AiMove {
    #[serde(rename = "move")]
    pub mv: Move,
    pub eval: Option<Eval>,
}

/// The engine: owns the transposition table and a seeded RNG.
pub struct Ai {
    pub solver: Solver,
    pub rng: SmallRng,
}

impl Ai {
    /// `tt_bits` = log2 of transposition table entries (20 → 16 MB).
    pub fn new(seed: u64, tt_bits: u32) -> Ai {
        Ai { solver: Solver::new(tt_bits), rng: SmallRng::seed_from_u64(seed) }
    }

    /// Choose a move for `view.me`, who must be the player to move. The engine sees only `view`.
    pub fn choose_move(&mut self, view: &PlayerView, level: Level, max_nodes: u64) -> Option<AiMove> {
        if view.to_move != Some(view.me) {
            return None;
        }
        let cfg = LevelConfig::for_level(level);
        let view = view.clone().with_memory(cfg.memory);
        if cfg.randomness > 0.0 && view.phase == Phase::Play && self.rng.gen::<f32>() < cfg.randomness {
            let legal = view.legal_cards().to_vec();
            if !legal.is_empty() {
                let card = legal[self.rng.gen_range(0..legal.len())];
                return Some(AiMove { mv: Move::Play { card }, eval: None });
            }
        }
        let eval = match cfg.search {
            Some(search) => self.evaluate_with(&view, search, max_nodes).or_else(|| self.evaluate_with(&view, SHALLOW, max_nodes)),
            None => None,
        };
        if let Some(eval) = eval {
            let mv = match view.phase {
                // contra only with a clear margin
                Phase::Contra => {
                    let no = eval.value_of(Move::Contra { contra: false }).unwrap_or(0.0);
                    let yes = eval.value_of(Move::Contra { contra: true }).unwrap_or(0.0);
                    Move::Contra { contra: yes > no + 1.0 }
                }
                _ => eval.best().map(|o| o.mv)?,
            };
            return Some(AiMove { mv, eval: Some(eval) });
        }
        let mv = self.heuristic_move(&view, cfg.randomness)?;
        Some(AiMove { mv, eval: None })
    }

    fn heuristic_move(&mut self, view: &PlayerView, randomness: f32) -> Option<Move> {
        match view.phase {
            Phase::Bidding => Some(Move::Bid { bid: heuristic::choose_bid(view) }),
            Phase::Contra => Some(Move::Contra { contra: heuristic::choose_contra(view) }),
            Phase::Play => heuristic::choose_play(view, randomness, &mut self.rng).map(|card| Move::Play { card }),
            Phase::Done => None,
        }
    }

    /// Evaluate all legal moves for `view.me` at the given level's search settings
    /// (perfect memory; `level` only selects the sample/horizon preset).
    pub fn evaluate(&mut self, view: &PlayerView, level: Level, max_nodes: u64) -> Option<Eval> {
        let cfg = LevelConfig::for_level(level);
        let search = cfg.search.unwrap_or(LevelConfig::for_level(Level::Hard).search.unwrap());
        if let Some(e) = self.evaluate_with(view, search, max_nodes) {
            return Some(e);
        }
        // Budget too small for the preset: fall back to a shallow search so there is always an answer.
        self.evaluate_with(view, SHALLOW, max_nodes)
    }

    fn evaluate_with(&mut self, view: &PlayerView, search: SearchConfig, max_nodes: u64) -> Option<Eval> {
        if view.to_move != Some(view.me) {
            return None;
        }
        let budget = Budget::nodes(max_nodes);
        match view.phase {
            Phase::Bidding => bidding::evaluate_bids(view, search.bid_samples, search.bid_horizon, budget, &mut self.rng, &mut self.solver),
            Phase::Contra => bidding::evaluate_contra(view, search.bid_samples, search.bid_horizon, budget, &mut self.rng, &mut self.solver),
            Phase::Play => pimc::evaluate_plays(view, pimc::PimcParams { samples: search.play_samples, horizon: search.play_horizon }, budget, &mut self.rng, &mut self.solver),
            Phase::Done => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::card::Player;
    use crate::deal::Deal;
    use crate::record::GameRecord;

    fn play_deal(ai: &mut Ai, levels: [Level; 2], seed: u64, nodes: u64) -> Deal {
        let mut rng = SmallRng::seed_from_u64(seed);
        let mut deal = Deal::random(&mut rng, Player::P0);
        while !deal.is_over() {
            let p = deal.to_move().unwrap();
            let view = PlayerView::of(&deal, p);
            let m = ai.choose_move(&view, levels[p.index()], nodes).expect("engine returns a move");
            assert!(deal.is_legal(m.mv), "illegal move {} by {p}", m.mv);
            deal.apply(m.mv).unwrap();
        }
        deal
    }

    #[test]
    fn every_level_plays_legal_complete_deals() {
        let mut ai = Ai::new(1, 16);
        for (i, levels) in [[Level::Easy, Level::Easy], [Level::Medium, Level::Easy], [Level::Hard, Level::Medium]].iter().enumerate() {
            let deal = play_deal(&mut ai, *levels, i as u64, 200_000);
            assert_eq!(deal.tricks().len(), 16);
        }
    }

    #[test]
    fn analysis_runs_and_labels() {
        let mut ai = Ai::new(2, 16);
        let deal = play_deal(&mut ai, [Level::Easy, Level::Easy], 11, 50_000);
        let record = GameRecord::of(&deal);
        let report = analyze(&mut ai, &record, Player::P0, 300_000, |_, _| {}).unwrap();
        assert_eq!(report.decisions.len(), 17); // bid + 16 plays for the dealer
        assert!(report.decisions.iter().all(|d| d.loss >= 0.0));
        let json = serde_json::to_string(&report).unwrap();
        assert!(json.contains("\"label\""));
    }
}
