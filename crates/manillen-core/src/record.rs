//! Replayable record of a deal.

use crate::card::Player;
use crate::deal::{Deal, DealError, Layout};
use crate::rules::Move;
use crate::view::PlayerView;
use serde::{Deserialize, Serialize};

#[derive(Clone, PartialEq, Eq, Debug, Serialize, Deserialize)]
pub struct GameRecord {
    pub version: u8,
    pub dealer: Player,
    pub layout: Layout,
    pub moves: Vec<Move>,
}

impl GameRecord {
    pub const VERSION: u8 = 1;

    pub fn of(deal: &Deal) -> GameRecord {
        GameRecord { version: Self::VERSION, dealer: deal.dealer(), layout: deal.layout().clone(), moves: deal.moves().to_vec() }
    }

    /// Replay the first `n` moves (all if `n >= moves.len()`).
    pub fn replay(&self, n: usize) -> Result<Deal, DealError> {
        let mut deal = Deal::new(self.layout.clone(), self.dealer)?;
        for mv in self.moves.iter().take(n) {
            deal.apply(*mv)?;
        }
        Ok(deal)
    }

    pub fn replay_all(&self) -> Result<Deal, DealError> {
        self.replay(self.moves.len())
    }

    /// The view of `player` just before `moves[index]` is applied.
    pub fn view_at(&self, index: usize, player: Player) -> Result<PlayerView, DealError> {
        Ok(PlayerView::of(&self.replay(index)?, player))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rand::rngs::SmallRng;
    use rand::{Rng, SeedableRng};

    #[test]
    fn record_round_trip_and_replay() {
        let mut rng = SmallRng::seed_from_u64(3);
        let mut deal = Deal::random(&mut rng, Player::P1);
        deal.apply(Move::Bid { bid: crate::rules::Bid::NoTrump }).unwrap();
        deal.apply(Move::Contra { contra: true }).unwrap();
        while !deal.is_over() {
            let legal = deal.legal_moves();
            deal.apply(legal[rng.gen_range(0..legal.len())]).unwrap();
        }
        let rec = GameRecord::of(&deal);
        let json = serde_json::to_string(&rec).unwrap();
        let back: GameRecord = serde_json::from_str(&json).unwrap();
        assert_eq!(back, rec);
        let replayed = back.replay_all().unwrap();
        assert_eq!(&replayed, &deal);
        assert_eq!(rec.moves.len(), 34);
    }

    #[test]
    fn fixture_record_parses() {
        let json = include_str!("../../../docs/fixtures/record.json");
        let rec: GameRecord = serde_json::from_str(json).unwrap();
        let deal = rec.replay_all().unwrap();
        assert_eq!(deal.tricks().len(), 2);
        let v = rec.view_at(6, Player::P0).unwrap();
        let expected: PlayerView = serde_json::from_str(include_str!("../../../docs/fixtures/view_play.json")).unwrap();
        assert_eq!(v.hand, expected.hand);
        assert_eq!(v.table, expected.table);
        assert_eq!(v.unknown, expected.unknown);
        assert_eq!(v.played, expected.played);
        assert_eq!(v.points, expected.points);
        assert_eq!(v.last_trick, expected.last_trick);
        assert_eq!(v.legal_moves.len(), expected.legal_moves.len());
    }
}
