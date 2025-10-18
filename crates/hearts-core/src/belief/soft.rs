//! Behavior-driven likelihood adjustments layered on top of hard belief updates.

use super::{Belief, BeliefUpdateCtx};
use crate::model::card::Card;
use crate::model::player::PlayerPosition;
use crate::model::rank::Rank;
use crate::model::suit::Suit;
use std::env;

/// Tunable configuration for soft likelihood adjustments.
#[derive(Debug, Clone, Copy)]
pub struct SoftConfig {
    /// Multiplier applied to the probability that the acting seat holds the Queen of Spades
    /// when they decline to play it in a spade-led trick.
    pub queen_avoidance_weight: f32,
    /// Multiplier applied to the acting seat's remaining cards of the suit they slough
    /// when discarding penalty cards off-suit.
    pub penalty_slough_weight: f32,
    /// Lower bound applied to any multiplier to avoid collapsing a column entirely.
    pub minimum_weight: f32,
}

impl Default for SoftConfig {
    fn default() -> Self {
        Self {
            queen_avoidance_weight: 0.65,
            penalty_slough_weight: 1.15,
            minimum_weight: 0.1,
        }
    }
}

impl SoftConfig {
    pub fn from_env() -> Self {
        let base = Self::default();
        let queen = parse_env_f32("MDH_BELIEF_SOFT_QUEEN", base.queen_avoidance_weight);
        let slough = parse_env_f32("MDH_BELIEF_SOFT_SLOUGH", base.penalty_slough_weight);
        let min_weight = parse_env_f32("MDH_BELIEF_SOFT_MIN", base.minimum_weight).clamp(0.01, 0.9);

        Self {
            queen_avoidance_weight: queen.clamp(0.1, 1.5),
            penalty_slough_weight: slough.clamp(0.5, 2.0),
            minimum_weight: min_weight,
        }
    }
}

fn parse_env_f32(key: &str, fallback: f32) -> f32 {
    env::var(key)
        .ok()
        .and_then(|value| value.parse::<f32>().ok())
        .filter(|value| value.is_finite())
        .unwrap_or(fallback)
}

/// Encapsulates the soft likelihood heuristics surfaced in Stage 1.
#[derive(Debug, Clone)]
pub struct SoftLikelihoodModel {
    config: SoftConfig,
}

impl SoftLikelihoodModel {
    pub fn new(config: SoftConfig) -> Self {
        Self { config }
    }

    pub fn default() -> Self {
        Self::new(SoftConfig::from_env())
    }

    /// Applies soft adjustments after the deterministic update for an observed play.
    pub fn update_after_play(
        &self,
        belief: &mut Belief,
        seat: PlayerPosition,
        card: Card,
        ctx: &BeliefUpdateCtx,
    ) {
        if seat == belief.perspective() {
            return;
        }

        self.apply_queen_avoidance(belief, seat, card, ctx);
        self.apply_penalty_slough(belief, seat, card, ctx);
    }

    fn apply_queen_avoidance(
        &self,
        belief: &mut Belief,
        seat: PlayerPosition,
        card: Card,
        ctx: &BeliefUpdateCtx,
    ) {
        if ctx.lead_suit() != Some(Suit::Spades) {
            return;
        }
        if card.suit != Suit::Spades || card.rank >= Rank::Queen {
            return;
        }

        let queen = Card::new(Rank::Queen, Suit::Spades);
        let weight = self
            .config
            .queen_avoidance_weight
            .max(self.config.minimum_weight);
        belief.scale_card_probability(seat, queen, weight);
    }

    fn apply_penalty_slough(
        &self,
        belief: &mut Belief,
        seat: PlayerPosition,
        card: Card,
        ctx: &BeliefUpdateCtx,
    ) {
        let Some(lead) = ctx.lead_suit() else {
            return;
        };

        if lead == card.suit || !card.is_penalty() {
            return;
        }

        let weight = self
            .config
            .penalty_slough_weight
            .max(self.config.minimum_weight);
        belief.scale_suit_for_seat(seat, card.suit, weight);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::rank::Rank;

    #[test]
    fn queen_avoidance_reduces_probability() {
        let mut belief = Belief::new_uninitialized(PlayerPosition::South);
        let model = SoftLikelihoodModel::default();
        let queen = Card::new(Rank::Queen, Suit::Spades);
        let before = belief.prob_card(PlayerPosition::East, queen);
        let ctx = BeliefUpdateCtx::new(0, Some(Suit::Spades));

        belief.on_card_played_with_soft(
            PlayerPosition::East,
            Card::new(Rank::Ten, Suit::Spades),
            &ctx,
            Some(&model),
        );

        let after = belief.prob_card(PlayerPosition::East, queen);
        assert!(after < before);
    }

    #[test]
    fn penalty_slough_increases_suit_weight() {
        let mut belief = Belief::new_uninitialized(PlayerPosition::South);
        let model = SoftLikelihoodModel::default();
        let target = Card::new(Rank::Ace, Suit::Hearts);
        let before = belief.prob_card(PlayerPosition::East, target);
        let ctx = BeliefUpdateCtx::new(0, Some(Suit::Clubs));

        belief.on_card_played_with_soft(
            PlayerPosition::East,
            Card::new(Rank::Two, Suit::Hearts),
            &ctx,
            Some(&model),
        );

        let after = belief.prob_card(PlayerPosition::East, target);
        assert!(after > before);
    }
}
