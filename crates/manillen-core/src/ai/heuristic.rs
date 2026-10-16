//! Rule-based play used by the easy level and as a fallback when the search budget runs out.
//! Encodes the strategies from strategies.md: unlock own hidden cards early, count what is
//! out, cash masters, force the opponent to trump with worthless cards, and don't feed points.

use crate::card::{Card, Rank, Suit};
use crate::cardset::CardSet;
use crate::rules::{beats, Bid};
use crate::view::PlayerView;
use rand::Rng;

/// Cards the opponent could still hold or reach: their visible table cards plus everything unknown.
fn opponent_pool(view: &PlayerView) -> CardSet {
    view.visible_table(view.me.other()) | view.unknown
}

/// Own face-up cards that still cover a hidden card.
fn flippers(view: &PlayerView) -> CardSet {
    CardSet::from_cards(view.table[view.me.index()].iter().filter(|s| s.covered).filter_map(|s| s.top))
}

fn suit_strength(cards: CardSet, suit: Suit) -> f32 {
    let s = cards.of_suit(suit);
    let mut v = s.len() as f32;
    if s.contains(Card::new(suit, Rank::Ten)) {
        v += 1.5;
    }
    if s.contains(Card::new(suit, Rank::Ace)) {
        v += 1.0;
    }
    if s.contains(Card::new(suit, Rank::King)) {
        v += 0.5;
    }
    v
}

/// Dealer's trump choice: the longest/strongest suit; no trump with many top cards.
pub fn choose_bid(view: &PlayerView) -> Bid {
    let mine = view.available();
    let tens = mine.of_rank(Rank::Ten).len();
    let aces = mine.of_rank(Rank::Ace).len();
    if tens >= 3 || (tens >= 2 && aces >= 2) {
        return Bid::NoTrump;
    }
    let best = Suit::ALL
        .iter()
        .copied()
        .max_by(|&a, &b| suit_strength(mine, a).partial_cmp(&suit_strength(mine, b)).unwrap())
        .unwrap();
    Bid::Trump { suit: best }
}

/// Non-dealer's contra decision: only with clear top strength.
pub fn choose_contra(view: &PlayerView) -> bool {
    let mine = view.available();
    match view.bid {
        Some(Bid::Trump { suit }) => {
            let tops = [Rank::Ten, Rank::Ace, Rank::King].iter().filter(|&&r| mine.contains(Card::new(suit, r))).count();
            tops >= 2 && mine.of_suit(suit).len() >= 3
        }
        Some(Bid::NoTrump) => mine.of_rank(Rank::Ten).len() + mine.of_rank(Rank::Ace).len() >= 4,
        None => false,
    }
}

/// Is `card` guaranteed to win if led now (nothing the opponent may hold beats it)?
fn is_master(card: Card, pool: CardSet, bid: Bid) -> bool {
    if !pool.above(card).is_empty() {
        return false;
    }
    match bid.trump_suit() {
        Some(t) if t != card.suit() => {
            // the opponent can ruff only when void; assume they can if they have no visible card of the suit
            // and there are still unknown trumps
            let opp_may_follow = !pool.of_suit(card.suit()).is_empty();
            opp_may_follow || pool.of_suit(t).is_empty()
        }
        _ => true,
    }
}

fn score_lead(card: Card, view: &PlayerView, bid: Bid) -> f32 {
    let pool = opponent_pool(view);
    let flip = flippers(view).contains(card);
    let pts = card.points() as f32;
    let mut s = 0.0;
    if is_master(card, pool, bid) {
        s += 8.0 + pts; // cash it
    } else {
        s -= pts * 1.5; // don't feed points
    }
    if flip && pts == 0.0 {
        s += 5.0; // free a hidden card cheaply
    } else if flip {
        s += 1.0;
    }
    if let Some(t) = bid.trump_suit() {
        let my_trumps = view.available().of_suit(t);
        let top_trumps = my_trumps.contains(Card::new(t, Rank::Ten)) as u8 + my_trumps.contains(Card::new(t, Rank::Ace)) as u8;
        if card.suit() == t && top_trumps >= 1 && !pool.of_suit(t).is_empty() && pts <= 3.0 {
            s += 2.0; // draw trumps while holding the top ones
        }
        // the opponent has no visible card in this suit but may hold trumps: lead low to make them ruff air
        let opp_visible = view.visible_table(view.me.other()).of_suit(card.suit());
        if card.suit() != t && opp_visible.is_empty() && pts == 0.0 && !pool.of_suit(t).is_empty() {
            s += 1.5;
        }
    }
    s
}

