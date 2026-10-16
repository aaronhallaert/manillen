//! Deal layout, perfect-information playing position, and the deal state machine.

use crate::card::{Card, Player};
use crate::cardset::CardSet;
use crate::rules::{beats, legal_plays, trick_points, Bid, DealResult, Move};
use rand::seq::SliceRandom;
use rand::Rng;
use serde::{Deserialize, Serialize};
use std::fmt;

/// A table slot as dealt: the face-up card and the face-down card beneath it.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Serialize, Deserialize)]
pub struct SlotPair {
    pub top: Card,
    pub under: Card,
}

/// The full dealt layout of a deal (all 32 cards, including hidden ones).
#[derive(Clone, PartialEq, Eq, Debug, Serialize, Deserialize)]
pub struct Layout {
    pub hands: [CardSet; 2],
    pub table: [[SlotPair; 4]; 2],
}

impl Layout {
    /// Deal a random layout. The non-dealer receives cards first, but ownership is
    /// all that matters for play.
    pub fn deal<R: Rng + ?Sized>(rng: &mut R) -> Layout {
        let mut deck: Vec<Card> = Card::all().collect();
        deck.shuffle(rng);
        Layout::from_deck(&deck)
    }

    /// Build a layout from a 32-card permutation: under cards first (4 per player),
    /// then tops (4 per player), then hands (8 per player).
    pub fn from_deck(deck: &[Card]) -> Layout {
        assert_eq!(deck.len(), 32);
        let mut table = [[SlotPair { top: Card(0), under: Card(0) }; 4]; 2];
        for p in 0..2 {
            for s in 0..4 {
                table[p][s].under = deck[p * 4 + s];
                table[p][s].top = deck[8 + p * 4 + s];
            }
        }
        let hands = [
            CardSet::from_cards(deck[16..24].iter().copied()),
            CardSet::from_cards(deck[24..32].iter().copied()),
        ];
        Layout { hands, table }
    }

    /// All 16 cards owned by `p`.
    pub fn cards_of(&self, p: Player) -> CardSet {
        let mut s = self.hands[p.index()];
        for slot in &self.table[p.index()] {
            s.insert(slot.top);
            s.insert(slot.under);
        }
        s
    }

    pub fn tops(&self, p: Player) -> CardSet {
        CardSet::from_cards(self.table[p.index()].iter().map(|s| s.top))
    }

    pub fn unders(&self, p: Player) -> CardSet {
        CardSet::from_cards(self.table[p.index()].iter().map(|s| s.under))
    }

    /// Face-down cards of `p` that are still blocked, given the cards `p` has not yet played.
    #[inline]
    pub fn blocked(&self, p: Player, remaining: CardSet) -> CardSet {
        let mut b = CardSet::EMPTY;
        for slot in &self.table[p.index()] {
            if remaining.contains(slot.top) {
                b.insert(slot.under);
            }
        }
        b
    }

    /// Cards `p` may play right now.
    #[inline]
    pub fn available(&self, p: Player, remaining: CardSet) -> CardSet {
        remaining - self.blocked(p, remaining)
    }

    /// Slot index of `p` whose top or under card is `card`.
    pub fn slot_of(&self, p: Player, card: Card) -> Option<usize> {
        self.table[p.index()].iter().position(|s| s.top == card || s.under == card)
    }

    /// Check that the layout contains every card exactly once.
    pub fn validate(&self) -> Result<(), DealError> {
        let mut all = CardSet::EMPTY;
        let mut count = 0;
        for p in [Player::P0, Player::P1] {
            count += self.hands[p.index()].len();
            all = all | self.hands[p.index()];
            for s in &self.table[p.index()] {
                count += 2;
                all.insert(s.top);
                all.insert(s.under);
            }
        }
        if count == 32 && all == CardSet::ALL && self.hands[0].len() == 8 && self.hands[1].len() == 8 {
            Ok(())
        } else {
            Err(DealError::InvalidLayout)
        }
    }
}

