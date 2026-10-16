//! A match: a sequence of deals with alternating dealer, played to a target score.

use crate::card::Player;
use crate::deal::{Deal, DealError, Event, Phase};
use crate::record::GameRecord;
use crate::rules::{DealResult, Move};
use rand::rngs::SmallRng;
use rand::SeedableRng;
use serde::{Deserialize, Serialize};

/// Public summary of the match (see docs/api.md `MatchState`).
#[derive(Clone, PartialEq, Eq, Debug, Serialize, Deserialize)]
pub struct MatchState {
    pub target: u16,
    pub scores: [u16; 2],
    pub dealer: Player,
    pub deal_index: u16,
    pub phase: Phase,
    pub finished: bool,
    pub winner: Option<Player>,
    pub deals: Vec<DealResult>,
}

/// Everything needed to rebuild a `Match` later (see docs/api.md `MatchSnapshot`).
#[derive(Clone, PartialEq, Eq, Debug, Serialize, Deserialize)]
pub struct MatchSnapshot {
    pub seed: [u32; 2],
    pub target: u16,
    pub first_dealer: Player,
    pub scores: [u16; 2],
    pub deals: Vec<DealResult>,
    pub record: GameRecord,
}

pub struct Match {
    seed: u64,
    target: u16,
    first_dealer: Player,
    scores: [u16; 2],
    deals: Vec<DealResult>,
    current: Deal,
}

fn deal_rng(seed: u64, deal_index: usize) -> SmallRng {
    SmallRng::seed_from_u64(seed ^ (deal_index as u64 + 1).wrapping_mul(0x9E37_79B9_7F4A_7C15))
}

impl Match {
    pub fn new(seed: u64, target: u16, first_dealer: Player) -> Match {
        let current = Deal::random(&mut deal_rng(seed, 0), first_dealer);
        Match { seed, target, first_dealer, scores: [0, 0], deals: Vec::new(), current }
    }

    pub fn snapshot(&self) -> MatchSnapshot {
        MatchSnapshot {
            seed: [(self.seed >> 32) as u32, self.seed as u32],
            target: self.target,
            first_dealer: self.first_dealer,
            scores: self.scores,
            deals: self.deals.clone(),
            record: GameRecord::of(&self.current),
        }
    }

    pub fn restore(snap: &MatchSnapshot) -> Result<Match, DealError> {
        let current = snap.record.replay_all()?;
        let seed = ((snap.seed[0] as u64) << 32) | snap.seed[1] as u64;
        Ok(Match { seed, target: snap.target, first_dealer: snap.first_dealer, scores: snap.scores, deals: snap.deals.clone(), current })
    }

    #[inline]
    pub fn deal(&self) -> &Deal {
        &self.current
    }

    #[inline]
    pub fn scores(&self) -> [u16; 2] {
        self.scores
    }

    #[inline]
    pub fn target(&self) -> u16 {
        self.target
    }

    pub fn winner(&self) -> Option<Player> {
        if self.scores[0] >= self.target && self.scores[0] > self.scores[1] {
            Some(Player::P0)
        } else if self.scores[1] >= self.target && self.scores[1] > self.scores[0] {
            Some(Player::P1)
        } else {
            None
        }
    }

    pub fn finished(&self) -> bool {
        self.winner().is_some()
    }

    pub fn state(&self) -> MatchState {
        MatchState {
            target: self.target,
            scores: self.scores,
            dealer: self.current.dealer(),
            deal_index: self.deals.len() as u16,
            phase: self.current.phase(),
            finished: self.finished(),
            winner: self.winner(),
            deals: self.deals.clone(),
        }
    }

    /// Apply a move to the current deal. When the deal ends, the match score is updated
    /// and a `MatchOver` event is appended if the target is reached.
    pub fn apply(&mut self, mv: Move) -> Result<Vec<Event>, DealError> {
        if self.finished() {
            return Err(DealError::WrongPhase);
        }
        let mut events = self.current.apply(mv)?;
        if let Some(result) = self.current.result() {
            if let Some(w) = result.winner {
                self.scores[w.index()] += result.score;
            }
            self.deals.push(result);
            if let Some(w) = self.winner() {
                events.push(Event::MatchOver { winner: w });
            }
        }
        Ok(events)
    }

    /// Start the next deal (dealer alternates). Only valid when the current deal is done.
    pub fn next_deal(&mut self) -> Result<(), DealError> {
        if !self.current.is_over() || self.finished() {
            return Err(DealError::WrongPhase);
        }
        let dealer = self.current.dealer().other();
        self.current = Deal::random(&mut deal_rng(self.seed, self.deals.len()), dealer);
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::rules::Bid;
    use rand::Rng;

    #[test]
    fn match_runs_to_target() {
        let mut m = Match::new(42, 20, Player::P0);
        let mut rng = SmallRng::seed_from_u64(9);
        let mut guard = 0;
        while !m.finished() {
            let legal = m.deal().legal_moves();
            let mv = if m.deal().phase() == Phase::Bidding { Move::Bid { bid: Bid::NoTrump } } else { legal[rng.gen_range(0..legal.len())] };
            m.apply(mv).unwrap();
            if m.deal().is_over() && !m.finished() {
                let prev = m.deal().dealer();
                m.next_deal().unwrap();
                assert_eq!(m.deal().dealer(), prev.other());
            }
            guard += 1;
            assert!(guard < 100_000);
        }
        let s = m.state();
        assert!(s.finished);
        assert!(s.scores[s.winner.unwrap().index()] >= 20);
        assert_eq!(s.deals.len() as u16, s.deal_index);
    }

    #[test]
    fn snapshot_restores_and_continues_identically() {
        let mut m = Match::new(0xDEAD_BEEF_1234, 101, Player::P1);
        m.apply(Move::Bid { bid: Bid::Trump { suit: crate::card::Suit::Clubs } }).unwrap();
        m.apply(Move::Contra { contra: false }).unwrap();
        let mut rng = SmallRng::seed_from_u64(1);
        for _ in 0..7 {
            let legal = m.deal().legal_moves();
            m.apply(legal[rng.gen_range(0..legal.len())]).unwrap();
        }
        let snap = m.snapshot();
        let json = serde_json::to_string(&snap).unwrap();
        let back: MatchSnapshot = serde_json::from_str(&json).unwrap();
        let mut r = Match::restore(&back).unwrap();
        assert_eq!(r.deal(), m.deal());
        assert_eq!(r.state(), m.state());
        // finishing the deal and dealing the next one gives the same layout in both
        while !m.deal().is_over() {
            let legal = m.deal().legal_moves();
            let mv = legal[rng.gen_range(0..legal.len())];
            m.apply(mv).unwrap();
            r.apply(mv).unwrap();
        }
        m.next_deal().unwrap();
        r.next_deal().unwrap();
        assert_eq!(r.deal().layout(), m.deal().layout());
        assert_eq!(r.deal().dealer(), Player::P0);
    }
}
