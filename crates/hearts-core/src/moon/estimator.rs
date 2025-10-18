use crate::model::hand::Hand;
use crate::model::passing::PassingDirection;
use crate::model::player::PlayerPosition;
use crate::model::rank::Rank;
use crate::model::round::RoundState;
use crate::model::score::ScoreBoard;
use crate::model::suit::Suit;

#[derive(Debug, Clone, Copy)]
pub struct MoonEstimatorConfig {
    pub intercept: f32,
    pub hearts_weight: f32,
    pub high_hearts_weight: f32,
    pub void_weight: f32,
    pub queen_guard_weight: f32,
    pub penalty_mass_weight: f32,
    pub score_pressure_weight: f32,
    pub direction_weight: f32,
    pub block_threshold: f32,
}

impl Default for MoonEstimatorConfig {
    fn default() -> Self {
        Self {
            intercept: -1.35,
            hearts_weight: 0.18,
            high_hearts_weight: 0.42,
            void_weight: -0.28,
            queen_guard_weight: -0.6,
            penalty_mass_weight: -0.04,
            score_pressure_weight: 0.12,
            direction_weight: 0.25,
            block_threshold: 0.45,
        }
    }
}

#[derive(Debug, Clone, Copy)]
pub struct MoonEstimator {
    config: MoonEstimatorConfig,
}

impl MoonEstimator {
    pub const fn new(config: MoonEstimatorConfig) -> Self {
        Self { config }
    }

    pub const fn config(&self) -> MoonEstimatorConfig {
        self.config
    }

    pub fn estimate(&self, features: MoonFeatures) -> MoonEstimate {
        let cfg = self.config;
        let mut linear = cfg.intercept;
        linear += features.hearts_in_hand * cfg.hearts_weight;
        linear += features.high_hearts * cfg.high_hearts_weight;
        linear += features.voids * cfg.void_weight;
        linear += features.queen_guard * cfg.queen_guard_weight;
        linear += features.penalty_mass * cfg.penalty_mass_weight;
        linear += features.score_pressure * cfg.score_pressure_weight;
        linear += features.direction_factor * cfg.direction_weight;

        let probability = sigmoid(linear).clamp(0.0, 1.0);
        let objective = if probability >= cfg.block_threshold {
            MoonObjective::BlockShooter
        } else {
            MoonObjective::MyPointsPerHand
        };

        MoonEstimate {
            probability,
            raw_score: linear,
            objective,
        }
    }
}

impl Default for MoonEstimator {
    fn default() -> Self {
        Self::new(MoonEstimatorConfig::default())
    }
}

#[derive(Debug, Clone, Copy)]
pub struct MoonFeatures {
    pub hearts_in_hand: f32,
    pub high_hearts: f32,
    pub voids: f32,
    pub queen_guard: f32,
    pub penalty_mass: f32,
    pub score_pressure: f32,
    pub direction_factor: f32,
}

impl MoonFeatures {
    pub fn from_state(
        seat: PlayerPosition,
        round: &RoundState,
        scores: &ScoreBoard,
        passing_direction: PassingDirection,
    ) -> Self {
        let hand = round.hand(seat);
        let hearts_in_hand = count_suit(hand, Suit::Hearts) as f32;
        let high_hearts = count_high_hearts(hand) as f32;
        let queen_guard = if hand.iter().any(|card| card.is_queen_of_spades()) {
            1.0
        } else {
            0.0
        };

        let voids = Suit::ALL
            .iter()
            .filter(|suit| count_suit(hand, **suit) == 0)
            .count() as f32;

        let penalty_mass = total_penalties(hand) as f32 / 26.0;

        let standings = scores.standings();
        let my_score = standings[seat.index()] as f32;
        let low_score = standings.iter().copied().min().unwrap_or(0) as f32;
        let score_pressure = ((my_score - low_score) / 26.0).clamp(-2.0, 4.0);

        let direction_factor = direction_bias(passing_direction);

        Self {
            hearts_in_hand,
            high_hearts,
            voids,
            queen_guard,
            penalty_mass,
            score_pressure,
            direction_factor,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MoonObjective {
    MyPointsPerHand,
    BlockShooter,
}

#[derive(Debug, Clone, Copy)]
pub struct MoonEstimate {
    pub probability: f32,
    pub raw_score: f32,
    pub objective: MoonObjective,
}

impl MoonEstimate {
    pub fn defensive_urgency(self) -> f32 {
        self.probability
    }
}

fn count_suit(hand: &Hand, suit: Suit) -> usize {
    hand.iter().filter(|card| card.suit == suit).count()
}

fn count_high_hearts(hand: &Hand) -> usize {
    hand.iter()
        .filter(|card| card.suit == Suit::Hearts && card.rank >= Rank::Queen)
        .count()
}

fn total_penalties(hand: &Hand) -> u32 {
    hand.iter().map(|card| card.penalty_value() as u32).sum()
}

fn direction_bias(direction: PassingDirection) -> f32 {
    match direction {
        PassingDirection::Left => 0.05,
        PassingDirection::Right => 0.22,
        PassingDirection::Across => 0.18,
        PassingDirection::Hold => 0.03,
    }
}

fn sigmoid(value: f32) -> f32 {
    1.0 / (1.0 + (-value).exp())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn estimator_increases_probability_with_hearts() {
        let estimator = MoonEstimator::default();
        let defensive = MoonFeatures {
            hearts_in_hand: 2.0,
            high_hearts: 0.0,
            voids: 1.0,
            queen_guard: 1.0,
            penalty_mass: 0.1,
            score_pressure: 0.2,
            direction_factor: 0.05,
        };
        let aggressive = MoonFeatures {
            hearts_in_hand: 8.0,
            high_hearts: 3.0,
            voids: 0.0,
            queen_guard: 0.0,
            penalty_mass: 0.6,
            score_pressure: 0.2,
            direction_factor: 0.05,
        };
        let defensive_prob = estimator.estimate(defensive).probability;
        let aggressive_prob = estimator.estimate(aggressive).probability;
        assert!(aggressive_prob > defensive_prob);
    }

    #[test]
    fn high_probability_triggers_block_objective() {
        let estimator = MoonEstimator::new(MoonEstimatorConfig {
            intercept: 5.0,
            hearts_weight: 0.0,
            high_hearts_weight: 0.0,
            void_weight: 0.0,
            queen_guard_weight: 0.0,
            penalty_mass_weight: 0.0,
            score_pressure_weight: 0.0,
            direction_weight: 0.0,
            block_threshold: 0.4,
        });
        let features = MoonFeatures {
            hearts_in_hand: 0.0,
            high_hearts: 0.0,
            voids: 0.0,
            queen_guard: 0.0,
            penalty_mass: 0.0,
            score_pressure: 0.0,
            direction_factor: 0.0,
        };
        let estimate = estimator.estimate(features);
        assert_eq!(estimate.objective, MoonObjective::BlockShooter);
        assert!(estimate.probability > 0.99);
    }
}
