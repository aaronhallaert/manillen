//! ASCII rendering of a player's view and of analysis reports.

use manillen_core::ai::{AnalysisReport, Eval, Unit};
use manillen_core::{Bid, Card, CardSet, DealResult, Move, Player, PlayerView, Suit};

pub fn card(c: Card) -> String {
    format!("{}{}", c.rank().label(), c.suit().symbol())
}

pub fn bid(b: Bid) -> String {
    match b {
        Bid::Trump { suit } => format!("trump {}", suit.symbol()),
        Bid::NoTrump => "no trump".to_string(),
    }
}

pub fn mv(m: Move) -> String {
    match m {
        Move::Bid { bid: b } => format!("bid {}", bid(b)),
        Move::Contra { contra: true } => "contra".to_string(),
        Move::Contra { contra: false } => "no contra".to_string(),
        Move::Play { card: c } => card(c),
    }
}

fn hand(cards: CardSet) -> String {
    let mut out = Vec::new();
    for suit in Suit::ALL {
        let s = cards.of_suit(suit);
        if s.is_empty() {
            continue;
        }
        let ranks: Vec<&str> = s.iter().map(|c| c.rank().label()).collect();
        out.push(format!("{}{}", suit.symbol(), ranks.join(" ")));
    }
    if out.is_empty() {
        "(empty)".to_string()
    } else {
        out.join("  ")
    }
}

fn slots(view: &PlayerView, p: Player) -> String {
    view.table[p.index()]
        .iter()
        .map(|s| match s.top {
            Some(c) => format!("{:>4}{}", card(c), if s.covered { "(+)" } else { "   " }),
            None => format!("{:>4}   ", "--"),
        })
        .collect::<Vec<_>>()
        .join(" ")
}

/// Full table as seen by `view.me`.
pub fn table(view: &PlayerView, scores: [u16; 2], target: u16) -> String {
    let me = view.me;
    let opp = me.other();
    let mut s = String::new();
    s.push_str(&format!(
        "=== Deal: dealer P{}  {}  x{}  |  Points P{me}: {}  P{opp}: {}  |  Match: {}-{} (to {target})\n",
        view.dealer,
        view.bid.map(bid).unwrap_or_else(|| "bidding".to_string()),
        view.multiplier,
        view.points[me.index()],
        view.points[opp.index()],
        scores[me.index()],
        scores[opp.index()],
    ));
    s.push_str(&format!("Opponent (P{opp}) hand: {} cards\n", view.opp_hand_count));
    s.push_str(&format!("Opponent table: {}\n", slots(view, opp)));
    s.push_str("\n");
    match &view.current_trick {
        Some(t) => {
            let who = if t.leader == me { "you" } else { "opponent" };
            s.push_str(&format!("Current trick: {who} led {}\n", t.cards.iter().map(|&c| card(c)).collect::<Vec<_>>().join(" ")));
        }
        None => s.push_str("Current trick: (you lead)\n"),
    }
    match view.last_trick {
        Some(t) => s.push_str(&format!(
            "Last trick:    P{} led {}, P{} played {} -> P{} won ({} pts)\n",
            t.leader,
            card(t.cards[0]),
            t.leader.other(),
            card(t.cards[1]),
            t.winner,
            t.points
        )),
        None => s.push_str("Last trick:    -\n"),
    }
    s.push_str("\n");
    s.push_str(&format!("Your table:     {}\n", slots(view, me)));
    s.push_str(&format!("Your hand (P{me}): {}\n", hand(view.hand)));
    s.push_str(&format!("Tricks played: {}/16\n", view.tricks_played));
    s
}

pub fn deal_result(r: &DealResult) -> String {
    let winner = match r.winner {
        Some(w) => format!("P{w} wins"),
        None => "tie".to_string(),
    };
    format!(
        "Deal over: P0 {} - P1 {} | {} contra={} x{} | {} base {} -> score {}",
        r.points[0],
        r.points[1],
        bid(r.bid),
        r.contra,
        r.multiplier,
        winner,
        r.base,
        r.score
    )
}

pub fn eval_short(e: &Eval, chosen: Move) -> String {
    let unit = match e.unit {
        Unit::Points => "pts",
        Unit::Score => "score",
    };
    match e.value_of(chosen) {
        Some(v) => format!("(expected {v:.1} {unit}, {} samples)", e.samples),
        None => String::new(),
    }
}

pub fn analysis(report: &AnalysisReport) -> String {
    let mut s = String::new();
    s.push_str(&format!("--- Review for P{} ---\n", report.player));
    for d in &report.decisions {
        let unit = match d.unit {
            Unit::Points => "pts",
            Unit::Score => "score",
        };
        let trick = if d.trick < 0 { "   ".to_string() } else { format!("T{:>2}", d.trick + 1) };
        s.push_str(&format!(
            "#{:>2} {trick} {:<14} best {:<14} {:>5.1} vs {:>5.1} {unit}  loss {:>4.1}  {:?}\n",
            d.index,
            mv(d.chosen),
            mv(d.best),
            d.value_chosen,
            d.value_best,
            d.loss,
            d.label
        ));
        if let Some(n) = &d.note {
            s.push_str(&format!("        {n}\n"));
        }
        for r in &d.reasons_best {
            s.push_str(&format!("        + {r}\n"));
        }
        for r in &d.reasons_chosen {
            s.push_str(&format!("        - {r}\n"));
        }
    }
    let sm = &report.summary;
    s.push_str(&format!(
        "Summary: best {} good {} inaccuracy {} mistake {} blunder {} | avg loss {:.2}\n",
        sm.best, sm.good, sm.inaccuracy, sm.mistake, sm.blunder, sm.avg_loss
    ));
    s
}
