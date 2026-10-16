//! Post-game review: re-evaluate every decision of one player from their own information set.

use super::pimc::Unit;
use super::{Ai, Level};
use crate::card::Player;
use crate::deal::Phase;
use crate::record::GameRecord;
use crate::rules::Move;
use crate::view::PlayerView;
use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, PartialEq, Eq, Debug, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Label {
    Best,
    Good,
    Inaccuracy,
    Mistake,
    Blunder,
}

impl Label {
    pub fn for_loss(loss: f32) -> Label {
        if loss <= 0.5 {
            Label::Best
        } else if loss < 2.0 {
            Label::Good
        } else if loss < 4.0 {
            Label::Inaccuracy
        } else if loss < 7.0 {
            Label::Mistake
        } else {
            Label::Blunder
        }
    }
}

#[derive(Clone, PartialEq, Debug, Serialize, Deserialize)]
pub struct Decision {
    pub index: usize,
    pub trick: i8,
    pub phase: Phase,
    pub chosen: Move,
    pub best: Move,
    pub options: Vec<super::pimc::EvalOption>,
    pub value_chosen: f32,
    pub value_best: f32,
    pub loss: f32,
    pub label: Label,
    pub unit: Unit,
    pub note: Option<String>,
    /// Why the best move is good, derived only from the player's own view.
    #[serde(default)]
    pub reasons_best: Vec<String>,
    /// What the chosen move did, derived only from the player's own view.
    #[serde(default)]
    pub reasons_chosen: Vec<String>,
    /// Number of sampled layouts of the unseen cards the values are averaged over.
    #[serde(default)]
    pub samples: u32,
}

#[derive(Clone, PartialEq, Debug, Serialize, Deserialize)]
pub struct TimelinePoint {
    pub index: usize,
    pub value: f32,
}

#[derive(Clone, PartialEq, Debug, Default, Serialize, Deserialize)]
pub struct Summary {
    pub best: u32,
    pub good: u32,
    pub inaccuracy: u32,
    pub mistake: u32,
    pub blunder: u32,
    pub avg_loss: f32,
}

#[derive(Clone, PartialEq, Debug, Serialize, Deserialize)]
pub struct AnalysisReport {
    pub player: Player,
    pub decisions: Vec<Decision>,
    pub timeline: Vec<TimelinePoint>,
    pub summary: Summary,
}

/// Review every decision `player` made in `record`. `max_nodes` is the budget per decision.
/// `progress` is called after each decision with (done, total).
pub fn analyze(ai: &mut Ai, record: &GameRecord, player: Player, max_nodes: u64, mut progress: impl FnMut(usize, usize)) -> Result<AnalysisReport, crate::deal::DealError> {
    let mut deal = crate::deal::Deal::new(record.layout.clone(), record.dealer)?;
    let total = record.moves.len();
    let mut decisions = Vec::new();
    let mut timeline = Vec::new();
    for (index, &mv) in record.moves.iter().enumerate() {
        if deal.to_move() == Some(player) {
            let view = PlayerView::of(&deal, player);
            if let Some(mut eval) = ai.evaluate(&view, Level::Hard, max_nodes) {
                eval.sort();
                let best = eval.best().cloned();
                let chosen_value = eval.value_of(mv);
                if let (Some(best), Some(value_chosen)) = (best, chosen_value) {
                    let loss = (best.value - value_chosen).max(0.0);
                    let note = note_for(&view, mv, best.mv, loss, eval.unit);
                    let outcome = |opt: &super::pimc::EvalOption, is_best: bool| -> Option<String> {
                        let lost = opt.trick_lost?;
                        let n = eval.samples;
                        let c = opt.mv.card()?;
                        let card = format!("{}{}", c.rank().label(), c.suit().symbol());
                        let won = n - lost;
                        let tail = if is_best {
                            format!("and it leaves you with the most points afterwards ({:.1} on average)", opt.value)
                        } else {
                            format!("and you average {:.1} points afterwards", opt.value)
                        };
                        Some(if lost == 0 {
                            format!("Leading {card}: you win this trick in all {n} sampled layouts, {tail}.")
                        } else if lost == n {
                            format!("Leading {card}: the opponent takes this trick in all {n} sampled layouts, {tail}.")
                        } else {
                            format!("Leading {card}: you win this trick in {won} of {n} sampled layouts, {tail}.")
                        })
                    };
                    let mut reasons_best = match best.mv {
                        Move::Play { card } => super::heuristic::reasons(card, &view),
                        _ => Vec::new(),
                    };
                    if let Some(o) = outcome(&best, true) {
                        reasons_best.insert(0, o);
                    }
                    let mut reasons_chosen = match mv {
                        Move::Play { card } if mv != best.mv => super::heuristic::reasons(card, &view),
                        _ => Vec::new(),
                    };
                    if mv != best.mv {
                        if let Some(o) = eval.options.iter().find(|o| o.mv == mv).and_then(|o| outcome(o, false)) {
                            reasons_chosen.insert(0, o);
                        }
                    }
                    decisions.push(Decision {
                        index,
                        trick: if deal.phase() == Phase::Play { deal.tricks().len() as i8 } else { -1 },
                        phase: deal.phase(),
                        chosen: mv,
                        best: best.mv,
                        options: eval.options.clone(),
                        value_chosen,
                        value_best: best.value,
                        loss,
                        label: Label::for_loss(loss),
                        unit: eval.unit,
                        note,
                        reasons_best,
                        reasons_chosen,
                        samples: eval.samples,
                    });
                    if eval.unit == Unit::Points {
                        timeline.push(TimelinePoint { index, value: value_chosen });
                    }
                }
            }
        }
        deal.apply(mv)?;
        progress(index + 1, total);
    }
    let mut summary = Summary::default();
    let mut total_loss = 0.0;
    for d in &decisions {
        match d.label {
            Label::Best => summary.best += 1,
            Label::Good => summary.good += 1,
            Label::Inaccuracy => summary.inaccuracy += 1,
            Label::Mistake => summary.mistake += 1,
            Label::Blunder => summary.blunder += 1,
        }
        total_loss += d.loss;
    }
    if !decisions.is_empty() {
        summary.avg_loss = total_loss / decisions.len() as f32;
    }
    Ok(AnalysisReport { player, decisions, timeline, summary })
}

fn note_for(view: &PlayerView, chosen: Move, best: Move, loss: f32, unit: Unit) -> Option<String> {
    if loss <= 0.5 {
        return None;
    }
    match (chosen, best) {
        (Move::Play { card: c }, Move::Play { card: b }) => {
            let _ = view;
            Some(format!("{b} was better than {c} (about {loss:.1} {} lost).", if unit == Unit::Points { "points" } else { "score" }))
        }
        (Move::Bid { bid: c }, Move::Bid { bid: b }) => Some(format!("Bidding {b} was better than {c} (about {loss:.1} score lost).")),
        (Move::Contra { contra: true }, Move::Contra { contra: false }) => Some(format!("Contra was too optimistic here (about {loss:.1} score lost).")),
        (Move::Contra { contra: false }, Move::Contra { contra: true }) => Some(format!("A contra would have paid off (about {loss:.1} score missed).")),
        _ => None,
    }
}
