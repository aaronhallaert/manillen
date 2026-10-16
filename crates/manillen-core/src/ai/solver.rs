//! Perfect-information solver for a determinized deal.
//!
//! Minimax with alpha-beta over card points. Within a trick the value is always expressed
//! for the leader of that trick ("V"): the leader maximizes, the follower minimizes.
//! At trick boundaries values are stored in a transposition table keyed by the set of
//! unplayed cards and the leader (ownership is fixed by the layout, so that identifies the
//! position). Values exclude points already scored, which makes the table history-independent.

use crate::card::{Card, Player, Suit};
use crate::cardset::CardSet;
use crate::deal::Position;

/// Search budget. Node counts rather than wall-clock so it behaves the same in WebAssembly.
#[derive(Clone, Copy, Debug)]
pub struct Budget {
    pub max_nodes: u64,
}

impl Budget {
    pub const UNLIMITED: Budget = Budget { max_nodes: u64::MAX };
    pub fn nodes(max_nodes: u64) -> Budget {
        Budget { max_nodes }
    }
}

/// Depth control: search exactly when few tricks remain, otherwise cut off after `depth`
/// tricks and estimate the rest heuristically.
#[derive(Clone, Copy, Debug)]
pub struct Horizon {
    /// Search to the end whenever at most this many tricks remain at the root.
    pub exact_when_remaining: u8,
    /// Otherwise, number of tricks to search from the root before evaluating.
    pub depth: u8,
}

impl Horizon {
    pub const FULL: Horizon = Horizon { exact_when_remaining: 16, depth: 16 };
}

#[derive(Clone, Copy, Default)]
struct Entry {
    key: u64,
    gen: u16,
    lo: i8,
    hi: i8,
    best: u8,
}

const NO_MOVE: u8 = 0xFF;

pub struct Solver {
    tt: Vec<Entry>,
    mask: usize,
    gen: u16,
    pub nodes: u64,
    max_nodes: u64,
    aborted: bool,
    /// Absolute trick count at which the search is cut off (17 = never).
    cutoff_at: u8,
}

struct Undo {
    card: Card,
    player: Player,
    leader: Player,
    led: Option<Card>,
    points: [u8; 2],
    tricks_played: u8,
}

/// Points still to be decided, including the card already led in the current trick.
#[inline]
fn points_at_stake(pos: &Position) -> i32 {
    pos.remaining_points() as i32 + pos.led.map(|c| c.points() as i32).unwrap_or(0)
}

#[inline]
fn play(pos: &mut Position, card: Card) -> (Undo, Option<(Player, u8)>) {
    let undo = Undo { card, player: pos.to_move(), leader: pos.leader, led: pos.led, points: pos.points, tricks_played: pos.tricks_played };
    let trick = pos.play(card).map(|t| (t.winner, t.points));
    (undo, trick)
}

#[inline]
fn unplay(pos: &mut Position, u: Undo) {
    pos.remaining[u.player.index()].insert(u.card);
    pos.leader = u.leader;
    pos.led = u.led;
    pos.points = u.points;
    pos.tricks_played = u.tricks_played;
}

impl Solver {
    /// `tt_bits` = log2 of the number of table entries (20 → ~16 MB).
    pub fn new(tt_bits: u32) -> Solver {
        let size = 1usize << tt_bits;
        Solver { tt: vec![Entry::default(); size], mask: size - 1, gen: 1, nodes: 0, max_nodes: u64::MAX, aborted: false, cutoff_at: 17 }
    }

    /// Forget all stored positions (call when the world/layout changes).
    pub fn new_world(&mut self) {
        self.gen = self.gen.wrapping_add(1);
        if self.gen == 0 {
            for e in self.tt.iter_mut() {
                *e = Entry::default();
            }
            self.gen = 1;
        }
    }

    pub fn aborted(&self) -> bool {
        self.aborted
    }

    fn start(&mut self, pos: &Position, budget: Budget, horizon: Horizon) {
        self.max_nodes = self.nodes.saturating_add(budget.max_nodes);
        self.aborted = false;
        let remaining = 16 - pos.tricks_played;
        self.cutoff_at = if remaining <= horizon.exact_when_remaining { 17 } else { pos.tricks_played + horizon.depth };
    }