/// Outcome of a completed trick.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Serialize, Deserialize)]
pub struct Trick {
    pub leader: Player,
    pub cards: [Card; 2],
    pub winner: Player,
    pub points: u8,
}

/// Perfect-information playing position. Shared by the authoritative deal state and the solver.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct Position {
    pub layout: Layout,
    pub bid: Bid,
    /// Cards each player has not yet played (hand + table, blocked or not).
    pub remaining: [CardSet; 2],
    pub leader: Player,
    pub led: Option<Card>,
    pub points: [u8; 2],
    pub tricks_played: u8,
}

impl Position {
    pub fn new(layout: Layout, bid: Bid, first_leader: Player) -> Position {
        let remaining = [layout.cards_of(Player::P0), layout.cards_of(Player::P1)];
        Position { layout, bid, remaining, leader: first_leader, led: None, points: [0, 0], tricks_played: 0 }
    }

    #[inline]
    pub fn to_move(&self) -> Player {
        if self.led.is_none() {
            self.leader
        } else {
            self.leader.other()
        }
    }

    #[inline]
    pub fn is_over(&self) -> bool {
        self.tricks_played >= 16
    }

    #[inline]
    pub fn available(&self, p: Player) -> CardSet {
        self.layout.available(p, self.remaining[p.index()])
    }

    #[inline]
    pub fn unplayed(&self) -> CardSet {
        self.remaining[0] | self.remaining[1]
    }

    /// Points still to be won: the points of all unplayed cards. (Computed from the cards,
    /// not from the score, so that sampled worlds with a different card population stay consistent.)
    #[inline]
    pub fn remaining_points(&self) -> u8 {
        self.unplayed().points()
    }

    /// Build a position directly from a (possibly sampled) layout and explicit remaining sets.
    /// `layout.hands` is not used by the solver; only the slot structure matters.
    pub fn from_world(layout: Layout, bid: Bid, remaining: [CardSet; 2], leader: Player, led: Option<Card>, points: [u8; 2], tricks_played: u8) -> Position {
        Position { layout, bid, remaining, leader, led, points, tricks_played }
    }

    /// Legal cards for the player to move.
    #[inline]
    pub fn legal(&self) -> CardSet {
        legal_plays(self.available(self.to_move()), self.led, self.bid)
    }

    /// Play a card for the player to move. Returns the completed trick, if any.
    /// Does not validate legality beyond ownership (callers use `legal()`).
    pub fn play(&mut self, card: Card) -> Option<Trick> {
        let p = self.to_move();
        debug_assert!(self.remaining[p.index()].contains(card));
        self.remaining[p.index()].remove(card);
        match self.led {
            None => {
                self.led = Some(card);
                None
            }
            Some(led) => {
                let follower_wins = beats(card, led, self.bid);
                let winner = if follower_wins { p } else { self.leader };
                let points = trick_points(led, card);
                self.points[winner.index()] += points;
                let trick = Trick { leader: self.leader, cards: [led, card], winner, points };
                self.leader = winner;
                self.led = None;
                self.tricks_played += 1;
                Some(trick)
            }
        }
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Debug, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Phase {
    Bidding,
    Contra,
    Play,
    Done,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum DealError {
    InvalidLayout,
    WrongPhase,
    NotYourTurn,
    IllegalMove,
}

impl fmt::Display for DealError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let s = match self {
            DealError::InvalidLayout => "invalid layout",
            DealError::WrongPhase => "move not allowed in this phase",
            DealError::NotYourTurn => "not this player's turn",
            DealError::IllegalMove => "illegal move",
        };
        f.write_str(s)
    }
}

impl std::error::Error for DealError {}

/// Something that happened as a result of applying a move.
#[derive(Clone, PartialEq, Eq, Debug, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum Event {
    Bid { player: Player, bid: Bid },
    Contra { player: Player, contra: bool },
    Played { player: Player, card: Card, from_table: bool },
    Flipped { player: Player, slot: u8, card: Card },
    Trick { trick: Trick },
    DealOver { result: DealResult },
    MatchOver { winner: Player },
}

