//! Review a saved deal record.

use crate::render;
use manillen_core::ai::{analyze, Ai};
use manillen_core::{GameRecord, Player};
use std::path::Path;

pub fn run(path: &Path, player: u8, nodes: u64) -> Result<(), Box<dyn std::error::Error>> {
    let json = std::fs::read_to_string(path)?;
    let record: GameRecord = serde_json::from_str(&json)?;
    let mut ai = Ai::new(7, 20);
    eprint!("Analyzing");
    let report = analyze(&mut ai, &record, Player(player), nodes, |done, total| {
        eprint!("\rAnalyzing {done}/{total}");
    })?;
    eprintln!();
    println!("{}", render::analysis(&report));
    let out = format!("{}.analysis.json", path.display());
    std::fs::write(&out, serde_json::to_string_pretty(&report)?)?;
    println!("Written {out}");
    Ok(())
}