    /// Value of every legal card for the player to move (points that player still collects),
    /// or `None` if the budget ran out.
    pub fn evaluate_moves(&mut self, pos: &Position, budget: Budget, horizon: Horizon) -> Option<Vec<(Card, i32)>> {
        self.start(pos, budget, horizon);
        let mut pos = pos.clone();
        let legal = pos.legal();
        let r = points_at_stake(&pos);
        let mut out = Vec::with_capacity(legal.len() as usize);
        if pos.led.is_none() {
            for card in legal.iter() {
                let (u, _) = play(&mut pos, card);
                let v = self.follow(&mut pos, 0, r);
                unplay(&mut pos, u);
                if self.aborted {
                    return None;
                }
                out.push((card, v));
            }
        } else {
            for card in legal.iter() {
                let (u, trick) = play(&mut pos, card);
                let (winner, tp) = trick.expect("follow completes the trick");
                let tp = tp as i32;
                let v_leader = if winner == u.leader { tp + self.lead(&mut pos, -tp, r - tp) } else { (r - tp) - self.lead(&mut pos, 0, r - tp) };
                unplay(&mut pos, u);
                if self.aborted {
                    return None;
                }
                out.push((card, r - v_leader));
            }
        }
        Some(out)
    }

    /// Points the player to move still collects with best play from both sides.
    pub fn value(&mut self, pos: &Position, budget: Budget, horizon: Horizon) -> Option<i32> {
        self.start(pos, budget, horizon);
        let mut pos = pos.clone();
        let r = points_at_stake(&pos);
        let v = if pos.led.is_none() { self.lead(&mut pos, 0, r) } else { r - self.follow(&mut pos, 0, r) };
        if self.aborted {
            None
        } else {
            Some(v)
        }
    }

    /// Is `v` at least `bound` for the player to move? Zero-window probe (cheaper than `value`).
    pub fn at_least(&mut self, pos: &Position, bound: i32, budget: Budget, horizon: Horizon) -> Option<bool> {
        self.start(pos, budget, horizon);
        let mut pos = pos.clone();
        let r = points_at_stake(&pos);
        let v = if pos.led.is_none() { self.lead(&mut pos, bound - 1, bound) } else { r - self.follow(&mut pos, r - bound, r - bound + 1) };
        if self.aborted {
            None
        } else {
            Some(v >= bound)
        }
    }

    #[inline]
    fn tt_index(key: u64, mask: usize) -> usize {
        (key.wrapping_mul(0x9E37_79B9_7F4A_7C15) >> 32) as usize & mask
    }

    /// V for `pos.leader` at a trick boundary.
    fn lead(&mut self, pos: &mut Position, mut alpha: i32, mut beta: i32) -> i32 {
        if pos.tricks_played >= 16 {
            return 0;
        }
        self.nodes += 1;
        if self.nodes > self.max_nodes {
            self.aborted = true;
            return 0;
        }
        if pos.tricks_played >= self.cutoff_at {
            return estimate(pos);
        }
        let r = pos.remaining_points() as i32;
        if alpha < 0 {
            alpha = 0;
        }
        if beta > r {
            beta = r;
        }
        if alpha >= beta {
            return alpha;
        }
        let key = pos.unplayed().0 as u64 | ((pos.leader.0 as u64) << 32);
        let idx = Solver::tt_index(key, self.mask);
        let mut tt_move = NO_MOVE;
        {
            let e = self.tt[idx];
            if e.gen == self.gen && e.key == key {
                let (lo, hi) = (e.lo as i32, e.hi as i32);
                if lo >= beta {
                    return lo;
                }
                if hi <= alpha {
                    return hi;
                }
                if lo == hi {
                    return lo;
                }
                if lo > alpha {
                    alpha = lo;
                }
                if hi < beta {
                    beta = hi;
                }
                tt_move = e.best;
            }
        }
        let orig_alpha = alpha;
        let me = pos.leader;
        let opp_unplayed = pos.remaining[me.other().index()];
        let legal = reduce_equivalent(pos.legal(), opp_unplayed, flippers(pos, me));
        let mut moves = [Card(0); 16];
        let n = order_leads(legal, opp_unplayed, pos, tt_move, &mut moves);
        let mut best = i32::MIN;
        let mut best_card = NO_MOVE;
        for &card in &moves[..n] {
            let (u, _) = play(pos, card);
            let v = self.follow(pos, alpha, beta);
            unplay(pos, u);
            if self.aborted {
                return 0;
            }
            if v > best {
                best = v;
                best_card = card.0;
            }
            if best > alpha {
                alpha = best;
            }
            if alpha >= beta {
                break;
            }
        }
        let e = &mut self.tt[idx];
        let replace = e.gen != self.gen || e.key != key;
        if replace {
            *e = Entry { key, gen: self.gen, lo: 0, hi: r as i8, best: NO_MOVE };
        }
        if best <= orig_alpha {
            if (best as i8) < e.hi {
                e.hi = best as i8;
            }
        } else if best >= beta {
            if (best as i8) > e.lo {
                e.lo = best as i8;
            }
        } else {
            e.lo = best as i8;
            e.hi = best as i8;
        }
        if best_card != NO_MOVE {
            e.best = best_card;
        }
        best
    }

