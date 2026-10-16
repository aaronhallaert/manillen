//! Perfect-information par of a saved deal record.

use manillen_core::ai::{par_analysis, Budget, Solver};
use manillen_core::{Bid, GameRecord, Move, Player};
use std::path::Path;

pub fn run(path: &Path, nodes: u64) -> Result<(), Box<dyn std::error::Error>> {
    let json = std::fs::read_to_string(path)?;
    let record: GameRecord = serde_json::from_str(&json)?;
    let bid = record.moves.first().and_then(|m| match m {
        Move::Bid { bid } => Some(*bid),
        _ => None,
    });
    let contra = record.moves.get(1).and_then(|m| match m {
        Move::Contra { contra } => Some(*contra),
        _ => None,
    });
    let mut solver = Solver::new(22);
    let t = std::time::Instant::now();
    let r = par_analysis(&mut solver, &record.layout, record.dealer, bid, contra, Budget::nodes(nodes)).ok_or("par analysis exceeded the node budget")?;
    println!("Par for dealer P{} (both players see all cards, perfect play, non-dealer leads):", r.dealer);
    println!("{:<12} {:>6} {:>6} {:>7} {:>12}", "bid", "P0", "P1", "contra", "dealer score");
    for b in &r.bids {
        let mark = if b.bid == r.best_bid { " <- best" } else { "" };
        println!("{:<12} {:>6} {:>6} {:>7} {:>+12}{mark}", b.bid.to_string(), b.points[0], b.points[1], if b.contra { "yes" } else { "no" }, b.dealer_score);
    }
    println!("Par score for the dealer: {:+} with {}", r.par_score, r.best_bid);
    if let Some(a) = &r.actual {
        let _: Bid = a.bid;
        println!(
            "Actual bid {} {}: perfect play gives P0 {} / P1 {} points, dealer score {:+}",
            a.bid,
            if a.contra { "with contra" } else { "without contra" },
            a.points[0],
            a.points[1],
            a.dealer_score
        );
    }
    let _: Player = r.dealer;
    eprintln!("({} nodes in {:?})", r.nodes, t.elapsed());
    Ok(())
}
