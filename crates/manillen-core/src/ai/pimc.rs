//! Perfect Information Monte Carlo: average exact solutions over sampled worlds.

use super::determinize::sample_world;
use super::solver::{Budget, Horizon, Solver};
use crate::rules::{Bid, Move};
use crate::view::PlayerView;
use rand::Rng;
use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug)]
pub struct PimcParams {
    pub samples: u32,
    pub horizon: Horizon,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Unit {
    /// Expected card points (0..60) for the player.
    Points,
    /// Expected signed deal score (with multiplier) for the player.
    Score,
}

#[derive(Clone, PartialEq, Debug, Serialize, Deserialize)]
pub struct EvalOption {
    #[serde(rename = "move")]
    pub mv: Move,
    pub value: f32,
    /// For a lead: in how many sampled layouts the opponent wins the trick. The follower's
    /// reply is forced by the rules (must beat / must trump), so this is exact per layout.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub trick_lost: Option<u32>,
}

/// Evaluation of every legal option from one player's information set.
#[derive(Clone, PartialEq, Debug, Serialize, Deserialize)]
pub struct Eval {
    pub player: crate::card::Player,
    pub unit: Unit,
    pub options: Vec<EvalOption>,
    pub samples: u32,
    pub nodes: u64,
}

impl Eval {
    pub fn best(&self) -> Option<&EvalOption> {
        self.options.iter().max_by(|a, b| a.value.partial_cmp(&b.value).unwrap_or(std::cmp::Ordering::Equal))
    }

    pub fn value_of(&self, mv: Move) -> Option<f32> {
        self.options.iter().find(|o| o.mv == mv).map(|o| o.value)
    }

    pub fn sort(&mut self) {
        self.options.sort_by(|a, b| b.value.partial_cmp(&a.value).unwrap_or(std::cmp::Ordering::Equal));
    }
}

/// Evaluate every legal card for `view.me` (who must be the player to move in the play phase).
/// Returns `None` when not a single sample fits in the budget.
pub fn evaluate_plays<R: Rng + ?Sized>(view: &PlayerView, params: PimcParams, budget: Budget, rng: &mut R, solver: &mut Solver) -> Option<Eval> {
    let bid = view.bid?;
    let legal = view.legal_cards();
    if legal.is_empty() {
        return None;
    }
    let cards = legal.to_vec();
    let mut sums = vec![0f64; cards.len()];
    let mut lost = vec![0u32; cards.len()];
    let leading = view.led().is_none();
    let start_nodes = solver.nodes;
    let mut done = 0u32;
    for _ in 0..params.samples.max(1) {
        let used = solver.nodes - start_nodes;
        if used >= budget.max_nodes {
            break;
        }
        let world = sample_world(view, bid, rng);
        solver.new_world();
        let values = match solver.evaluate_moves(&world, Budget::nodes(budget.max_nodes - used), params.horizon) {
            Some(v) => v,
            None => break,
        };
        for (card, v) in values {
            if let Some(i) = cards.iter().position(|&c| c == card) {
                sums[i] += v as f64;
            }
        }
        if leading {
            let opp = view.me.other();
            let replies_avail = world.available(opp);
            for (i, &card) in cards.iter().enumerate() {
                let replies = crate::rules::legal_plays(replies_avail, Some(card), bid);
                if replies.iter().any(|r| crate::rules::beats(r, card, bid)) {
                    lost[i] += 1;
                }
            }
        }
        done += 1;
    }
    if done == 0 {
        return None;
    }
    let options = cards
        .iter()
        .zip(sums.iter())
        .enumerate()
        .map(|(i, (&card, &s))| EvalOption {
            mv: Move::Play { card },
            value: (s / done as f64) as f32,
            trick_lost: if leading { Some(lost[i]) } else { None },
        })
        .collect();
    let mut eval = Eval { player: view.me, unit: Unit::Points, options, samples: done, nodes: solver.nodes - start_nodes };
    eval.sort();
    Some(eval)
}

/// Expected card points for `view.me` under a hypothetical `bid`, averaged over worlds.
/// Used for bidding and contra decisions. Returns per-sample points so callers can
/// apply multipliers and model the opponent's reaction.
pub fn sample_points<R: Rng + ?Sized>(view: &PlayerView, bid: Bid, samples: u32, horizon: Horizon, budget: Budget, rng: &mut R, solver: &mut Solver) -> Vec<i32> {
    let start_nodes = solver.nodes;
    let mut out = Vec::with_capacity(samples as usize);
    for _ in 0..samples.max(1) {
        let used = solver.nodes - start_nodes;
        if used >= budget.max_nodes {
            break;
        }
        let world = sample_world(view, bid, rng);
        solver.new_world();
        let v = match solver.value(&world, Budget::nodes(budget.max_nodes - used), horizon) {
            Some(v) => v,
            None => break,
        };
        // `v` is for the player to move in the world
        let r = world.remaining_points() as i32 + world.led.map(|c| c.points() as i32).unwrap_or(0);
        let mine = if world.to_move() == view.me { v } else { r - v };
        out.push(mine + view.points[view.me.index()] as i32);
    }
    out
}
