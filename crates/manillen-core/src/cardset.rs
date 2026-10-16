//! A set of cards as a 32-bit mask.

use crate::card::{Card, Rank, Suit};
use serde::{Deserialize, Deserializer, Serialize, Serializer};
use std::fmt;
use std::ops::{BitAnd, BitOr, BitXor, Not, Sub};

#[derive(Clone, Copy, PartialEq, Eq, Hash, Default, PartialOrd, Ord)]
pub struct CardSet(pub u32);

impl CardSet {
    pub const EMPTY: CardSet = CardSet(0);
    pub const ALL: CardSet = CardSet(0xFFFF_FFFF);

    #[inline]
    pub const fn suit_mask(suit: Suit) -> CardSet {
        CardSet(0xFF << ((suit as u32) * 8))
    }

    #[inline]
    pub fn single(card: Card) -> CardSet {
        CardSet(1 << card.0)
    }

    #[inline]
    pub fn is_empty(self) -> bool {
        self.0 == 0
    }

    #[inline]
    pub fn len(self) -> u32 {
        self.0.count_ones()
    }

    #[inline]
    pub fn contains(self, card: Card) -> bool {
        self.0 & (1 << card.0) != 0
    }

    #[inline]
    pub fn insert(&mut self, card: Card) {
        self.0 |= 1 << card.0;
    }

    #[inline]
    pub fn remove(&mut self, card: Card) {
        self.0 &= !(1 << card.0);
    }

    #[inline]
    pub fn with(self, card: Card) -> CardSet {
        CardSet(self.0 | (1 << card.0))
    }

    #[inline]
    pub fn without(self, card: Card) -> CardSet {
        CardSet(self.0 & !(1 << card.0))
    }

    #[inline]
    pub fn of_suit(self, suit: Suit) -> CardSet {
        self & CardSet::suit_mask(suit)
    }

    /// Cards of the same suit as `card` that rank higher than it.
    #[inline]
    pub fn above(self, card: Card) -> CardSet {
        let suit_base = (card.0 & !7) as u32;
        let rank = (card.0 & 7) as u32;
        // bits for ranks > rank within the suit
        let higher = (0xFFu32 << (rank + 1)) & 0xFF;
        CardSet(self.0 & (higher << suit_base))
    }

    /// Cards of the same suit as `card` that rank lower than it.
    #[inline]
    pub fn below(self, card: Card) -> CardSet {
        let suit_base = (card.0 & !7) as u32;
        let rank = (card.0 & 7) as u32;
        let lower = (1u32 << rank) - 1;
        CardSet(self.0 & (lower << suit_base))
    }

    /// Lowest card (by index, i.e. lowest suit then lowest rank).
    #[inline]
    pub fn lowest(self) -> Option<Card> {
        if self.0 == 0 {
            None
        } else {
            Some(Card(self.0.trailing_zeros() as u8))
        }
    }

    /// Highest card (by index).
    #[inline]
    pub fn highest(self) -> Option<Card> {
        if self.0 == 0 {
            None
        } else {
            Some(Card(31 - self.0.leading_zeros() as u8))
        }
    }

    /// Sum of the points of all cards in the set.
    #[inline]
    pub fn points(self) -> u8 {
        const BASE: u32 = 0x0101_0101;
        let tens = (self.0 & (BASE << Rank::Ten.index())).count_ones();
        let aces = (self.0 & (BASE << Rank::Ace.index())).count_ones();
        let kings = (self.0 & (BASE << Rank::King.index())).count_ones();
        let queens = (self.0 & (BASE << Rank::Queen.index())).count_ones();
        let jacks = (self.0 & (BASE << Rank::Jack.index())).count_ones();
        (tens * 5 + aces * 4 + kings * 3 + queens * 2 + jacks) as u8
    }

    pub fn iter(self) -> CardSetIter {
        CardSetIter(self.0)
    }

    pub fn to_vec(self) -> Vec<Card> {
        self.iter().collect()
    }

    pub fn from_cards<I: IntoIterator<Item = Card>>(cards: I) -> CardSet {
        let mut s = CardSet::EMPTY;
        for c in cards {
            s.insert(c);
        }
        s
    }

