//! Interactive match against the engine.

use crate::render;
use manillen_core::ai::{analyze, Ai, Level};
use manillen_core::{Bid, Card, GameRecord, Match, Move, Phase, Player, PlayerView, Suit};
use std::io::{self, BufRead, Write};

pub fn run(level: Level, seed: Option<u64>, target: u16, human: u8, nodes: u64) -> Result<(), Box<dyn std::error::Error>> {
    let seed = crate::seed_or_time(seed);
    let human = Player(human);
    let mut m = Match::new(seed, target, Player::P0);
    let mut ai = Ai::new(seed ^ 0xA5A5, 20);
    let stdin = io::stdin();
    let mut lines = stdin.lock().lines();
    println!("Manillen — you are P{human}, engine level {level:?}, seed {seed}, target {target}.");
    println!("Enter a card like 10H or as, a bid as C/D/H/S/N, contra as y/n. 'q' quits.\n");

    loop {
        if m.deal().is_over() {
            let result = m.deal().result().expect("deal over");
            println!("\n{}", render::deal_result(&result));
            let deal_no = m.state().deal_index;
            let record = GameRecord::of(m.deal());
            std::fs::create_dir_all("records")?;
            let path = format!("records/deal-{deal_no}.json");
            std::fs::write(&path, serde_json::to_string_pretty(&record)?)?;
            println!("Record saved to {path}. Match: {}-{}", m.scores()[0], m.scores()[1]);
            if m.finished() {
                println!("Match over: P{} wins!", m.winner().unwrap());
            }
            print!("[r]eview this deal, [n]ext deal, [q]uit? ");
            io::stdout().flush()?;
            let line = match lines.next() {
                Some(l) => l?,
                None => return Ok(()),
            };
            match line.trim().to_ascii_lowercase().as_str() {
                "r" => {
                    review(&mut ai, &record, human, nodes);
                    if m.finished() {
                        return Ok(());
                    }
                    m.next_deal()?;
                }
                "q" => return Ok(()),
                _ => {
                    if m.finished() {
                        return Ok(());
                    }
                    m.next_deal()?;
                }
            }
            continue;
        }

        let to_move = m.deal().to_move().expect("deal in progress");
        if to_move == human {
            let view = PlayerView::of(m.deal(), human);
            println!("\n{}", render::table(&view, m.scores(), target));
            let legal = m.deal().legal_moves();
            let prompt = match view.phase {
                Phase::Bidding => "Your bid (C/D/H/S/N): ",
                Phase::Contra => "Contra? (y/n): ",
                _ => "Your card: ",
            };
            let mv = loop {
                print!("{prompt}");
                io::stdout().flush()?;
                let line = match lines.next() {
                    Some(l) => l?,
                    None => return Ok(()),
                };
                let line = line.trim();
                if line.eq_ignore_ascii_case("q") {
                    return Ok(());
                }
                match parse_input(line, view.phase) {
                    Some(mv) if legal.contains(&mv) => break mv,
                    _ => {
                        let options: Vec<String> = legal.iter().map(|&x| render::mv(x)).collect();
                        println!("Illegal. Legal: {}", options.join(", "));
                    }
                }
            };
            m.apply(mv)?;
            println!("You: {}", render::mv(mv));
        } else {
            let view = PlayerView::of(m.deal(), to_move);
            let chosen = ai.choose_move(&view, level, nodes).expect("engine move");
            let extra = chosen.eval.as_ref().map(|e| render::eval_short(e, chosen.mv)).unwrap_or_default();
            m.apply(chosen.mv)?;
            println!("Engine (P{to_move}): {} {extra}", render::mv(chosen.mv));
        }
    }
}

fn parse_input(s: &str, phase: Phase) -> Option<Move> {
    match phase {
        Phase::Bidding => {
            let c = s.chars().next()?.to_ascii_uppercase();
            if c == 'N' {
                Some(Move::Bid { bid: Bid::NoTrump })
            } else {
                Suit::from_letter(c).map(|suit| Move::Bid { bid: Bid::Trump { suit } })
            }
        }
        Phase::Contra => match s.to_ascii_lowercase().as_str() {
            "y" | "yes" | "contra" => Some(Move::Contra { contra: true }),
            "n" | "no" => Some(Move::Contra { contra: false }),
            _ => None,
        },
        Phase::Play => Card::parse(s).map(|card| Move::Play { card }),
        Phase::Done => None,
    }
}

pub fn review(ai: &mut Ai, record: &GameRecord, player: Player, nodes: u64) {
    eprint!("Analyzing");
    let report = analyze(ai, record, player, nodes, |done, total| {
        eprint!("\rAnalyzing {done}/{total}");
    });
    eprintln!();
    match report {
        Ok(r) => println!("{}", render::analysis(&r)),
        Err(e) => eprintln!("analysis failed: {e}"),
    }
}
