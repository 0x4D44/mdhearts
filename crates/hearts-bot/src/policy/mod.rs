mod heuristic;

pub use heuristic::HeuristicPolicy;

use crate::bot::{BotFeatures, UnseenTracker};
use hearts_core::belief::Belief;
use hearts_core::model::card::Card;
use hearts_core::model::hand::Hand;
use hearts_core::model::passing::PassingDirection;
use hearts_core::model::player::PlayerPosition;
use hearts_core::model::round::RoundState;
use hearts_core::model::score::ScoreBoard;

/// Context provided to policies for decision-making
pub struct PolicyContext<'a> {
    pub seat: PlayerPosition,
    pub hand: &'a Hand,
    pub round: &'a RoundState,
    pub scores: &'a ScoreBoard,
    pub passing_direction: PassingDirection,
    pub tracker: &'a UnseenTracker,
    pub belief: Option<&'a Belief>,
    pub features: BotFeatures,
}

/// Unified interface for AI decision-making (heuristic and learned policies)
pub trait Policy: Send {
    /// Choose 3 cards to pass (called during Passing phase)
    fn choose_pass(&mut self, ctx: &PolicyContext) -> [Card; 3];

    /// Choose 1 card to play (called during Playing phase)
    fn choose_play(&mut self, ctx: &PolicyContext) -> Card;

    /// Optional: Forward pass with critic for RL training
    /// Returns (card, value, log_prob)
    /// Default implementation uses deterministic play with placeholder values
    #[allow(dead_code)]
    fn forward_with_critic(&mut self, ctx: &PolicyContext) -> (Card, f32, f32) {
        let card = self.choose_play(ctx);
        (card, 0.0, 0.0)
    }

    /// Optional: Observe terminal state (for RL policies)
    #[allow(dead_code)]
    fn observe_terminal(&mut self, _final_scores: &[u32; 4]) {}
}
