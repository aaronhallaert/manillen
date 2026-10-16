//! Sample a complete world (all hidden cards placed) consistent with a player's view.

use crate::card::{Card, Player};
use crate::cardset::CardSet;
use crate::deal::{Layout, Position, SlotPair};
use crate::rules::Bid;
use crate::view::PlayerView;
use rand::seq::SliceRandom;
use rand::Rng;

/// Draw `n` distinct cards uniformly from `pool`.
fn draw<R: Rng + ?Sized>(pool: CardSet, n: usize, rng: &mut R) -> CardSet {
    let mut v = pool.to_vec();
    v.shuffle(rng);
    CardSet::from_cards(v.into_iter().take(n))
}

/// Sample a perfect-information position consistent with `view`.
///
/// Unknown cards are split into the opponent's hand (uniformly among the cards not
/// forbidden by inference) and the covered table slots. Any surplus unknown cards (cards
/// the viewer has forgotten) are left out of play. `bid` overrides the view's bid, which
/// lets the bidder evaluate hypothetical trump choices.
pub fn sample_world<R: Rng + ?Sized>(view: &PlayerView, bid: Bid, rng: &mut R) -> Position {
    let me = view.me;
    let opp = me.other();
    let n_opp = view.opp_hand_count as usize;

    let mut pool = view.unknown - view.opp_hand_forbidden;
    if pool.len() < n_opp as u32 {
        pool = view.unknown; // inconsistent constraints (should not happen); ignore them
    }
    let opp_hand = draw(pool, n_opp, rng);
    let mut rest = (view.unknown - opp_hand).to_vec();
    rest.shuffle(rng);
    let mut rest = rest.into_iter();

    let mut hands = [CardSet::EMPTY; 2];
    hands[me.index()] = view.hand;
    hands[opp.index()] = opp_hand;
    let mut remaining = hands;

    // placeholder for played cards: any card nobody still holds
    let mut table = [[SlotPair { top: Card(0), under: Card(0) }; 4]; 2];
    let mut unders = [CardSet::EMPTY; 2];
    for p in [Player::P0, Player::P1] {
        for (i, slot) in view.table[p.index()].iter().enumerate() {
            match (slot.top, slot.covered) {
                (Some(top), true) => {
                    let under = rest.next().expect("enough unknown cards for covered slots");
                    table[p.index()][i] = SlotPair { top, under };
                    remaining[p.index()].insert(top);
                    remaining[p.index()].insert(under);
                    unders[p.index()].insert(under);
                }
                (Some(c), false) => {
                    table[p.index()][i] = SlotPair { top: Card(0), under: c };
                    remaining[p.index()].insert(c);
                }
                (None, _) => {}
            }
        }
    }
    let unplayed = remaining[0] | remaining[1];
    let dummy = (CardSet::ALL - unplayed).lowest().unwrap_or(Card(0));
    for p in 0..2 {
        for i in 0..4 {
            let s = &mut table[p][i];
            let visible = view.table[p][i];
            match (visible.top, visible.covered) {
                (Some(_), true) => {}
                (Some(_), false) => s.top = dummy,
                (None, _) => {
                    s.top = dummy;
                    s.under = dummy;
                }
            }
        }
    }

    let (leader, led) = match &view.current_trick {
        Some(t) => (t.leader, t.cards.first().copied()),
        None => (view.last_trick.map(|t| t.winner).unwrap_or(view.dealer.other()), None),
    };
    let layout = Layout { hands, table };
    Position::from_world(layout, bid, remaining, leader, led, view.points, view.tricks_played)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::deal::Deal;
    use crate::rules::Move;
    use rand::rngs::SmallRng;
    use rand::SeedableRng;

    #[test]
    fn sampled_worlds_are_consistent_with_the_view() {
        for seed in 0..40u64 {
            let mut rng = SmallRng::seed_from_u64(seed);
            let mut deal = Deal::random(&mut rng, Player::P1);
            let bid = Bid::Trump { suit: crate::card::Suit::Spades };
            deal.apply(Move::Bid { bid }).unwrap();
            deal.apply(Move::Contra { contra: false }).unwrap();
            for step in 0..32 {
                let me = deal.to_move().unwrap();
                let view = PlayerView::of(&deal, me);
                for _ in 0..3 {
                    let w = sample_world(&view, bid, &mut rng);
                    assert_eq!(w.to_move(), me, "seed {seed} step {step}");
                    assert_eq!(w.available(me) & view.hand, view.hand);
                    assert_eq!(w.legal(), view.legal_cards(), "seed {seed} step {step}");
                    assert_eq!(w.remaining[me.other().index()].len(), deal.position().remaining[me.other().index()].len());
                    assert!((w.remaining[0] & w.remaining[1]).is_empty());
                    assert_eq!(w.unplayed() | view.played, CardSet::ALL);
                    assert!((w.remaining[me.other().index()] & view.opp_hand_forbidden & view.hand).is_empty());
                    // opponent hand never contains forbidden cards
                    let opp_hand = w.layout.hands[me.other().index()];
                    assert!((opp_hand & view.opp_hand_forbidden).is_empty());
                    assert_eq!(w.remaining_points() as u32, 60 - view.points[0] as u32 - view.points[1] as u32 - view.led().map(|c| c.points() as u32).unwrap_or(0));
                    assert_eq!(w.leader, deal.position().leader);
                    assert_eq!(w.led, deal.position().led);
                }
                let legal = deal.legal_moves();
                deal.apply(legal[rng.gen_range(0..legal.len())]).unwrap();
            }
        }
    }

    #[test]
    fn true_world_is_in_support() {
        // Every unknown card must appear in the opponent's hand in some sample (coverage).
        let mut rng = SmallRng::seed_from_u64(5);
        let deal = Deal::random(&mut rng, Player::P0);
        let view = PlayerView::of(&deal, Player::P0);
        let mut seen = CardSet::EMPTY;
        for _ in 0..300 {
            let w = sample_world(&view, Bid::NoTrump, &mut rng);
            seen = seen | w.layout.hands[1];
        }
        assert_eq!(seen, view.unknown);
    }
}