    /// V for `pos.leader` when the follower is to move. The follower minimizes.
    fn follow(&mut self, pos: &mut Position, alpha: i32, mut beta: i32) -> i32 {
        self.nodes += 1;
        if self.nodes > self.max_nodes {
            self.aborted = true;
            return 0;
        }
        let r = points_at_stake(pos);
        let led = pos.led.expect("follower to move");
        let me = pos.to_move();
        let opp_unplayed = pos.remaining[me.other().index()];
        let legal = reduce_equivalent(pos.legal(), opp_unplayed, flippers(pos, me));
        let mut moves = [Card(0); 16];
        let n = order_follows(legal, led, pos, &mut moves);
        let mut best = i32::MAX;
        for &card in &moves[..n] {
            let (u, trick) = play(pos, card);
            let (winner, tp) = trick.expect("trick completes");
            let tp = tp as i32;
            let v = if winner == u.leader {
                tp + self.lead(pos, alpha - tp, beta - tp)
            } else {
                let rest = r - tp;
                rest - self.lead(pos, rest - beta, rest - alpha)
            };
            unplay(pos, u);
            if self.aborted {
                return 0;
            }
            if v < best {
                best = v;
            }
            if best < beta {
                beta = best;
            }
            if alpha >= beta {
                break;
            }
        }
        best
    }
}

/// Cards of `p` whose play would flip a hidden card (face-up tops of covered slots).
#[inline]
fn flippers(pos: &Position, p: Player) -> CardSet {
    let rem = pos.remaining[p.index()];
    let mut f = CardSet::EMPTY;
    for slot in &pos.layout.table[p.index()] {
        if rem.contains(slot.top) && rem.contains(slot.under) {
            f.insert(slot.top);
        }
    }
    f
}

/// Drop zero-point cards that are interchangeable with a lower one of the same suit
/// (no opponent card ranks between them). Cards that would flip a hidden card are never
/// merged, since playing them has a side effect.
#[inline]
fn reduce_equivalent(legal: CardSet, opp_unplayed: CardSet, flippers: CardSet) -> CardSet {
    let zeros = legal.zero_point() - flippers;
    if zeros.len() < 2 {
        return legal;
    }
    let mut out = legal;
    for suit in Suit::ALL {
        let z = zeros.of_suit(suit);
        if z.len() < 2 {
            continue;
        }
        let mut prev: Option<Card> = None;
        for c in z.iter() {
            if let Some(p) = prev {
                let between = opp_unplayed.above(p).below(c);
                if between.is_empty() {
                    out.remove(c);
                    continue;
                }
            }
            prev = Some(c);
        }
    }
    out
}

/// Does `card` currently win against anything the opponent could play on it?
#[inline]
fn is_master(card: Card, opp_unplayed: CardSet, pos: &Position) -> bool {
    if !opp_unplayed.above(card).is_empty() {
        return false;
    }
    match pos.bid.trump_suit() {
        Some(t) if t != card.suit() => {
            // opponent can ruff only when void in the suit
            let opp_void = opp_unplayed.of_suit(card.suit()).is_empty();
            !opp_void || opp_unplayed.of_suit(t).is_empty()
        }
        _ => true,
    }
}

fn order_leads(legal: CardSet, opp_unplayed: CardSet, pos: &Position, tt_move: u8, out: &mut [Card; 16]) -> usize {
    let mut n = 0;
    let mut rest = legal;
    if tt_move != NO_MOVE {
        let c = Card(tt_move);
        if legal.contains(c) {
            out[n] = c;
            n += 1;
            rest.remove(c);
        }
    }
    // masters with the most points first
    let mut masters: Vec<Card> = rest.iter().filter(|&c| is_master(c, opp_unplayed, pos)).collect();
    masters.sort_by_key(|c| std::cmp::Reverse(c.points()));
    for c in masters {
        out[n] = c;
        n += 1;
        rest.remove(c);
    }
    // then cheap cards, then the rest by ascending points
    let mut others: Vec<Card> = rest.iter().collect();
    others.sort_by_key(|c| (c.points(), c.rank()));
    for c in others {
        out[n] = c;
        n += 1;
    }
    n
}

