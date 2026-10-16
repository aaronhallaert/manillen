//! Par analysis: the outcome of a deal when both players see every card and play perfectly.
//! This uses hidden information and is meant for study (before the deal, if the player
//! chooses to peek, or in the review), never for the engine's own play.

use super::solver::{Budget, Horizon, Solver};
use crate::card::Player;
use crate::deal::{Layout, Position};
use crate::rules::{signed_score_for_points, Bid};
use serde::{Deserialize, Serialize};

/// Perfect-play outcome of one bid.
#[derive(Clone, PartialEq, Eq, Debug, Serialize, Deserialize)]
pub struct ParBid {
    pub bid: Bid,
    /// Card points per seat with perfect play, non-dealer leading.
    pub points: [u8; 2],
    /// Whether the non-dealer should contra (they win the deal with perfect play).
    pub contra: bool,
    /// Signed deal score for the dealer, with the non-dealer's optimal contra applied.
    pub dealer_score: i32,
}

#[derive(Clone, PartialEq, Eq, Debug, Serialize, Deserialize)]
pub struct ParReport {
    pub dealer: Player,
    pub bids: Vec<ParBid>,
    /// The dealer's best bid under perfect play.
    pub best_bid: Bid,
    /// Signed par score for the dealer (the value of the deal).
    pub par_score: i32,
    /// Perfect-play outcome of the bid actually made, if the deal has been bid.
    pub actual: Option<ParBid>,
    pub nodes: u64,
}

/// Solve every bid for `layout` with perfect information. Returns `None` if the budget
/// (total nodes) runs out before all bids are solved.
pub fn par_analysis(solver: &mut Solver, layout: &Layout, dealer: Player, actual_bid: Option<Bid>, actual_contra: Option<bool>, budget: Budget) -> Option<ParReport> {
    let start = solver.nodes;
    let mut bids = Vec::with_capacity(5);
    for bid in Bid::ALL {
        let used = solver.nodes - start;
        if used >= budget.max_nodes {
            return None;
        }
        let pos = Position::new(layout.clone(), bid, dealer.other());
        solver.new_world();
        // Binary search with zero-window probes (much cheaper than a full-window solve);
        // the probes share the transposition table of this world.
        let mut lo = 0i32; // value >= lo is known
        let mut hi = 60i32; // value <= hi is known
        while lo < hi {
            let mid = (lo + hi + 1) / 2;
            let used = solver.nodes - start;
            if used >= budget.max_nodes {
                return None;
            }
            if solver.at_least(&pos, mid, Budget::nodes(budget.max_nodes - used), Horizon::FULL)? {
                lo = mid;
            } else {
                hi = mid - 1;
            }
        }
        let nondealer = lo as u8;
        let mut points = [0u8; 2];
        points[dealer.other().index()] = nondealer;
        points[dealer.index()] = 60 - nondealer;
        let contra = nondealer > 30;
        let mult = bid.multiplier() as i32 * if contra { 2 } else { 1 };
        let dealer_score = signed_score_for_points(points[dealer.index()] as i32, mult);
        bids.push(ParBid { bid, points, contra, dealer_score });
    }
    let best = bids.iter().max_by_key(|b| b.dealer_score).expect("five bids");
    let actual = actual_bid.and_then(|ab| {
        let b = bids.iter().find(|b| b.bid == ab)?;
        let contra = actual_contra.unwrap_or(b.contra);
        let mult = ab.multiplier() as i32 * if contra { 2 } else { 1 };
        Some(ParBid { bid: ab, points: b.points, contra, dealer_score: signed_score_for_points(b.points[dealer.index()] as i32, mult) })
    });
    Some(ParReport { dealer, bids: bids.clone(), best_bid: best.bid, par_score: best.dealer_score, actual, nodes: solver.nodes - start })
}

#[cfg(test)]
mod tests {
    use super::*;
    use rand::rngs::SmallRng;
    use rand::SeedableRng;

    #[test]
    fn par_is_consistent() {
        let mut rng = SmallRng::seed_from_u64(77);
        let layout = Layout::deal(&mut rng);
        let mut solver = Solver::new(20);
        let r = par_analysis(&mut solver, &layout, Player::P1, Some(Bid::NoTrump), Some(false), Budget::UNLIMITED).unwrap();
        assert_eq!(r.bids.len(), 5);
        for b in &r.bids {
            assert_eq!(b.points[0] as u32 + b.points[1] as u32, 60);
            assert!(b.dealer_score <= r.par_score);
        }
        let a = r.actual.unwrap();
        assert_eq!(a.bid, Bid::NoTrump);
        assert!(!a.contra);
        assert_eq!(a.dealer_score, (a.points[1] as i32 - 30) * 2);
    }
}