fn score_follow(card: Card, led: Card, view: &PlayerView, bid: Bid) -> f32 {
    let wins = beats(card, led, bid);
    let pts = card.points() as f32;
    let flip = flippers(view).contains(card);
    let mut s = 0.0;
    if wins {
        // winning: bank points, but prefer keeping the biggest cards when a cheaper one wins
        s += 10.0 - card.rank().index() as f32 * 0.3;
        if led.points() >= 3 {
            s += pts * 0.3;
        }
    } else {
        s -= pts * 2.0; // losing: give away as little as possible
    }
    if flip && pts == 0.0 {
        s += 1.5;
    }
    s
}

/// Pick a card by heuristic scoring. With probability `randomness` a random legal card is chosen.
pub fn choose_play<R: Rng + ?Sized>(view: &PlayerView, randomness: f32, rng: &mut R) -> Option<Card> {
    let bid = view.bid?;
    let legal = view.legal_cards();
    if legal.is_empty() {
        return None;
    }
    let cards = legal.to_vec();
    if randomness > 0.0 && rng.gen::<f32>() < randomness {
        return Some(cards[rng.gen_range(0..cards.len())]);
    }
    let led = view.led();
    let scored: Vec<(Card, f32)> = cards
        .iter()
        .map(|&c| {
            let s = match led {
                None => score_lead(c, view, bid),
                Some(l) => score_follow(c, l, view, bid),
            };
            (c, s + rng.gen::<f32>() * 0.01)
        })
        .collect();
    scored.into_iter().max_by(|a, b| a.1.partial_cmp(&b.1).unwrap()).map(|(c, _)| c)
}

/// A short explanation of why a card is a good lead/follow (used by analysis notes).
pub fn explain(card: Card, view: &PlayerView) -> Option<String> {
    let bid = view.bid?;
    let pool = opponent_pool(view);
    let flip = flippers(view).contains(card);
    match view.led() {
        None => {
            if is_master(card, pool, bid) && card.points() > 0 {
                Some(format!("{card} cannot be beaten right now: cash the points."))
            } else if flip && card.points() == 0 {
                Some(format!("Leading the worthless {card} from the table frees your hidden card early."))
            } else if card.points() == 0 && bid.trump_suit().map(|t| !pool.of_suit(t).is_empty() && pool.of_suit(card.suit()).is_empty()).unwrap_or(false) {
                Some(format!("The opponent has nothing visible in this suit: {card} makes them trump a worthless card."))
            } else {
                None
            }
        }
        Some(led) => {
            if beats(card, led, bid) {
                Some(format!("{card} wins the trick."))
            } else if card.points() == 0 {
                Some(format!("The trick is lost: {card} gives away nothing."))
            } else {
                None
            }
        }
    }
}

// ---------------------------------------------------------------------------
// Explanations for the review. Every statement below is derived from the
// player's own view only: their hand, the face-up cards, the cards played so
// far and the set of cards they have not seen. Nothing looks at hidden cards.
// ---------------------------------------------------------------------------

fn pretty(card: Card) -> String {
    format!("{}{}", card.rank().label(), card.suit().symbol())
}

fn list(set: CardSet) -> String {
    let mut v: Vec<Card> = set.to_vec();
    v.sort_by_key(|c| std::cmp::Reverse(c.rank()));
    v.iter().map(|&c| pretty(c)).collect::<Vec<_>>().join(" ")
}

fn suit_name(s: Suit) -> &'static str {
    match s {
        Suit::Clubs => "clubs",
        Suit::Diamonds => "diamonds",
        Suit::Hearts => "hearts",
        Suit::Spades => "spades",
    }
}

