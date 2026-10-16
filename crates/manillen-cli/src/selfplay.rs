//! Level-vs-level self-play over independent deals.

use manillen_core::ai::{Ai, Level};
use manillen_core::{Deal, Player, PlayerView};
use rand::rngs::SmallRng;
use rand::SeedableRng;
use std::time::Instant;

#[derive(Default)]
struct Stats {
    wins: u32,
    ties: u32,
    points: u64,
    score: i64,
}

pub fn run(a: Level, b: Level, games: u32, seed: Option<u64>, nodes: u64) -> Result<(), Box<dyn std::error::Error>> {
    let seed = crate::seed_or_time(seed);
    let mut rng = SmallRng::seed_from_u64(seed);
    let mut ai = Ai::new(seed ^ 0x5151, 20);
    let mut stats = [Stats::default(), Stats::default()]; // [a, b]
    let start = Instant::now();
    let nodes_start = ai.solver.nodes;
    for g in 0..games {
        let seat_of_a = Player((g & 1) as u8);
        let levels = |p: Player| if p == seat_of_a { a } else { b };
        let dealer = Player(((g >> 1) & 1) as u8);
        let mut deal = Deal::random(&mut rng, dealer);
        while !deal.is_over() {
            let p = deal.to_move().expect("in progress");
            let view = PlayerView::of(&deal, p);
            let m = ai.choose_move(&view, levels(p), nodes).expect("engine move");
            deal.apply(m.mv)?;
        }
        let r = deal.result().expect("over");
        for (i, seat) in [seat_of_a, seat_of_a.other()].iter().enumerate() {
            let st = &mut stats[i];
            st.points += r.points[seat.index()] as u64;
            st.score += r.signed_score(*seat) as i64;
            match r.winner {
                Some(w) if w == *seat => st.wins += 1,
                None => st.ties += 1,
                _ => {}
            }
        }
        if (g + 1) % 10 == 0 {
            eprint!("\r{}/{games}", g + 1);
        }
    }
    eprintln!();
    let elapsed = start.elapsed();
    let used = ai.solver.nodes - nodes_start;
    println!("Self-play: {games} deals, seed {seed}, nodes/move budget {nodes}, {:.1?} total, {} nodes ({:.1} Mn/s)", elapsed, used, used as f64 / elapsed.as_secs_f64().max(1e-9) / 1e6);
    for (st, lvl) in stats.iter().zip([a, b]) {
        println!(
            "  {:<6?} wins {:>3} ties {:>3} | avg points {:>5.1} | avg signed score {:>6.2}",
            lvl,
            st.wins,
            st.ties,
            st.points as f64 / games as f64,
            st.score as f64 / games as f64
        );
    }
    Ok(())
}
