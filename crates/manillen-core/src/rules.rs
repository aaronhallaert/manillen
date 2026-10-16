//! Pure rule functions: bids, legal plays, trick winner, scoring.

use crate::card::{Card, Player, Suit};
use crate::cardset::CardSet;
use serde::{Deserialize, Serialize};
use std::fmt;

/// The dealer's choice: a trump suit or no trump ("muulen").
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum Bid {
    Trump { suit: Suit },
    NoTrump,
}

impl Bid {
    pub const ALL: [Bid; 5] = [
        Bid::Trump { suit: Suit::Clubs },
        Bid::Trump { suit: Suit::Diamonds },
        Bid::Trump { suit: Suit::Hearts },
        Bid::Trump { suit: Suit::Spades },
        Bid::NoTrump,
    ];

    #[inline]
    pub fn trump_suit(self) -> Option<Suit> {
        match self {
            Bid::Trump { suit } => Some(suit),
            Bid::NoTrump => None,
        }
    }

    #[inline]
    pub fn is_trump(self, card: Card) -> bool {
        self.trump_suit() == Some(card.suit())
    }

    /// Cards of the trump suit (empty for no-trump).
    #[inline]
    pub fn trump_set(self) -> CardSet {
        match self.trump_suit() {
            Some(s) => CardSet::suit_mask(s),
            None => CardSet::EMPTY,
        }
    }

    /// Score multiplier contributed by the bid (2 for no trump).
    #[inline]
    pub fn multiplier(self) -> u8 {
        match self {
            Bid::Trump { .. } => 1,
            Bid::NoTrump => 2,
        }
    }
}

impl fmt::Display for Bid {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Bid::Trump { suit } => write!(f, "trump {}", suit.symbol()),
            Bid::NoTrump => write!(f, "no trump"),
        }
    }
}

/// A move in a deal.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum Move {
    Bid { bid: Bid },
    Contra { contra: bool },
    Play { card: Card },
}

impl Move {
    #[inline]
    pub fn card(self) -> Option<Card> {
        match self {
            Move::Play { card } => Some(card),
            _ => None,
        }
    }
}

impl fmt::Display for Move {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Move::Bid { bid } => write!(f, "bid {bid}"),
            Move::Contra { contra: true } => write!(f, "contra"),
            Move::Contra { contra: false } => write!(f, "no contra"),
            Move::Play { card } => write!(f, "{card}"),
        }
    }
}

/// Does `candidate` win against `led` (the card that opened the trick)?
#[inline]
pub fn beats(candidate: Card, led: Card, bid: Bid) -> bool {
    if candidate.suit() == led.suit() {
        candidate.rank() > led.rank()
    } else {
        bid.is_trump(candidate)
    }
}

/// Legal cards for the player to move, given the cards they may use.
///
/// * Leader: anything available.
/// * Follower: follow suit; if a card of the led suit beats the led card, must beat;
///   if void, must trump if possible; else anything.
pub fn legal_plays(available: CardSet, led: Option<Card>, bid: Bid) -> CardSet {
    let led = match led {
        None => return available,
        Some(l) => l,
    };
    let same = available.of_suit(led.suit());
    if !same.is_empty() {
        let higher = same.above(led);
        return if higher.is_empty() { same } else { higher };
    }
    let trumps = available & bid.trump_set();
    if !trumps.is_empty() {
        trumps
    } else {
        available
    }
}

/// Points carried by a trick.
#[inline]
pub fn trick_points(a: Card, b: Card) -> u8 {
    a.points() + b.points()
}

/// Result of a finished deal.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Serialize, Deserialize)]
pub struct DealResult {
    pub dealer: Player,
    pub bid: Bid,
    pub contra: bool,
    pub multiplier: u8,
    pub points: [u8; 2],
    pub winner: Option<Player>,
    /// Winner's points above 30 (0 on a tie).
    pub base: u8,
    /// `base * multiplier`.
    pub score: u16,
}

impl DealResult {
    pub fn compute(dealer: Player, bid: Bid, contra: bool, points: [u8; 2]) -> DealResult {
        debug_assert_eq!(points[0] as u32 + points[1] as u32, 60);
        let multiplier = bid.multiplier() * if contra { 2 } else { 1 };
        let (winner, base) = if points[0] > points[1] {
            (Some(Player::P0), points[0] - 30)
        } else if points[1] > points[0] {
            (Some(Player::P1), points[1] - 30)
        } else {
            (None, 0)
        };
        DealResult {
            dealer,
            bid,
            contra,
            multiplier,
            points,
            winner,
            base,
            score: base as u16 * multiplier as u16,
        }
    }