fn order_follows(legal: CardSet, led: Card, pos: &Position, out: &mut [Card; 16]) -> usize {
    let mut n = 0;
    let mut winners: Vec<Card> = legal.iter().filter(|&c| crate::rules::beats(c, led, pos.bid)).collect();
    winners.sort_by_key(|c| (c.points(), c.rank()));
    let mut losers: Vec<Card> = legal.iter().filter(|&c| !crate::rules::beats(c, led, pos.bid)).collect();
    losers.sort_by_key(|c| (c.points(), c.rank()));
    for c in winners.into_iter().chain(losers) {
        out[n] = c;
        n += 1;
    }
    n
}

/// Heuristic estimate of V (points the leader will still collect) at a cut-off node.
pub fn estimate(pos: &Position) -> i32 {
    let r = pos.remaining_points() as i32;
    if r == 0 {
        return 0;
    }
    let me = pos.leader;
    let opp = me.other();
    let mine = pos.remaining[me.index()];
    let theirs = pos.remaining[opp.index()];
    let my_masters = mine.iter().filter(|&c| is_master(c, theirs, pos)).map(|c| c.points() as i32).sum::<i32>();
    let their_masters = theirs.iter().filter(|&c| is_master(c, mine, pos)).map(|c| c.points() as i32).sum::<i32>();
    let contested = (r - my_masters - their_masters).max(0);
    let mut share = 0.5f32;
    if let Some(t) = pos.bid.trump_suit() {
        let d = mine.of_suit(t).len() as i32 - theirs.of_suit(t).len() as i32;
        share += 0.06 * d as f32;
    }
    share += 0.05; // being on lead is worth a little
    let share = share.clamp(0.15, 0.85);
    (my_masters as f32 + contested as f32 * share).round() as i32
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::card::Player;
    use crate::deal::{Deal, Layout};
    use crate::rules::{Bid, Move};
    use rand::rngs::SmallRng;
    use rand::{Rng, SeedableRng};

    /// Plain minimax without pruning, tables or equivalence: points the player to move collects.
    fn naive(pos: &Position) -> i32 {
        if pos.is_over() {
            return 0;
        }
        let me = pos.to_move();
        let mut best = i32::MIN;
        for card in pos.legal().iter() {
            let mut p = pos.clone();
            let before = p.points[me.index()] as i32;
            let trick = p.play(card);
            let gained = p.points[me.index()] as i32 - before;
            let v = if trick.is_some() {
                // next mover is the trick winner
                if p.to_move() == me {
                    gained + naive(&p)
                } else {
                    gained + (p.remaining_points() as i32 - naive(&p))
                }
            } else {
                // follower to move; the led card's points are still at stake
                let rest = p.remaining_points() as i32 + card.points() as i32;
                rest - naive(&p)
            };
            best = best.max(v);
        }
        best
    }

    fn random_endgame(seed: u64, tricks_left: u8) -> Position {
        let mut rng = SmallRng::seed_from_u64(seed);
        let mut deal = Deal::random(&mut rng, Player::P0);
        let bid = if seed % 5 == 4 { Bid::NoTrump } else { Bid::Trump { suit: Suit::from_index(seed as usize % 4) } };
        deal.apply(Move::Bid { bid }).unwrap();
        deal.apply(Move::Contra { contra: false }).unwrap();
        while deal.tricks().len() < (16 - tricks_left) as usize {
            let legal = deal.legal_moves();
            deal.apply(legal[rng.gen_range(0..legal.len())]).unwrap();
        }
        deal.position().clone()
    }

    #[test]
    fn solver_matches_naive_minimax_on_endgames() {
        let mut solver = Solver::new(16);
        for seed in 0..60u64 {
            let tricks_left = 3 + (seed % 4) as u8; // 3..6
            let mut pos = random_endgame(seed, tricks_left);
            // sometimes test mid-trick positions too
            if seed % 3 == 0 {
                let legal = pos.legal();
                pos.play(legal.lowest().unwrap());
            }
            solver.new_world();
            let expected = naive(&pos);
            let got = solver.value(&pos, Budget::UNLIMITED, Horizon::FULL).unwrap();
            assert_eq!(got, expected, "seed {seed}");
            let moves = solver.evaluate_moves(&pos, Budget::UNLIMITED, Horizon::FULL).unwrap();
            let best = moves.iter().map(|&(_, v)| v).max().unwrap();
            assert_eq!(best, expected, "seed {seed} root moves {moves:?}");
            // zero-sum: value for the mover + value for the opponent == remaining points
            let r = points_at_stake(&pos);
            for &(card, v) in &moves {
                let mut p = pos.clone();
                p.play(card);
                let opp_v = if p.to_move() == pos.to_move() {
                    // we won the trick and lead again: opponent's value is r - gained - our continuation
                    let gained = p.points[pos.to_move().index()] as i32 - pos.points[pos.to_move().index()] as i32;
                    r - gained - solver.value(&p, Budget::UNLIMITED, Horizon::FULL).unwrap()
                } else {
                    let gained_by_opp = p.points[pos.to_move().other().index()] as i32 - pos.points[pos.to_move().other().index()] as i32;
                    gained_by_opp + solver.value(&p, Budget::UNLIMITED, Horizon::FULL).unwrap()
                };
                assert_eq!(v + opp_v, r, "seed {seed} card {card}");
            }
        }
    }

    #[test]
    fn budget_abort_returns_none() {
        let pos = random_endgame(1, 12);
        let mut solver = Solver::new(16);
        assert!(solver.value(&pos, Budget::nodes(10), Horizon::FULL).is_none());
        assert!(solver.aborted());
        solver.new_world();
        assert!(solver.value(&pos, Budget::nodes(10_000_000), Horizon { exact_when_remaining: 4, depth: 3 }).is_some());
    }

    #[test]
    fn at_least_agrees_with_value() {
        let mut solver = Solver::new(16);
        for seed in 100..120u64 {
            let pos = random_endgame(seed, 5);
            solver.new_world();
            let v = solver.value(&pos, Budget::UNLIMITED, Horizon::FULL).unwrap();
            assert!(solver.at_least(&pos, v, Budget::UNLIMITED, Horizon::FULL).unwrap());
            assert!(!solver.at_least(&pos, v + 1, Budget::UNLIMITED, Horizon::FULL).unwrap());
        }
    }

    #[test]
    fn full_deal_solve_from_trick_one() {
        let mut rng = SmallRng::seed_from_u64(2024);
        let layout = Layout::deal(&mut rng);
        let pos = Position::new(layout, Bid::Trump { suit: Suit::Hearts }, Player::P1);
        let mut solver = Solver::new(20);
        let v = solver.value(&pos, Budget::nodes(200_000_000), Horizon::FULL);
        assert!(v.is_some(), "full solve exceeded node budget");
        let v = v.unwrap();
        assert!((0..=60).contains(&v));
        eprintln!("full solve value {v} nodes {}", solver.nodes);
    }
}

