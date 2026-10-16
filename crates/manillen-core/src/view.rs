//! What a single player is allowed to know: the information set.

use crate::card::{Card, Player};
use crate::cardset::CardSet;
use crate::deal::{Deal, Phase, Trick};
use crate::rules::{legal_plays, Bid, Move};
use serde::{Deserialize, Serialize};

/// Visible state of one table slot.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Serialize, Deserialize)]
pub struct SlotView {
    pub top: Option<Card>,
    pub covered: bool,
}

/// Trick in progress.
#[derive(Clone, PartialEq, Eq, Debug, Serialize, Deserialize)]
pub struct CurrentTrick {
    pub leader: Player,
    pub cards: Vec<Card>,
}

/// How much of the played-card history a player remembers.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MemoryModel {
    /// Every played card is remembered (a perfect counter).
    Perfect,
    /// Remembers trumps, 10s, aces, voids and the last trick; forgets other low cards.
    Partial,
    /// Remembers only the last trick.
    LastTrickOnly,
}

/// Everything player `me` may know. The AI only ever receives this, never a `Deal`.
#[derive(Clone, PartialEq, Debug, Serialize, Deserialize)]
pub struct PlayerView {
    pub me: Player,
    pub dealer: Player,
    pub phase: Phase,
    pub to_move: Option<Player>,
    pub bid: Option<Bid>,
    pub contra: bool,
    pub multiplier: u8,
    pub hand: CardSet,
    pub opp_hand_count: u8,
    pub table: [[SlotView; 4]; 2],
    pub current_trick: Option<CurrentTrick>,
    pub last_trick: Option<Trick>,
    pub tricks_played: u8,
    pub points: [u8; 2],
    /// Cards known to be out of play (as far as this memory model remembers).
    pub played: CardSet,
    /// Cards whose location `me` cannot know: opponent's hand, face-down cards (and forgotten cards).
    pub unknown: CardSet,
    pub legal_moves: Vec<Move>,
    /// Cards that cannot be in the opponent's hand (inferred from their forced plays).
    #[serde(default, skip_serializing)]
    pub opp_hand_forbidden: CardSet,
}

impl PlayerView {
    /// Build the perfect-memory view of `me`.
    pub fn of(deal: &Deal, me: Player) -> PlayerView {
        let opp = me.other();
        let mut table = [[SlotView { top: None, covered: false }; 4]; 2];
        let mut visible = CardSet::EMPTY;
        for p in [Player::P0, Player::P1] {
            for i in 0..4 {
                let (top, covered) = deal.slot_view(p, i);
                table[p.index()][i] = SlotView { top, covered };
                if let Some(c) = top {
                    visible.insert(c);
                }
            }
        }
        let hand = deal.hand(me);
        let played = deal.played();
        let unknown = CardSet::ALL - played - hand - visible;
        let opp_hand_count = deal.hand(opp).len() as u8;
        let to_move = deal.to_move();
        let legal_moves = if to_move == Some(me) { deal.legal_moves() } else { Vec::new() };
        let current_trick = deal.current_trick().map(|(leader, c)| CurrentTrick { leader, cards: vec![c] });
        let mut view = PlayerView {
            me,
            dealer: deal.dealer(),
            phase: deal.phase(),
            to_move,
            bid: deal.bid(),
            contra: deal.contra(),
            multiplier: deal.multiplier(),
            hand,
            opp_hand_count,
            table,
            current_trick,
            last_trick: deal.last_trick(),
            tricks_played: deal.tricks().len() as u8,
            points: deal.points(),
            played,
            unknown,
            legal_moves,
            opp_hand_forbidden: CardSet::EMPTY,
        };
        view.opp_hand_forbidden = infer_forbidden(deal, me, view.unknown);
        view
    }

    /// Face-up table cards of `p`.
    pub fn visible_table(&self, p: Player) -> CardSet {
        CardSet::from_cards(self.table[p.index()].iter().filter_map(|s| s.top))
    }

    /// Cards `me` may play right now (hand + own face-up cards).
    pub fn available(&self) -> CardSet {
        self.hand | self.visible_table(self.me)
    }

    /// Number of face-down cards still covered on `p`'s row.
    pub fn covered_count(&self, p: Player) -> u8 {
        self.table[p.index()].iter().filter(|s| s.covered).count() as u8
    }

    /// Card led in the trick in progress, if any.
    pub fn led(&self) -> Option<Card> {
        self.current_trick.as_ref().and_then(|t| t.cards.first().copied())
    }

    /// Legal cards for `me` if it is `me`'s turn to play.
    pub fn legal_cards(&self) -> CardSet {
        match self.bid {
            Some(bid) if self.phase == Phase::Play && self.to_move == Some(self.me) => {
                legal_plays(self.available(), self.led(), bid)
            }
            _ => CardSet::EMPTY,
        }
    }

    /// Degrade this view to what a player with the given memory remembers.
    /// Forgotten cards move from `played` to `unknown`; constraints derived from
    /// forgotten tricks are dropped for `LastTrickOnly`.
    pub fn with_memory(mut self, model: MemoryModel) -> PlayerView {
        match model {
            MemoryModel::Perfect => self,
            MemoryModel::Partial => {
                let trumps = self.bid.map(|b| b.trump_set()).unwrap_or(CardSet::EMPTY);
                let keep = trumps | CardSet::ALL.of_rank(crate::card::Rank::Ten) | CardSet::ALL.of_rank(crate::card::Rank::Ace);
                let last = self.last_trick.map(|t| CardSet::from_cards(t.cards)).unwrap_or(CardSet::EMPTY);
                let forgotten = self.played - keep - last;
                self.played = self.played - forgotten;
                self.unknown = self.unknown | forgotten;
                self
            }
            MemoryModel::LastTrickOnly => {
                let last = self.last_trick.map(|t| CardSet::from_cards(t.cards)).unwrap_or(CardSet::EMPTY);
                let forgotten = self.played - last;
                self.played = self.played - forgotten;
                self.unknown = self.unknown | forgotten;
                // Only the last trick can still support an inference.
                let mut forbidden = CardSet::EMPTY;
                if let (Some(t), Some(bid)) = (self.last_trick, self.bid) {
                    if t.leader == self.me {
                        forbidden = forbidden_from_follow(t.cards[0], t.cards[1], bid, self.unknown);
                    }
                }
                self.opp_hand_forbidden = forbidden;
                self
            }
        }
    }
}

