//! Bidding and contra decisions by sampling.

use super::pimc::{sample_points, Eval, EvalOption, Unit};
use super::solver::{Budget, Horizon, Solver};
use crate::rules::{signed_score_for_points, Bid, Move};
use crate::view::PlayerView;
use rand::Rng;

/// Expected signed deal score for the dealer for each possible bid.
/// The opponent is assumed to contra whenever they would collect 33+ points.
pub fn evaluate_bids<R: Rng + ?Sized>(view: &PlayerView, samples: u32, horizon: Horizon, budget: Budget, rng: &mut R, solver: &mut Solver) -> Option<Eval> {
    let start_nodes = solver.nodes;
    let per_bid = Budget::nodes(budget.max_nodes / 5);
    let mut options = Vec::with_capacity(5);
    let mut done = 0;
    for bid in Bid::ALL {
        let pts = sample_points(view, bid, samples, horizon, per_bid, rng, solver);
        if pts.is_empty() {
            return None;
        }
        done = done.max(pts.len() as u32);
        let mean = pts
            .iter()
            .map(|&p| {
                let opp_contra = 60 - p >= 33;
                let mult = bid.multiplier() as i32 * if opp_contra { 2 } else { 1 };
                signed_score_for_points(p, mult) as f64
            })
            .sum::<f64>()
            / pts.len() as f64;
        options.push(EvalOption { mv: Move::Bid { bid }, value: mean as f32, trick_lost: None });
    }
    let mut eval = Eval { player: view.me, unit: Unit::Score, options, samples: done, nodes: solver.nodes - start_nodes };
    eval.sort();
    Some(eval)
}

/// Expected signed deal score for the non-dealer with and without contra.
pub fn evaluate_contra<R: Rng + ?Sized>(view: &PlayerView, samples: u32, horizon: Horizon, budget: Budget, rng: &mut R, solver: &mut Solver) -> Option<Eval> {
    let bid = view.bid?;
    let start_nodes = solver.nodes;
    let pts = sample_points(view, bid, samples, horizon, budget, rng, solver);
    if pts.is_empty() {
        return None;
    }
    let base = pts.iter().map(|&p| signed_score_for_points(p, bid.multiplier() as i32) as f64).sum::<f64>() / pts.len() as f64;
    let options = vec![
        EvalOption { mv: Move::Contra { contra: false }, value: base as f32, trick_lost: None },
        EvalOption { mv: Move::Contra { contra: true }, value: (2.0 * base) as f32, trick_lost: None },
    ];
    let mut eval = Eval { player: view.me, unit: Unit::Score, options, samples: pts.len() as u32, nodes: solver.nodes - start_nodes };
    eval.sort();
    Some(eval)
}