/// Authoritative state of one deal, including all hidden cards.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct Deal {
    dealer: Player,
    phase: Phase,
    bid: Option<Bid>,
    contra: bool,
    pos: Position,
    tricks: Vec<Trick>,
    moves: Vec<Move>,
}

impl Deal {
    pub fn new(layout: Layout, dealer: Player) -> Result<Deal, DealError> {
        layout.validate()?;
        let pos = Position::new(layout, Bid::NoTrump, dealer.other());
        Ok(Deal { dealer, phase: Phase::Bidding, bid: None, contra: false, pos, tricks: Vec::new(), moves: Vec::new() })
    }

    pub fn random<R: Rng + ?Sized>(rng: &mut R, dealer: Player) -> Deal {
        Deal::new(Layout::deal(rng), dealer).expect("random layout is valid")
    }

    #[inline]
    pub fn dealer(&self) -> Player {
        self.dealer
    }
    #[inline]
    pub fn phase(&self) -> Phase {
        self.phase
    }
    #[inline]
    pub fn bid(&self) -> Option<Bid> {
        self.bid
    }
    #[inline]
    pub fn contra(&self) -> bool {
        self.contra
    }
    #[inline]
    pub fn multiplier(&self) -> u8 {
        self.bid.map(|b| b.multiplier()).unwrap_or(1) * if self.contra { 2 } else { 1 }
    }
    #[inline]
    pub fn layout(&self) -> &Layout {
        &self.pos.layout
    }
    #[inline]
    pub fn position(&self) -> &Position {
        &self.pos
    }
    #[inline]
    pub fn tricks(&self) -> &[Trick] {
        &self.tricks
    }
    #[inline]
    pub fn moves(&self) -> &[Move] {
        &self.moves
    }
    #[inline]
    pub fn points(&self) -> [u8; 2] {
        self.pos.points
    }
    #[inline]
    pub fn is_over(&self) -> bool {
        self.phase == Phase::Done
    }

    /// Cards `p` has played so far.
    pub fn played_by(&self, p: Player) -> CardSet {
        self.pos.layout.cards_of(p) - self.pos.remaining[p.index()]
    }

    pub fn played(&self) -> CardSet {
        CardSet::ALL - self.pos.unplayed()
    }

    /// The card led in the trick in progress, if any.
    #[inline]
    pub fn led(&self) -> Option<Card> {
        self.pos.led
    }

    pub fn to_move(&self) -> Option<Player> {
        match self.phase {
            Phase::Bidding => Some(self.dealer),
            Phase::Contra => Some(self.dealer.other()),
            Phase::Play => Some(self.pos.to_move()),
            Phase::Done => None,
        }
    }

    /// Hand cards of `p` (cards not on the table and not yet played).
    pub fn hand(&self, p: Player) -> CardSet {
        self.pos.remaining[p.index()] & self.pos.layout.hands[p.index()]
    }

    /// Visible state of slot `i` of `p`: the face-up card and whether a hidden card lies beneath.
    pub fn slot_view(&self, p: Player, i: usize) -> (Option<Card>, bool) {
        let slot = self.pos.layout.table[p.index()][i];
        let rem = self.pos.remaining[p.index()];
        if rem.contains(slot.top) {
            (Some(slot.top), true)
        } else if rem.contains(slot.under) {
            (Some(slot.under), false)
        } else {
            (None, false)
        }
    }

    /// Face-up table cards of `p`.
    pub fn visible_table(&self, p: Player) -> CardSet {
        let mut s = CardSet::EMPTY;
        for i in 0..4 {
            if let (Some(c), _) = self.slot_view(p, i) {
                s.insert(c);
            }
        }
        s
    }