/// Cards among `unknown` that the follower cannot have held in hand, given that they
/// answered `led` with `follow` legally.
fn forbidden_from_follow(led: Card, follow: Card, bid: Bid, unknown: CardSet) -> CardSet {
    let mut forbidden = CardSet::EMPTY;
    for c in unknown.iter() {
        let hypothetical = CardSet::single(follow).with(c);
        if !legal_plays(hypothetical, Some(led), bid).contains(follow) {
            forbidden.insert(c);
        }
    }
    forbidden
}

/// Infer which currently-unknown cards cannot be in the opponent's hand, from every trick
/// in which the opponent followed. A follower's choice is forced by what they hold, and a
/// hand only shrinks, so every inference stays valid for the rest of the deal.
fn infer_forbidden(deal: &Deal, me: Player, unknown: CardSet) -> CardSet {
    let bid = match deal.bid() {
        Some(b) => b,
        None => return CardSet::EMPTY,
    };
    let mut forbidden = CardSet::EMPTY;
    for t in deal.tricks() {
        if t.leader == me {
            forbidden = forbidden | forbidden_from_follow(t.cards[0], t.cards[1], bid, unknown);
        }
    }
    forbidden
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::card::{Rank, Suit};
    use crate::deal::Deal;
    use rand::rngs::SmallRng;
    use rand::{Rng, SeedableRng};

    #[test]
    fn view_partitions_the_deck_and_never_excludes_truth() {
        for seed in 0..100u64 {
            let mut rng = SmallRng::seed_from_u64(seed);
            let mut deal = Deal::random(&mut rng, Player::P0);
            deal.apply(Move::Bid { bid: Bid::Trump { suit: Suit::from_index(seed as usize % 4) } }).unwrap();
            deal.apply(Move::Contra { contra: false }).unwrap();
            while !deal.is_over() {
                for me in [Player::P0, Player::P1] {
                    let v = PlayerView::of(&deal, me);
                    let visible = v.visible_table(Player::P0) | v.visible_table(Player::P1);
                    assert_eq!(v.hand | visible | v.played | v.unknown, CardSet::ALL);
                    assert!((v.hand & v.unknown).is_empty());
                    assert!((v.played & v.unknown).is_empty());
                    assert_eq!(v.unknown.len(), v.opp_hand_count as u32 + v.covered_count(Player::P0) as u32 + v.covered_count(Player::P1) as u32);
                    // the true opponent hand never intersects the forbidden set
                    let opp_hand = deal.hand(me.other());
                    assert!((opp_hand & v.opp_hand_forbidden).is_empty(), "seed {seed}: forbidden {:?} vs hand {:?}", v.opp_hand_forbidden, opp_hand);
                    assert!(v.opp_hand_forbidden.0 & !v.unknown.0 == 0);
                    if v.to_move == Some(me) {
                        let legal = CardSet::from_cards(v.legal_moves.iter().filter_map(|m| m.card()));
                        assert_eq!(legal, v.legal_cards());
                    }
                    // memory models only ever enlarge the unknown set
                    let partial = v.clone().with_memory(MemoryModel::Partial);
                    assert!(partial.unknown.0 & v.unknown.0 == v.unknown.0);
                    let last = v.clone().with_memory(MemoryModel::LastTrickOnly);
                    assert!(last.unknown.0 & partial.unknown.0 == partial.unknown.0);
                    assert!((opp_hand & last.opp_hand_forbidden).is_empty());
                }
                let legal = deal.legal_moves();
                let pick = legal[rng.gen_range(0..legal.len())];
                deal.apply(pick).unwrap();
            }
        }
    }

    #[test]
    fn inference_detects_void_and_no_higher() {
        let bid = Bid::Trump { suit: Suit::Hearts };
        let unknown = CardSet::ALL;
        // opponent answered KC with 7C: cannot hold AC or 10C
        let f = forbidden_from_follow(Card::new(Suit::Clubs, Rank::King), Card::new(Suit::Clubs, Rank::Seven), bid, unknown);
        assert_eq!(f, CardSet::from_cards([Card::new(Suit::Clubs, Rank::Ace), Card::new(Suit::Clubs, Rank::Ten)]));
        // opponent discarded 8D on KC in a hearts game: no clubs, no hearts
        let f = forbidden_from_follow(Card::new(Suit::Clubs, Rank::King), Card::new(Suit::Diamonds, Rank::Eight), bid, unknown);
        assert_eq!(f, CardSet::suit_mask(Suit::Clubs) | CardSet::suit_mask(Suit::Hearts));
        // opponent ruffed with 7H: no clubs, but may hold higher hearts
        let f = forbidden_from_follow(Card::new(Suit::Clubs, Rank::King), Card::new(Suit::Hearts, Rank::Seven), bid, unknown);
        assert_eq!(f, CardSet::suit_mask(Suit::Clubs));
    }
}
