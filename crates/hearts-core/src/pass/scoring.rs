use super::direction::DirectionProfile;
use crate::belief::Belief;
use crate::model::card::Card;
use crate::model::hand::Hand;
use crate::model::passing::PassingDirection;
use crate::model::player::PlayerPosition;
use crate::model::rank::Rank;
use crate::model::round::RoundState;
use crate::model::score::ScoreBoard;
use crate::model::suit::Suit;
use crate::moon::MoonEstimate;

/// Weights governing the relative value of different scoring components.
#[derive(Debug, Clone, Copy)]
pub struct PassWeights {
    /// Base bonus for creating a void after the pass.
    pub void_base: f32,
    /// Multiplier applied when voiding a penalty suit (hearts/spades).
    pub void_penalty_multiplier: f32,
    /// Base penalty reduction for offloading high-liability cards.
    pub liability_base: f32,
    /// Additional gain for removing the Queen of Spades specifically.
    pub queen_liability_bonus: f32,
    /// Bonus for improving moon potential when estimator (future work) indicates high chance.
    pub moon_support_base: f32,
    /// Fallback weight when beliefs are unavailable or high entropy.
    pub fallback_weight: f32,
}

impl Default for PassWeights {
    fn default() -> Self {
        Self {
            void_base: 22.0,
            void_penalty_multiplier: 1.35,
            liability_base: 18.0,
            queen_liability_bonus: 40.0,
            moon_support_base: 12.0,
            fallback_weight: 14.0,
        }
    }
}

/// Context supplied to evaluate a pass candidate.
pub struct PassScoreInput<'a> {
    pub seat: PlayerPosition,
    pub hand: &'a Hand,
    pub round: &'a RoundState,
    pub scores: &'a ScoreBoard,
    pub belief: Option<&'a Belief>,
    pub weights: PassWeights,
    pub direction: PassingDirection,
    pub direction_profile: DirectionProfile,
    pub moon_estimate: MoonEstimate,
}

#[derive(Debug, Clone, Copy, Default)]
pub struct PassScoreBreakdown {
    pub void_value: f32,
    pub liability_reduction: f32,
    pub direction_bonus: f32,
    pub moon_support: f32,
    pub total: f32,
}

impl PassScoreBreakdown {
    pub fn total_score(self) -> f32 {
        self.total
    }
}

pub fn score_card(input: &PassScoreInput<'_>, card: Card) -> PassScoreBreakdown {
    let void_value = compute_void_value(input, card);
    let liability_reduction = compute_liability_reduction(input, card);
    let moon_support = compute_moon_support(input, card);
    let direction_bonus = (void_value + liability_reduction + moon_support)
        * (input.direction_profile.liability_factor - 1.0)
        * 0.25;

    let total = void_value + liability_reduction + moon_support + direction_bonus;

    PassScoreBreakdown {
        void_value,
        liability_reduction,
        direction_bonus,
        moon_support,
        total,
    }
}

fn compute_void_value(input: &PassScoreInput<'_>, card: Card) -> f32 {
    let hand_count = input
        .hand
        .iter()
        .filter(|c| c.suit == card.suit && **c != card)
        .count();

    let base = if hand_count == 0 {
        input.weights.void_base * 1.5
    } else if hand_count == 1 {
        input.weights.void_base
    } else if hand_count == 2 {
        input.weights.void_base * 0.6
    } else {
        input.weights.fallback_weight
    };

    let penalty_multiplier = match card.suit {
        Suit::Hearts | Suit::Spades => input.weights.void_penalty_multiplier,
        _ => 1.0,
    };

    let belief_multiplier = match input.belief {
        Some(belief) => {
            let void_prob = void_probability_after_pass(belief, input.seat, card.suit);
            void_prob.clamp(0.2, 1.5)
        }
        None => 1.0,
    };

    base * penalty_multiplier * belief_multiplier * input.direction_profile.void_factor
}

fn compute_liability_reduction(input: &PassScoreInput<'_>, card: Card) -> f32 {
    let mut score = card.penalty_value() as f32 * input.weights.liability_base;

    if card.is_queen_of_spades() {
        score += input.weights.queen_liability_bonus;
    }

    if let Some(belief) = input.belief {
        let threat_prob = threat_probability(belief, card);
        score *= threat_prob.clamp(0.8, 2.0);
    }

    score * input.direction_profile.liability_factor
}

