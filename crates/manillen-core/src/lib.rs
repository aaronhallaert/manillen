//! Manillen (two-player) rules engine and AI.
//!
//! See `docs/api.md` in the repository for the JSON contract that the serde types follow.

pub mod ai;
pub mod card;
pub mod cardset;
pub mod deal;
pub mod game;
pub mod record;
pub mod rules;
pub mod view;

pub use card::{Card, Player, Rank, Suit};
pub use cardset::CardSet;
pub use deal::{Deal, DealError, Event, Layout, Phase, Position, SlotPair, Trick};
pub use game::{Match, MatchSnapshot, MatchState};
pub use ai::{Ai, AiMove, Level};
pub use record::GameRecord;
pub use rules::{Bid, DealResult, Move};
pub use view::{CurrentTrick, MemoryModel, PlayerView, SlotView};

pub const VERSION: &str = env!("CARGO_PKG_VERSION");