    pub fn legal_moves(&self) -> Vec<Move> {
        match self.phase {
            Phase::Bidding => Bid::ALL.iter().map(|&bid| Move::Bid { bid }).collect(),
            Phase::Contra => vec![Move::Contra { contra: true }, Move::Contra { contra: false }],
            Phase::Play => self.pos.legal().iter().map(|card| Move::Play { card }).collect(),
            Phase::Done => Vec::new(),
        }
    }

    pub fn is_legal(&self, mv: Move) -> bool {
        match (self.phase, mv) {
            (Phase::Bidding, Move::Bid { .. }) => true,
            (Phase::Contra, Move::Contra { .. }) => true,
            (Phase::Play, Move::Play { card }) => self.pos.legal().contains(card),
            _ => false,
        }
    }

    pub fn apply(&mut self, mv: Move) -> Result<Vec<Event>, DealError> {
        let mut events = Vec::with_capacity(4);
        match (self.phase, mv) {
            (Phase::Bidding, Move::Bid { bid }) => {
                self.bid = Some(bid);
                self.pos.bid = bid;
                self.phase = Phase::Contra;
                events.push(Event::Bid { player: self.dealer, bid });
            }
            (Phase::Contra, Move::Contra { contra }) => {
                self.contra = contra;
                self.phase = Phase::Play;
                events.push(Event::Contra { player: self.dealer.other(), contra });
            }
            (Phase::Play, Move::Play { card }) => {
                if !self.pos.legal().contains(card) {
                    return Err(DealError::IllegalMove);
                }
                let p = self.pos.to_move();
                let from_table = !self.pos.layout.hands[p.index()].contains(card);
                let slot = if from_table { self.pos.layout.slot_of(p, card) } else { None };
                let trick = self.pos.play(card);
                events.push(Event::Played { player: p, card, from_table });
                if let Some(i) = slot {
                    let pair = self.pos.layout.table[p.index()][i];
                    if pair.top == card {
                        events.push(Event::Flipped { player: p, slot: i as u8, card: pair.under });
                    }
                }
                if let Some(t) = trick {
                    self.tricks.push(t);
                    events.push(Event::Trick { trick: t });
                    if self.pos.is_over() {
                        self.phase = Phase::Done;
                        events.push(Event::DealOver { result: self.result().expect("deal is over") });
                    }
                }
            }
            (Phase::Done, _) => return Err(DealError::WrongPhase),
            _ => return Err(DealError::WrongPhase),
        }
        self.moves.push(mv);
        Ok(events)
    }

    pub fn result(&self) -> Option<DealResult> {
        if self.phase != Phase::Done {
            return None;
        }
        Some(DealResult::compute(self.dealer, self.bid.expect("bid set"), self.contra, self.pos.points))
    }

    pub fn current_trick(&self) -> Option<(Player, Card)> {
        self.pos.led.map(|c| (self.pos.leader, c))
    }