    /// Cards with zero point value (7, 8, 9).
    #[inline]
    pub fn zero_point(self) -> CardSet {
        const Z: u32 = 0x0707_0707;
        CardSet(self.0 & Z)
    }

    #[inline]
    pub fn of_rank(self, rank: Rank) -> CardSet {
        const BASE: u32 = 0x0101_0101;
        CardSet(self.0 & (BASE << rank.index()))
    }
}

pub struct CardSetIter(u32);

impl Iterator for CardSetIter {
    type Item = Card;
    #[inline]
    fn next(&mut self) -> Option<Card> {
        if self.0 == 0 {
            None
        } else {
            let i = self.0.trailing_zeros();
            self.0 &= self.0 - 1;
            Some(Card(i as u8))
        }
    }
    fn size_hint(&self) -> (usize, Option<usize>) {
        let n = self.0.count_ones() as usize;
        (n, Some(n))
    }
}

impl ExactSizeIterator for CardSetIter {}

impl IntoIterator for CardSet {
    type Item = Card;
    type IntoIter = CardSetIter;
    fn into_iter(self) -> CardSetIter {
        self.iter()
    }
}

impl BitOr for CardSet {
    type Output = CardSet;
    #[inline]
    fn bitor(self, o: CardSet) -> CardSet {
        CardSet(self.0 | o.0)
    }
}
impl BitAnd for CardSet {
    type Output = CardSet;
    #[inline]
    fn bitand(self, o: CardSet) -> CardSet {
        CardSet(self.0 & o.0)
    }
}
impl BitXor for CardSet {
    type Output = CardSet;
    #[inline]
    fn bitxor(self, o: CardSet) -> CardSet {
        CardSet(self.0 ^ o.0)
    }
}
impl Sub for CardSet {
    type Output = CardSet;
    #[inline]
    fn sub(self, o: CardSet) -> CardSet {
        CardSet(self.0 & !o.0)
    }
}
impl Not for CardSet {
    type Output = CardSet;
    #[inline]
    fn not(self) -> CardSet {
        CardSet(!self.0)
    }
}

impl fmt::Debug for CardSet {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{{")?;
        for (i, c) in self.iter().enumerate() {
            if i > 0 {
                write!(f, " ")?;
            }
            write!(f, "{c}")?;
        }
        write!(f, "}}")
    }
}

impl fmt::Display for CardSet {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        for (i, c) in self.iter().enumerate() {
            if i > 0 {
                write!(f, " ")?;
            }
            write!(f, "{c}")?;
        }
        Ok(())
    }
}

impl Serialize for CardSet {
    fn serialize<S: Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        self.to_vec().serialize(s)
    }
}

impl<'de> Deserialize<'de> for CardSet {
    fn deserialize<D: Deserializer<'de>>(d: D) -> Result<CardSet, D::Error> {
        let v: Vec<Card> = Vec::deserialize(d)?;
        Ok(CardSet::from_cards(v))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn above_below() {
        let s = CardSet::suit_mask(Suit::Hearts);
        let q = Card::new(Suit::Hearts, Rank::Queen);
        assert_eq!(s.above(q).len(), 3); // K A 10
        assert_eq!(s.below(q).len(), 4); // 7 8 9 J
        let ten = Card::new(Suit::Hearts, Rank::Ten);
        assert!(s.above(ten).is_empty());
        assert!(CardSet::suit_mask(Suit::Spades).above(q).is_empty());
    }

    #[test]
    fn zero_point_mask() {
        assert_eq!(CardSet::ALL.zero_point().len(), 12);
        assert_eq!(CardSet::ALL.zero_point().points(), 0);
        assert_eq!(CardSet::ALL.points(), 60);
    }

    #[test]
    fn serde_set() {
        let s = CardSet::from_cards([Card::parse("7C").unwrap(), Card::parse("10S").unwrap()]);
        assert_eq!(serde_json::to_string(&s).unwrap(), "[\"7C\",\"10S\"]");
    }
}
