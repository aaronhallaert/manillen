//! Solver and search timing.

use manillen_core::ai::{Ai, Budget, Horizon, Level, Solver};
use manillen_core::{Bid, Deal, Move, Player, PlayerView, Position, Suit};
use rand::rngs::SmallRng;
use rand::{Rng, SeedableRng};
use std::time::Instant;

fn random_deal_at(rng: &mut SmallRng, seed: u64, tricks_played: u8) -> Deal {
    let bid = if seed % 5 == 4 { Bid::NoTrump } else { Bid::Trump { suit: Suit::from_index(seed as usize % 4) } };
    let mut deal = Deal::random(rng, Player::P0);
    deal.apply(Move::Bid { bid }).expect("bid");
    deal.apply(Move::Contra { contra: false }).expect("contra");
    while deal.tricks().len() < tricks_played as usize {
        let legal = deal.legal_moves();
        deal.apply(legal[rng.gen_range(0..legal.len())]).expect("play");
    }
    deal
}

pub fn run(deals: u32, nodes: u64) -> Result<(), Box<dyn std::error::Error>> {
    let mut solver = Solver::new(22);
    println!("Exact solves ({deals} random positions each):");
    println!("{:>8} {:>14} {:>10} {:>8}", "tricks", "avg nodes", "avg ms", "Mn/s");
    for &remaining in &[16u8, 14, 12, 10, 8, 6] {
        let mut total_nodes = 0u64;
        let mut total_time = 0f64;
        for d in 0..deals {
            let seed = 1000 + d as u64;
            let mut rng = SmallRng::seed_from_u64(seed);
            let deal = random_deal_at(&mut rng, seed, 16 - remaining);
            let pos: Position = deal.position().clone();
            solver.new_world();
            let before = solver.nodes;
            let t = Instant::now();
            solver.value(&pos, Budget::UNLIMITED, Horizon::FULL).expect("unlimited");
            total_time += t.elapsed().as_secs_f64();
            total_nodes += solver.nodes - before;
        }
        let avg_nodes = total_nodes / deals as u64;
        let avg_ms = total_time * 1000.0 / deals as f64;
        println!("{:>8} {:>14} {:>10.1} {:>8.1}", remaining, avg_nodes, avg_ms, total_nodes as f64 / total_time.max(1e-9) / 1e6);
    }

    println!("\nPIMC evaluate (hard preset, budget {nodes} nodes/decision):");
    let mut ai = Ai::new(3, 22);
    for &tricks_played in &[0u8, 5] {
        let mut total_time = 0f64;
        let mut total_nodes = 0u64;
        let mut samples = 0u64;
        let mut aborted = 0u32;
        for d in 0..deals {
            let seed = 2000 + d as u64;
            let mut rng = SmallRng::seed_from_u64(seed);
            let deal = random_deal_at(&mut rng, seed, tricks_played);
            let p = deal.to_move().expect("in progress");
            let view = PlayerView::of(&deal, p);
            let t = Instant::now();
            match ai.evaluate(&view, Level::Hard, nodes) {
                Some(e) => {
                    samples += e.samples as u64;
                    total_nodes += e.nodes;
                }
                None => aborted += 1,
            }
            total_time += t.elapsed().as_secs_f64();
        }
        println!(
            "  trick {:>2}: avg {:>7.1} ms, avg {:>9} nodes, avg {:.1} samples, {} aborted",
            tricks_played + 1,
            total_time * 1000.0 / deals as f64,
            total_nodes / deals as u64,
            samples as f64 / deals as f64,
            aborted
        );
    }
    Ok(())
}