/// Where the cards of `set` are, as far as the viewer knows.
fn locate(set: CardSet, view: &PlayerView) -> String {
    let mut parts = Vec::new();
    let played = set & view.played;
    let mine = set & view.hand;
    let my_table = set & view.visible_table(view.me);
    let opp_table = set & view.visible_table(view.me.other());
    let unseen = set & view.unknown;
    if !played.is_empty() {
        parts.push(format!("{} already played", list(played)));
    }
    if !mine.is_empty() {
        parts.push(format!("{} in your hand", list(mine)));
    }
    if !my_table.is_empty() {
        parts.push(format!("{} open on your table", list(my_table)));
    }
    if !opp_table.is_empty() {
        parts.push(format!("{} open on the opponent's table", list(opp_table)));
    }
    if !unseen.is_empty() {
        parts.push(format!("{} unseen (opponent's hand or face down)", list(unseen)));
    }
    parts.join("; ")
}

/// Known facts about the opponent's hand in `suit`, inferred from their earlier forced plays.
fn void_fact(suit: Suit, view: &PlayerView) -> Option<String> {
    let unseen = view.unknown.of_suit(suit);
    if unseen.is_empty() {
        return None;
    }
    let possible_in_hand = unseen - view.opp_hand_forbidden;
    if possible_in_hand.is_empty() {
        Some(format!(
            "The opponent's hand holds no {} (they could not follow it earlier); the unseen {} ({}) can only be face down.",
            suit_name(suit),
            suit_name(suit),
            list(unseen)
        ))
    } else if (unseen & view.opp_hand_forbidden).len() > 0 {
        Some(format!(
            "From earlier play the opponent's hand cannot hold {}; those can only be face down.",
            list(unseen & view.opp_hand_forbidden)
        ))
    } else {
        None
    }
}