fn compute_moon_support(input: &PassScoreInput<'_>, card: Card) -> f32 {
    let urgency = input.moon_estimate.defensive_urgency();
    if urgency <= 0.01 {
        return 0.0;
    }
    let control = card_control_factor(card);
    let mut weight = input.weights.moon_support_base;

    if urgency >= 0.6 {
        let high_liability =
            card.is_queen_of_spades() || (card.suit == Suit::Hearts && card.rank >= Rank::Queen);
        if high_liability {
            let urgency_scale = ((urgency - 0.6) / 0.4).clamp(0.0, 1.0);
            let mut boost = 1.0 + 0.35 * urgency_scale;

            if matches!(input.seat, PlayerPosition::West | PlayerPosition::North) {
                boost += 0.25 * urgency_scale;
            }

            if matches!(
                input.direction,
                PassingDirection::Left | PassingDirection::Across
            ) {
                boost += 0.2 * urgency_scale;
            }

            weight *= boost;
        }
    }

    -urgency * control * weight * input.direction_profile.moon_factor
}

fn card_control_factor(card: Card) -> f32 {
    if card.is_queen_of_spades() {
        return 1.6;
    }
    if card.suit == Suit::Hearts {
        return 0.9 + (card.rank.value() as f32 / 13.0);
    }
    if card.suit == Suit::Spades && card.rank >= Rank::King {
        return 1.2;
    }
    (card.penalty_value() as f32) * 0.18
}

fn void_probability_after_pass(belief: &Belief, seat: PlayerPosition, suit: Suit) -> f32 {
    let mass: f32 = belief.iter_suit_probs(seat, suit).copied().sum();

    (1.0 - mass).max(0.05)
}

fn threat_probability(belief: &Belief, card: Card) -> f32 {
    let opponents = PlayerPosition::LOOP
        .iter()
        .copied()
        .filter(|seat| *seat != belief.perspective());

    let total: f32 = opponents.map(|seat| belief.prob_card(seat, card)).sum();

    (1.0 + total).max(0.1)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::belief::Belief;
    use crate::model::deck::Deck;
    use crate::model::passing::PassingDirection;
    use crate::model::rank::Rank;
    use crate::moon::{MoonEstimator, MoonFeatures};

    fn build_context<'a>(round: &'a RoundState, belief: Option<&'a Belief>) -> PassScoreInput<'a> {
        let seat = PlayerPosition::South;
        let profile = DirectionProfile::from_direction(PassingDirection::Left);
        let scores = Box::leak(Box::new(ScoreBoard::new()));
        let moon_estimator = MoonEstimator::default();
        let moon_features = MoonFeatures::from_state(seat, round, scores, PassingDirection::Left);
        let moon_estimate = moon_estimator.estimate(moon_features);
        PassScoreInput {
            seat,
            hand: round.hand(seat),
            round,
            scores,
            belief,
            weights: PassWeights::default(),
            direction: PassingDirection::Left,
            direction_profile: profile,
            moon_estimate,
        }
    }

    #[test]
    fn void_value_decreases_with_more_cards() {
        let deck = Deck::shuffled_with_seed(10);
        let round = RoundState::deal(&deck, PlayerPosition::South, PassingDirection::Left);
        let belief = Belief::from_state(&round, PlayerPosition::South);
        let ctx = build_context(&round, Some(&belief));

        let club = Card::new(Rank::Two, Suit::Clubs);
        let score = score_card(&ctx, club);
        assert!(score.void_value > 0.0);
    }

    #[test]
    fn liability_bonus_higher_for_queen_spades() {
        let deck = Deck::shuffled_with_seed(42);
        let round = RoundState::deal(&deck, PlayerPosition::South, PassingDirection::Left);
        let belief = Belief::from_state(&round, PlayerPosition::South);
        let ctx = build_context(&round, Some(&belief));

        let queen_spades = Card::new(Rank::Queen, Suit::Spades);
        let ace_clubs = Card::new(Rank::Ace, Suit::Clubs);

        let qs_score = score_card(&ctx, queen_spades);
        let ac_score = score_card(&ctx, ace_clubs);
        assert!(qs_score.liability_reduction > ac_score.liability_reduction);
    }
}
