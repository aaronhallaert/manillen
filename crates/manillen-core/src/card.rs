//! Cards, suits, ranks and point values.

use serde::{Deserialize, Deserializer, Serialize, Serializer};
use std::fmt;

/// Suits in index order. The order has no game meaning.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug, PartialOrd, Ord)]
#[repr(u8)]
pub enum Suit {
    Clubs = 0,
    Diamonds = 1,
    Hearts = 2,
    Spades = 3,
}

impl Suit {
    pub const ALL: [Suit; 4] = [Suit::Clubs, Suit::Diamonds, Suit::Hearts, Suit::Spades];

    #[inline]
    pub fn index(self) -> usize {
        self as usize
    }

    #[inline]
    pub fn from_index(i: usize) -> Suit {
        Suit::ALL[i & 3]
    }

    pub fn letter(self) -> char {
        match self {
            Suit::Clubs => 'C',
            Suit::Diamonds => 'D',
            Suit::Hearts => 'H',
            Suit::Spades => 'S',
        }
    }

    pub fn symbol(self) -> char {
        match self {
            Suit::Clubs => '♣',
            Suit::Diamonds => '♦',
            Suit::Hearts => '♥',
            Suit::Spades => '♠',
        }
    }

    pub fn from_letter(c: char) -> Option<Suit> {
        match c.to_ascii_uppercase() {
            'C' => Some(Suit::Clubs),
            'D' => Some(Suit::Diamonds),
            'H' => Some(Suit::Hearts),
            'S' => Some(Suit::Spades),
            _ => None,
        }
    }
}

impl fmt::Display for Suit {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.letter())
    }
}

impl Serialize for Suit {
    fn serialize<S: Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        s.serialize_str(&self.letter().to_string())
    }
}

impl<'de> Deserialize<'de> for Suit {
    fn deserialize<D: Deserializer<'de>>(d: D) -> Result<Suit, D::Error> {
        let s = String::deserialize(d)?;
        let mut it = s.chars();
        match (it.next().and_then(Suit::from_letter), it.next()) {
            (Some(suit), None) => Ok(suit),
            _ => Err(serde::de::Error::custom(format!("invalid suit {s:?}"))),
        }
    }
}

/// Ranks in strength order: 7 is the lowest, 10 (the Manille) the highest.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug, PartialOrd, Ord)]
#[repr(u8)]
pub enum Rank {
    Seven = 0,
    Eight = 1,
    Nine = 2,
    Jack = 3,
    Queen = 4,
    King = 5,
    Ace = 6,
    Ten = 7,
}

impl Rank {
    pub const ALL: [Rank; 8] = [
        Rank::Seven,
        Rank::Eight,
        Rank::Nine,
        Rank::Jack,
        Rank::Queen,
        Rank::King,
        Rank::Ace,
        Rank::Ten,
    ];

    #[inline]
    pub fn index(self) -> usize {
        self as usize
    }

    #[inline]
    pub fn from_index(i: usize) -> Rank {
        Rank::ALL[i & 7]
    }

    /// Card points: 10=5, A=4, K=3, Q=2, J=1, others 0.
    #[inline]
    pub fn points(self) -> u8 {
        match self {
            Rank::Ten => 5,
            Rank::Ace => 4,
            Rank::King => 3,
            Rank::Queen => 2,
            Rank::Jack => 1,
            _ => 0,
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            Rank::Seven => "7",
            Rank::Eight => "8",
            Rank::Nine => "9",
            Rank::Jack => "J",
            Rank::Queen => "Q",
            Rank::King => "K",
            Rank::Ace => "A",
            Rank::Ten => "10",
        }
    }

    pub fn from_label(s: &str) -> Option<Rank> {
        match s.to_ascii_uppercase().as_str() {
            "7" => Some(Rank::Seven),
            "8" => Some(Rank::Eight),
            "9" => Some(Rank::Nine),
            "J" => Some(Rank::Jack),
            "Q" => Some(Rank::Queen),
            "K" => Some(Rank::King),
            "A" => Some(Rank::Ace),
            "10" | "T" => Some(Rank::Ten),
            _ => None,
        }
    }
}