/// Reasons why `card` is a good or bad play right now, from `view` only.
pub fn reasons(card: Card, view: &PlayerView) -> Vec<String> {
    let mut out = Vec::new();
    let bid = match view.bid {
        Some(b) => b,
        None => return out,
    };
    let legal = view.legal_cards();
    if legal.len() == 1 && legal.contains(card) {
        out.push("Forced: it was the only legal card.".to_string());
        return out;
    }
    let suit = card.suit();
    let trump = bid.trump_suit();
    let opp_open = view.visible_table(view.me.other());
    let opp_open_higher = opp_open.above(card);
    let unseen_higher = view.unknown.above(card);
    let unseen_higher_in_hand = unseen_higher - view.opp_hand_forbidden;
    let higher_all = CardSet::suit_mask(suit).above(card);
    let opp_open_suit = opp_open.of_suit(suit);
    let unseen_trumps = trump.map(|t| view.unknown.of_suit(t)).unwrap_or(CardSet::EMPTY);
    let open_trumps = trump.map(|t| opp_open.of_suit(t)).unwrap_or(CardSet::EMPTY);
    let flip = flippers(view).contains(card);
    let pts = card.points();

    match view.led() {
        None => {
            // --- leading ---
            if !opp_open_higher.is_empty() {
                out.push(format!(
                    "{} is open on the opponent's table: they are forced to beat {} with it (the rules require beating when possible).",
                    list(opp_open_higher),
                    pretty(card)
                ));
            }
            if !unseen_higher.is_empty() {
                if unseen_higher_in_hand.is_empty() {
                    out.push(format!(
                        "Higher {} not yet seen: {}, but earlier play shows they are not in the opponent's hand, so they must be face down and cannot be played on this trick.",
                        suit_name(suit),
                        list(unseen_higher)
                    ));
                } else {
                    out.push(format!(
                        "Higher {} still unseen: {} (could be in the opponent's hand or face down).",
                        suit_name(suit),
                        list(unseen_higher_in_hand)
                    ));
                }
            }
            if opp_open_higher.is_empty() && unseen_higher_in_hand.is_empty() {
                if higher_all.is_empty() {
                    out.push(format!("{} is the highest {}.", pretty(card), suit_name(suit)));
                } else if unseen_higher.is_empty() {
                    out.push(format!("No higher {} can be played against it: {}.", suit_name(suit), locate(higher_all, view)));
                }
                match trump {
                    Some(t) if t != suit => {
                        if !opp_open_suit.is_empty() {
                            out.push(format!(
                                "The opponent has {} open in {} and must follow suit, so they cannot trump it.",
                                list(opp_open_suit),
                                suit_name(suit)
                            ));
                        } else if unseen_trumps.is_empty() && open_trumps.is_empty() {
                            out.push("No trumps are unseen, so it cannot be ruffed.".to_string());
                        } else {
                            let can_ruff = (unseen_trumps - view.opp_hand_forbidden) | open_trumps;
                            if can_ruff.is_empty() {
                                out.push("The remaining trumps cannot be in the opponent's hand, so it cannot be ruffed.".to_string());
                            } else {
                                out.push(format!(
                                    "Risk: the opponent shows no {}; if their hand is void they must ruff, and they could hold {}.",
                                    suit_name(suit),
                                    list(can_ruff)
                                ));
                            }
                        }
                        if let Some(f) = void_fact(suit, view) {
                            out.push(f);
                        }
                    }
                    _ => {}
                }
                if pts > 0 && opp_open_suit.is_empty() == false || (pts > 0 && unseen_trumps.is_empty() && open_trumps.is_empty()) {
                    out.push(format!("Cashing it banks {pts} point{} safely.", if pts == 1 { "" } else { "s" }));
                }
            } else if pts > 0 {
                out.push(format!("Leading {} risks giving away its {pts} points.", pretty(card)));
            }
            if flip && pts == 0 {
                out.push("Playing it from the table turns up the hidden card beneath, which gives you more options.".to_string());
            } else if flip {
                out.push("Playing it from the table turns up the hidden card beneath.".to_string());
            }
            if pts == 0 && (!unseen_higher_in_hand.is_empty() || !opp_open_higher.is_empty()) {
                out.push("It gives away nothing if the opponent wins the trick.".to_string());
            }
            if let Some(t) = trump {
                if suit == t {
                    let my_trumps = view.available().of_suit(t);
                    let others = unseen_trumps | open_trumps;
                    if !others.is_empty() {
                        out.push(format!("Leading trumps: you hold {}; trumps not in your hand: {}.", list(my_trumps), list(others)));
                    } else {
                        out.push("No other trumps remain; leading a trump cannot draw anything.".to_string());
                    }
                } else if opp_open_suit.is_empty() && pts == 0 && !(unseen_trumps - view.opp_hand_forbidden).is_empty() {
                    out.push(format!(
                        "The opponent shows no {}: if their hand is void they must spend a trump on a worthless card.",
                        suit_name(suit)
                    ));
                    if let Some(f) = void_fact(suit, view) {
                        out.push(f);
                    }
                }
            }
        }
        Some(led) => {
            // --- following ---
            if beats(card, led, bid) {
                out.push(format!("{} wins the trick against {}.", pretty(card), pretty(led)));
                let cheaper = legal.iter().filter(|&c| beats(c, led, bid) && c.rank() < card.rank() && c.suit() == card.suit()).count();
                if cheaper == 0 && legal.iter().filter(|&c| beats(c, led, bid)).count() > 1 {
                    out.push("It is the cheapest card that wins, keeping your higher cards for later.".to_string());
                }
                let worth = pts + led.points();
                if worth > 0 {
                    out.push(format!("The trick is worth {worth} point{}.", if worth == 1 { "" } else { "s" }));
                }
            } else {
                let same = view.available().of_suit(led.suit());
                if !same.is_empty() && same.above(led).is_empty() {
                    out.push(format!("You must follow {} and hold nothing above {}.", suit_name(led.suit()), pretty(led)));
                } else if same.is_empty() {
                    match trump {
                        Some(t) if !view.available().of_suit(t).is_empty() && led.suit() != t => {}
                        Some(_) => out.push(format!("You have no {} and no trump, so the trick is lost.", suit_name(led.suit()))),
                        None => out.push(format!("You have no {}; without trumps the trick is lost.", suit_name(led.suit()))),
                    }
                }
                if pts == 0 {
                    out.push("Discarding a worthless card gives the opponent nothing extra.".to_string());
                } else {
                    out.push(format!("This hands the opponent {pts} extra points on a lost trick."));
                }
            }
            if flip && pts == 0 {
                out.push("Playing it from the table turns up the hidden card beneath.".to_string());
            }
        }
    }
    out
}