    pub fn last_trick(&self) -> Option<Trick> {
        self.tricks.last().copied()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::card::{Rank, Suit};
    use rand::rngs::SmallRng;
    use rand::SeedableRng;

    #[test]
    fn deal_is_valid_and_disjoint() {
        let mut rng = SmallRng::seed_from_u64(1);
        let l = Layout::deal(&mut rng);
        l.validate().unwrap();
        assert!((l.cards_of(Player::P0) & l.cards_of(Player::P1)).is_empty());
        assert_eq!(l.cards_of(Player::P0).len(), 16);
    }

    #[test]
    fn blocked_cards_unlock_when_top_played() {
        let mut rng = SmallRng::seed_from_u64(7);
        let l = Layout::deal(&mut rng);
        let top = l.table[0][2].top;
        let under = l.table[0][2].under;
        let mut pos = Position::new(l.clone(), Bid::NoTrump, Player::P0);
        assert!(!pos.available(Player::P0).contains(under));
        assert!(pos.available(Player::P0).contains(top));
        pos.play(top);
        assert!(pos.available(Player::P0).contains(under));
    }

    #[test]
    fn full_random_deal_invariants() {
        for seed in 0..200u64 {
            let mut rng = SmallRng::seed_from_u64(seed);
            let mut deal = Deal::random(&mut rng, Player(seed as u8 & 1));
            assert_eq!(deal.phase(), Phase::Bidding);
            assert_eq!(deal.to_move(), Some(deal.dealer()));
            let bids = deal.legal_moves();
            assert_eq!(bids.len(), 5);
            deal.apply(bids[(seed % 5) as usize]).unwrap();
            assert_eq!(deal.phase(), Phase::Contra);
            deal.apply(Move::Contra { contra: seed % 3 == 0 }).unwrap();
            assert_eq!(deal.phase(), Phase::Play);
            assert_eq!(deal.to_move(), Some(deal.dealer().other()));
            let mut plays = 0;
            while !deal.is_over() {
                let legal = deal.legal_moves();
                assert!(!legal.is_empty());
                let p = deal.to_move().unwrap();
                let avail = deal.position().available(p);
                for m in &legal {
                    assert!(avail.contains(m.card().unwrap()));
                }
                let pick = legal[rng.gen_range(0..legal.len())];
                deal.apply(pick).unwrap();
                plays += 1;
            }
            assert_eq!(plays, 32);
            assert_eq!(deal.tricks().len(), 16);
            let r = deal.result().unwrap();
            assert_eq!(r.points[0] as u32 + r.points[1] as u32, 60);
            assert_eq!(deal.played(), CardSet::ALL);
            assert!(deal.apply(Move::Play { card: Card(0) }).is_err());
        }
    }

    #[test]
    fn must_beat_is_enforced() {
        // Construct a deterministic layout: P0 hand gets all clubs + 7D..., simple check through legal()
        let mut deck: Vec<Card> = Card::all().collect();
        // arrange so that player 1's hand (deck[24..32]) is all clubs, and P0 hand (16..24) all diamonds
        let clubs: Vec<Card> = Rank::ALL.iter().map(|&r| Card::new(Suit::Clubs, r)).collect();
        let diamonds: Vec<Card> = Rank::ALL.iter().map(|&r| Card::new(Suit::Diamonds, r)).collect();
        let rest: Vec<Card> = deck.drain(..).filter(|c| c.suit() != Suit::Clubs && c.suit() != Suit::Diamonds).collect();
        let mut deck = rest;
        deck.extend(diamonds);
        deck.extend(clubs);
        let l = Layout::from_deck(&deck);
        let mut deal = Deal::new(l, Player::P1).unwrap();
        deal.apply(Move::Bid { bid: Bid::Trump { suit: Suit::Hearts } }).unwrap();
        deal.apply(Move::Contra { contra: false }).unwrap();
        // P0 leads: play a diamond from hand (7D)
        deal.apply(Move::Play { card: Card::new(Suit::Diamonds, Rank::Seven) }).unwrap();
        // P1 has no diamonds in hand; its table might have one. Legal moves must be diamonds if any available, else trumps.
        let legal = deal.legal_moves();
        let avail = deal.position().available(Player::P1);
        let diamonds_avail = avail.of_suit(Suit::Diamonds);
        if !diamonds_avail.is_empty() {
            for m in &legal {
                assert_eq!(m.card().unwrap().suit(), Suit::Diamonds);
                assert!(m.card().unwrap().rank() > Rank::Seven);
            }
        } else {
            let hearts = avail.of_suit(Suit::Hearts);
            let legal_set = CardSet::from_cards(legal.iter().filter_map(|m| m.card()));
            if hearts.is_empty() {
                assert_eq!(legal_set, avail);
            } else {
                assert_eq!(legal_set, hearts);
            }
        }
    }
}