/// A card is an index `suit * 8 + rank` in `0..32`.
#[derive(Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct Card(pub u8);

impl Card {
    pub const COUNT: usize = 32;

    #[inline]
    pub const fn new(suit: Suit, rank: Rank) -> Card {
        Card((suit as u8) << 3 | rank as u8)
    }

    #[inline]
    pub fn from_index(i: u8) -> Card {
        debug_assert!(i < 32);
        Card(i)
    }

    #[inline]
    pub fn index(self) -> usize {
        self.0 as usize
    }

    #[inline]
    pub fn suit(self) -> Suit {
        Suit::from_index((self.0 >> 3) as usize)
    }

    #[inline]
    pub fn rank(self) -> Rank {
        Rank::from_index((self.0 & 7) as usize)
    }

    #[inline]
    pub fn points(self) -> u8 {
        self.rank().points()
    }

    /// All 32 cards in index order.
    pub fn all() -> impl Iterator<Item = Card> {
        (0..32u8).map(Card)
    }

    /// Parse `"10H"`, `"AS"`, `"7c"` …
    pub fn parse(s: &str) -> Option<Card> {
        let s = s.trim();
        if s.len() < 2 {
            return None;
        }
        let (rank_str, suit_str) = s.split_at(s.len() - 1);
        let suit = Suit::from_letter(suit_str.chars().next()?)?;
        let rank = Rank::from_label(rank_str)?;
        Some(Card::new(suit, rank))
    }

    pub fn label(self) -> String {
        format!("{}{}", self.rank().label(), self.suit().letter())
    }
}

impl fmt::Display for Card {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}{}", self.rank().label(), self.suit().letter())
    }
}

impl fmt::Debug for Card {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{self}")
    }
}

impl Serialize for Card {
    fn serialize<S: Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        s.serialize_str(&self.label())
    }
}

impl<'de> Deserialize<'de> for Card {
    fn deserialize<D: Deserializer<'de>>(d: D) -> Result<Card, D::Error> {
        let s = String::deserialize(d)?;
        Card::parse(&s).ok_or_else(|| serde::de::Error::custom(format!("invalid card {s:?}")))
    }
}

/// Seat index, 0 or 1.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug, Default, Serialize, Deserialize, PartialOrd, Ord)]
#[serde(transparent)]
pub struct Player(pub u8);

impl Player {
    pub const P0: Player = Player(0);
    pub const P1: Player = Player(1);

    #[inline]
    pub fn other(self) -> Player {
        Player(self.0 ^ 1)
    }

    #[inline]
    pub fn index(self) -> usize {
        self.0 as usize
    }
}

impl fmt::Display for Player {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_and_label_round_trip() {
        for c in Card::all() {
            assert_eq!(Card::parse(&c.label()), Some(c));
        }
        assert_eq!(Card::parse("10H"), Some(Card::new(Suit::Hearts, Rank::Ten)));
        assert_eq!(Card::parse("as"), Some(Card::new(Suit::Spades, Rank::Ace)));
        assert_eq!(Card::parse("1H"), None);
        assert_eq!(Card::parse("X"), None);
    }

    #[test]
    fn total_points_is_60() {
        assert_eq!(Card::all().map(|c| c.points() as u32).sum::<u32>(), 60);
    }

    #[test]
    fn ten_is_highest() {
        assert!(Rank::Ten > Rank::Ace);
        assert!(Rank::Ace > Rank::King);
        assert!(Rank::Jack > Rank::Nine);
        assert!(Rank::Nine > Rank::Eight);
    }

    #[test]
    fn serde_card() {
        let c = Card::new(Suit::Diamonds, Rank::Ten);
        assert_eq!(serde_json::to_string(&c).unwrap(), "\"10D\"");
        let back: Card = serde_json::from_str("\"10D\"").unwrap();
        assert_eq!(back, c);
    }
}
