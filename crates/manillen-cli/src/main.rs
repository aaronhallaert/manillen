//! Developer command line for the Manillen engine: play, self-play, analyze, bench.

mod analyze;
mod par;
mod bench;
mod play;
mod render;
mod selfplay;

use clap::{Parser, Subcommand};
use manillen_core::ai::Level;

#[derive(Parser)]
#[command(name = "manillen-cli", version, about = "Manillen (2-player) engine tools")]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    /// Play a match against the engine in the terminal.
    Play {
        #[arg(long, default_value = "hard", value_parser = parse_level)]
        level: Level,
        #[arg(long)]
        seed: Option<u64>,
        #[arg(long, default_value_t = 101)]
        target: u16,
        #[arg(long, default_value_t = 0)]
        human: u8,
        #[arg(long, default_value_t = 2_000_000)]
        nodes: u64,
    },
    /// Let two levels play single deals against each other.
    Selfplay {
        #[arg(long, value_parser = parse_level)]
        a: Level,
        #[arg(long, value_parser = parse_level)]
        b: Level,
        #[arg(long, default_value_t = 100)]
        games: u32,
        #[arg(long)]
        seed: Option<u64>,
        #[arg(long, default_value_t = 300_000)]
        nodes: u64,
        /// Unused for single deals; kept for symmetry with `play`.
        #[arg(long, default_value_t = 0)]
        target: u16,
    },
    /// Review a saved deal record for one player.
    Analyze {
        record: std::path::PathBuf,
        #[arg(long, default_value_t = 0)]
        player: u8,
        #[arg(long, default_value_t = 2_000_000)]
        nodes: u64,
    },
    /// Perfect-information par of a saved deal: what both sides score with perfect play, per bid.
    Par {
        record: std::path::PathBuf,
        #[arg(long, default_value_t = 200_000_000)]
        nodes: u64,
    },
    /// Measure solver and search speed.
    Bench {
        #[arg(long, default_value_t = 10)]
        deals: u32,
        #[arg(long, default_value_t = 2_000_000)]
        nodes: u64,
    },
}

fn parse_level(s: &str) -> Result<Level, String> {
    Level::parse(s).ok_or_else(|| format!("unknown level {s:?} (easy|medium|hard)"))
}

fn main() {
    let cli = Cli::parse();
    let result = match cli.command {
        Command::Play { level, seed, target, human, nodes } => play::run(level, seed, target, human & 1, nodes),
        Command::Selfplay { a, b, games, seed, nodes, .. } => selfplay::run(a, b, games, seed, nodes),
        Command::Analyze { record, player, nodes } => analyze::run(&record, player & 1, nodes),
        Command::Par { record, nodes } => par::run(&record, nodes),
        Command::Bench { deals, nodes } => bench::run(deals, nodes),
    };
    if let Err(e) = result {
        eprintln!("error: {e}");
        std::process::exit(1);
    }
}

fn seed_or_time(seed: Option<u64>) -> u64 {
    seed.unwrap_or_else(|| {
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_nanos() as u64)
            .unwrap_or(1)
    })
}