#[cfg(test)]
mod bench {
    use super::*;
    use crate::card::Player;
    use crate::deal::Layout;
    use crate::rules::Bid;
    use rand::rngs::SmallRng;
    use rand::SeedableRng;

    #[test]
    #[ignore]
    fn bench_full_solves() {
        let mut solver = Solver::new(22);
        let mut total_nodes = 0u64;
        let start = std::time::Instant::now();
        let n = 10;
        for seed in 0..n {
            let mut rng = SmallRng::seed_from_u64(seed);
            let layout = Layout::deal(&mut rng);
            let bid = if seed % 5 == 4 { Bid::NoTrump } else { Bid::Trump { suit: Suit::from_index(seed as usize % 4) } };
            let pos = Position::new(layout, bid, Player::P1);
            solver.new_world();
            let before = solver.nodes;
            let t = std::time::Instant::now();
            let v = solver.value(&pos, Budget::UNLIMITED, Horizon::FULL).unwrap();
            let nodes = solver.nodes - before;
            total_nodes += nodes;
            eprintln!("seed {seed} {bid}: value {v} nodes {nodes} in {:?}", t.elapsed());
        }
        let el = start.elapsed();
        eprintln!("avg nodes {} avg time {:?} rate {:.1} Mn/s", total_nodes / n, el / n as u32, total_nodes as f64 / el.as_secs_f64() / 1e6);
    }
}