    /// Signed score from `player`'s point of view.
    pub fn signed_score(&self, player: Player) -> i32 {
        match self.winner {
            Some(w) if w == player => self.score as i32,
            Some(_) => -(self.score as i32),
            None => 0,
        }
    }
}

/// Signed deal score for a player who collected `points` card points.
#[inline]
pub fn signed_score_for_points(points: i32, multiplier: i32) -> i32 {
    (points - 30) * multiplier
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::card::Rank;

    fn c(s: &str) -> Card {
        Card::parse(s).unwrap()
    }
    fn set(s: &str) -> CardSet {
        CardSet::from_cards(s.split_whitespace().map(c))
    }
    const H: Bid = Bid::Trump { suit: Suit::Hearts };

    #[test]
    fn follow_suit_must_beat() {
        // led QC, holding 7C KC 10H 7D: must play KC
        assert_eq!(legal_plays(set("7C KC 10H 7D"), Some(c("QC")), H), set("KC"));
        // holding only lower clubs: any club
        assert_eq!(legal_plays(set("7C 9C 10H"), Some(c("QC")), H), set("7C 9C"));
        // led 10C: nothing beats; any club
        assert_eq!(legal_plays(set("7C AC 10H"), Some(c("10C")), H), set("7C AC"));
    }

    #[test]
    fn void_must_trump_else_anything() {
        assert_eq!(legal_plays(set("7H KS 8D"), Some(c("QC")), H), set("7H"));
        assert_eq!(legal_plays(set("KS 8D"), Some(c("QC")), H), set("KS 8D"));
        // no-trump: void → anything
        assert_eq!(legal_plays(set("7H KS 8D"), Some(c("QC")), Bid::NoTrump), set("7H KS 8D"));
    }

    #[test]
    fn trump_led_forced_undertrump() {
        // led KH (trump), holding 7H 9H: must follow with a lower trump
        assert_eq!(legal_plays(set("7H 9H AS"), Some(c("KH")), H), set("7H 9H"));
        // holding AH too: must beat
        assert_eq!(legal_plays(set("7H AH AS"), Some(c("KH")), H), set("AH"));
    }

    #[test]
    fn leader_plays_anything() {
        assert_eq!(legal_plays(set("7H AH AS"), None, H), set("7H AH AS"));
    }

    #[test]
    fn beats_rules() {
        assert!(beats(c("10C"), c("AC"), H));
        assert!(!beats(c("AC"), c("10C"), H));
        assert!(beats(c("7H"), c("10C"), H));
        assert!(!beats(c("7H"), c("10C"), Bid::NoTrump));
        assert!(!beats(c("AS"), c("7C"), H));
        assert!(beats(c("AH"), c("KH"), H));
        assert!(!beats(c("7H"), c("KH"), H));
        assert_eq!(Rank::Ten.points(), 5);
    }

    #[test]
    fn scoring() {
        let r = DealResult::compute(Player::P1, Bid::NoTrump, true, [38, 22]);
        assert_eq!(r.winner, Some(Player::P0));
        assert_eq!(r.base, 8);
        assert_eq!(r.multiplier, 4);
        assert_eq!(r.score, 32);
        assert_eq!(r.signed_score(Player::P1), -32);
        let tie = DealResult::compute(Player::P0, H, false, [30, 30]);
        assert_eq!(tie.winner, None);
        assert_eq!(tie.score, 0);
    }

    #[test]
    fn serde_shapes() {
        assert_eq!(
            serde_json::to_string(&Move::Bid { bid: H }).unwrap(),
            r#"{"type":"bid","bid":{"type":"trump","suit":"H"}}"#
        );
        assert_eq!(serde_json::to_string(&Move::Bid { bid: Bid::NoTrump }).unwrap(), r#"{"type":"bid","bid":{"type":"no_trump"}}"#);
        assert_eq!(serde_json::to_string(&Move::Contra { contra: true }).unwrap(), r#"{"type":"contra","contra":true}"#);
        assert_eq!(serde_json::to_string(&Move::Play { card: c("10H") }).unwrap(), r#"{"type":"play","card":"10H"}"#);
    }
}
