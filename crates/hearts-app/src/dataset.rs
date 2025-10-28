use crate::bot::{BotDifficulty, PlayPlanner, PlayPlannerHard, play_bias};
use crate::controller::GameController;
use hearts_core::model::player::PlayerPosition;
use hearts_core::model::suit::Suit;
use serde::Serialize;

#[derive(Serialize)]
pub struct PlayCandidateRecord {
    pub card: String,
    pub base: i32,
    pub continuation: Option<i32>,
    pub total: i32,
    pub penalty_value: u8,
    pub belief_prob: f32,
    pub void_in_lead: bool,
    pub is_best: bool,
    pub cont_breakdown: Option<ContinuationBreakdown>,
    pub adviser_bias: i32,
}

#[derive(Serialize, Default)]
pub struct ContinuationBreakdown {
    pub feed: i32,
    pub self_capture: i32,
    pub next_start: i32,
    pub next_probe: i32,
    pub qs_risk: i32,
    pub ctrl_hearts: i32,
    pub ctrl_handoff: i32,
    pub moon_relief: i32,
    pub capped_delta: i32,
}

#[derive(Serialize)]
pub struct PlaySampleRecord {
    pub seed: u64,
    pub seat: String,
    pub difficulty: String,
    pub trick_index: usize,
    pub legal_count: usize,
    pub lead_suit: Option<String>,
    pub hearts_broken: bool,
    pub penalties_on_trick: u8,
    pub belief_entropy_self: f32,
    pub candidates: Vec<PlayCandidateRecord>,
}

pub fn collect_play_sample(
    controller: &GameController,
    seat: PlayerPosition,
    seed: u64,
) -> Option<PlaySampleRecord> {
    let legal = controller.legal_moves(seat);
    if legal.is_empty() {
        return None;
    }
    let ctx = controller.bot_context(seat);
    let round = ctx.round;
    let trick = round.current_trick();
    let lead_suit = trick.lead_suit();
    let penalties_on_trick = trick.penalty_total();
    let belief = ctx.tracker.belief_state(seat);
    let belief_entropy_self = belief.entropy();
    let mut candidates = Vec::new();
    match ctx.difficulty {
        BotDifficulty::FutureHard => {
            let verbose = PlayPlannerHard::explain_candidates_verbose_parts(&legal, &ctx);
            for (card, base, parts, total) in verbose.into_iter() {
                let belief_prob = belief.card_probability(card);
                let void_in_lead = lead_suit
                    .map(|s| ctx.tracker.is_void(seat, s))
                    .unwrap_or(false);
                let adviser_bias = play_bias(card, &ctx);
                let breakdown = ContinuationBreakdown {
                    feed: parts.feed,
                    self_capture: parts.self_capture,
                    next_start: parts.next_start,
                    next_probe: parts.next_probe,
                    qs_risk: parts.qs_risk,
                    ctrl_hearts: parts.ctrl_hearts,
                    ctrl_handoff: parts.ctrl_handoff,
                    moon_relief: parts.moon_relief,
                    capped_delta: parts.capped_delta,
                };
                candidates.push(PlayCandidateRecord {
                    card: card.to_string(),
                    base,
                    continuation: Some(total - base),
                    total,
                    penalty_value: card.penalty_value(),
                    belief_prob,
                    void_in_lead,
                    is_best: false,
                    cont_breakdown: Some(breakdown),
                    adviser_bias,
                });
            }
        }
        _ => {
            let explained = PlayPlanner::explain_candidates(&legal, &ctx);
            for (card, base) in explained.into_iter() {
                let belief_prob = belief.card_probability(card);
                let void_in_lead = lead_suit
                    .map(|s| ctx.tracker.is_void(seat, s))
                    .unwrap_or(false);
                let adviser_bias = play_bias(card, &ctx);
                candidates.push(PlayCandidateRecord {
                    card: card.to_string(),
                    base,
                    continuation: None,
                    total: base,
                    penalty_value: card.penalty_value(),
                    belief_prob,
                    void_in_lead,
                    is_best: false,
                    cont_breakdown: None,
                    adviser_bias,
                });
            }
        }
    }
    if candidates.is_empty() {
        return None;
    }
    let best_total = candidates.iter().map(|c| c.total).max().unwrap_or(0);
    for candidate in candidates.iter_mut() {
        if candidate.total == best_total {
            candidate.is_best = true;
        }
    }
    Some(PlaySampleRecord {
        seed,
        seat: seat.to_string(),
        difficulty: format!("{:?}", ctx.difficulty),
        trick_index: round.trick_history().len(),
        legal_count: legal.len(),
        lead_suit: lead_suit.map(|s| s.to_string()),
        hearts_broken: round.hearts_broken(),
        penalties_on_trick,
        belief_entropy_self,
        candidates,
    })
}

trait SuitDisplay {
    fn to_string(self) -> String;
}

impl SuitDisplay for Suit {
    fn to_string(self) -> String {
        format!("{}", self)
    }
}
