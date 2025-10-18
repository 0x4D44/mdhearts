use std::cmp::Ordering;

use super::direction::DirectionWeightKind;
use super::scoring::{PassScoreBreakdown, PassScoreInput, score_card as score_single_card};
use crate::model::card::Card;
use crate::model::passing::PassingDirection;
use crate::model::player::PlayerPosition;
use crate::model::rank::Rank;
use crate::model::suit::Suit;
use crate::moon::MoonObjective;

#[derive(Debug, Clone, Copy)]
pub struct PassOptimizerConfig {
    /// Max number of individual cards to keep in the evaluation pool.
    pub max_card_pool: usize,
    /// Max number of ranked combinations to return.
    pub max_candidates: usize,
    /// Minimum score a single card must have to be considered; ensures poor cards still have
    /// representation when the pool would otherwise be empty.
    pub min_single_score: f32,
}

impl Default for PassOptimizerConfig {
    fn default() -> Self {
        Self {
            max_card_pool: 9,
            max_candidates: 30,
            min_single_score: -40.0,
        }
    }
}

#[derive(Debug, Clone, Copy)]
pub struct PassCandidate {
    pub cards: [Card; 3],
    pub score: f32,
    pub void_score: f32,
    pub liability_score: f32,
    pub moon_score: f32,
    pub synergy: f32,
    pub direction_bonus: f32,
    pub moon_liability_penalty: f32,
}

pub fn enumerate_pass_triples(input: &PassScoreInput<'_>) -> Vec<PassCandidate> {
    enumerate_pass_triples_with_config(input, &PassOptimizerConfig::default())
}

pub fn enumerate_pass_triples_with_config(
    input: &PassScoreInput<'_>,
    config: &PassOptimizerConfig,
) -> Vec<PassCandidate> {
    let hand = input.hand;
    if hand.len() < 3 {
        return Vec::new();
    }

    let mut singles: Vec<(Card, PassScoreBreakdown)> = hand
        .iter()
        .copied()
        .map(|card| {
            let breakdown = score_single_card(input, card);
            (card, breakdown)
        })
        .collect();

    singles.sort_by(|a, b| b.1.total.total_cmp(&a.1.total));

    let pool = build_pool(&singles, config);

    let mut candidates = Vec::new();
    for i in 0..pool.len() {
        for j in (i + 1)..pool.len() {
            for k in (j + 1)..pool.len() {
                let combo = [pool[i], pool[j], pool[k]];
                let resolution = enforce_premium_support_rule(input, &combo);
                let mut combos_to_eval: Vec<[(Card, PassScoreBreakdown); 3]> = match resolution {
                    PremiumSupportResolution::Valid => vec![combo],
                    PremiumSupportResolution::Replacements(alts) => alts,
                    PremiumSupportResolution::Discard => continue,
                };

                for adjusted in combos_to_eval.drain(..) {
                    let eval = evaluate_combo(input, &adjusted);
                    if eval.moon_penalty >= HARD_REJECTION_PENALTY {
                        continue;
                    }
                    if eval.total < 0.5 && !contains_liability_combo(&adjusted) {
                        continue;
                    }
                    candidates.push(PassCandidate {
                        cards: [adjusted[0].0, adjusted[1].0, adjusted[2].0],
                        score: eval.total,
                        void_score: eval.void_sum,
                        liability_score: eval.liability_sum,
                        moon_score: eval.moon_sum,
                        synergy: eval.synergy,
                        direction_bonus: eval.direction_bonus,
                        moon_liability_penalty: eval.moon_penalty,
                    });
                }
            }
        }
    }

    candidates.sort_by(|a, b| b.score.total_cmp(&a.score));
    if candidates.len() > config.max_candidates {
        candidates.truncate(config.max_candidates);
    }
    candidates
}

fn enforce_premium_support_rule(
    input: &PassScoreInput<'_>,
    combo: &[(Card, PassScoreBreakdown); 3],
) -> PremiumSupportResolution {
    if !matches!(input.direction, PassingDirection::Left) {
        return PremiumSupportResolution::Valid;
    }
    let urgency = input.moon_estimate.defensive_urgency();
    if urgency < 0.6 {
        return PremiumSupportResolution::Valid;
    }
    let pass_has_ace = combo
        .iter()
        .any(|(card, _)| card.suit == Suit::Hearts && card.rank == Rank::Ace);
    let pass_has_king = combo
        .iter()
        .any(|(card, _)| card.suit == Suit::Hearts && card.rank == Rank::King);
    if !pass_has_ace && !pass_has_king {
        return PremiumSupportResolution::Valid;
    }
    let passed_ten_plus = combo
        .iter()
        .filter(|(card, _)| card.suit == Suit::Hearts && card.rank >= Rank::Ten)
        .count();
    if passed_ten_plus >= 3 {
        return PremiumSupportResolution::Valid;
    }

    let required = 3usize.saturating_sub(passed_ten_plus);
    let available: Vec<Card> = input
        .hand
        .iter()
        .filter(|card| {
            card.suit == Suit::Hearts
                && card.rank >= Rank::Ten
                && !combo.iter().any(|(existing, _)| existing == *card)
        })
        .cloned()
        .collect();
    if available.len() < required {
        return PremiumSupportResolution::Discard;
    }

    let mut replaceable: Vec<usize> = combo
        .iter()
        .enumerate()
        .filter(|(_, (card, _))| !(card.suit == Suit::Hearts && card.rank >= Rank::Ten))
        .map(|(idx, _)| idx)
        .collect();
    if replaceable.len() < required {
        return PremiumSupportResolution::Discard;
    }
    replaceable.sort_by(|a, b| {
        combo[*a]
            .1
            .total
            .partial_cmp(&combo[*b].1.total)
            .unwrap_or(Ordering::Equal)
    });

    let mut adjusted = combo.clone();
    let mut replacements = Vec::new();
    let mut hearts_iter = available.into_iter().rev();
    for slot in replaceable {
        if let Some(card) = hearts_iter.next() {
            adjusted[slot] = (card, score_single_card(input, card));
        }
        let count = adjusted
            .iter()
            .filter(|(card, _)| card.suit == Suit::Hearts && card.rank >= Rank::Ten)
            .count();
        if count >= 3 {
            replacements.push(adjusted);
            break;
        }
    }

    if replacements.is_empty() {
        PremiumSupportResolution::Discard
    } else {
        PremiumSupportResolution::Replacements(replacements)
    }
}

enum PremiumSupportResolution {
    Valid,
    Replacements(Vec<[(Card, PassScoreBreakdown); 3]>),
    Discard,
}

struct ComboEval {
    total: f32,
    void_sum: f32,
    liability_sum: f32,
    moon_sum: f32,
    synergy: f32,
    direction_bonus: f32,
    moon_penalty: f32,
}

fn evaluate_combo(
    input: &PassScoreInput<'_>,
    combo: &[(Card, PassScoreBreakdown); 3],
) -> ComboEval {
    let void_sum = combo.iter().map(|(_, b)| b.void_value).sum::<f32>();
    let liability_sum = combo
        .iter()
        .map(|(_, b)| b.liability_reduction)
        .sum::<f32>();
    let moon_sum = combo.iter().map(|(_, b)| b.moon_support).sum::<f32>();
    let synergy = void_synergy(input, combo);
    let base_total = void_sum + liability_sum + moon_sum + synergy;
    let direction_bonus = direction_bonus(input, combo, base_total);
    let moon_penalty = moon_liability_penalty(input, combo);

    ComboEval {
        total: base_total + direction_bonus - moon_penalty,
        void_sum,
        liability_sum,
        moon_sum,
        synergy,
        direction_bonus,
        moon_penalty,
    }
}

fn void_synergy(input: &PassScoreInput<'_>, combo: &[(Card, PassScoreBreakdown); 3]) -> f32 {
    let mut remaining = [0u8; 4];
    for card in input.hand.iter() {
        let idx = card.suit as usize;
        remaining[idx] = remaining[idx].saturating_add(1);
    }
    for (card, _) in combo {
        let idx = card.suit as usize;
        remaining[idx] = remaining[idx].saturating_sub(1);
    }

    let mut value = 0.0;
    for suit in Suit::ALL {
        let count = remaining[suit as usize];
        if count == 0 {
            value += input.weights.void_base * 0.6 * input.direction_profile.void_factor;
        } else if count == 1 {
            value += input.weights.void_base * 0.25;
        }
    }

    if combo.iter().any(|(card, _)| card.is_queen_of_spades()) {
        let spade_count = remaining[Suit::Spades as usize];
        let mitigation = (3u8.saturating_sub(spade_count)) as f32;
        value += mitigation * (input.weights.liability_base * 0.4);
    }

    value
}

fn direction_bonus(
    input: &PassScoreInput<'_>,
    combo: &[(Card, PassScoreBreakdown); 3],
    total: f32,
) -> f32 {
    let liability_bias = (input.direction_profile.liability_factor - 1.0) * 0.18;
    let void_bias = (input.direction_profile.void_factor - 1.0) * 0.12;
    let mut bonus = total * (liability_bias + void_bias);

    if input.direction_profile.kind == DirectionWeightKind::RightProtect
        && combo.iter().any(|(card, _)| card.is_queen_of_spades())
    {
        bonus += 18.0;
    }

    bonus
}

const HARD_REJECTION_PENALTY: f32 = 1_000_000_000.0;

fn moon_liability_penalty(
    input: &PassScoreInput<'_>,
    combo: &[(Card, PassScoreBreakdown); 3],
) -> f32 {
    let urgency = input.moon_estimate.defensive_urgency();
    if input.moon_estimate.objective != MoonObjective::BlockShooter || urgency < 0.55 {
        return 0.0;
    }

    let mut remaining: Vec<Card> = input.hand.iter().copied().collect();
    for (card, _) in combo.iter() {
        if let Some(idx) = remaining.iter().position(|candidate| candidate == card) {
            remaining.remove(idx);
        }
    }

    let high_hearts_remaining = remaining
        .iter()
        .filter(|card| card.suit == Suit::Hearts && card.rank >= Rank::Queen)
        .count();
    let high_hearts_passed = combo
        .iter()
        .filter(|(card, _)| card.suit == Suit::Hearts && card.rank >= Rank::Queen)
        .count();

    let queen_spades_remaining = remaining.iter().any(|card| card.is_queen_of_spades());
    let queen_spades_passed = combo.iter().any(|(card, _)| card.is_queen_of_spades());

    let remaining_high_hearts: Vec<Card> = remaining
        .iter()
        .copied()
        .filter(|card| card.suit == Suit::Hearts && card.rank >= Rank::Queen)
        .collect();
    let passed_high_heart_cards: Vec<Card> = combo
        .iter()
        .filter_map(|(card, _)| {
            if card.suit == Suit::Hearts && card.rank >= Rank::Queen {
                Some(*card)
            } else {
                None
            }
        })
        .collect();

    let remaining_mid_hearts = remaining
        .iter()
        .filter(|card| card.suit == Suit::Hearts && card.rank >= Rank::Ten)
        .count();
    let remaining_low_hearts = remaining
        .iter()
        .filter(|card| card.suit == Suit::Hearts && card.rank < Rank::Ten)
        .count();
    let passed_mid_hearts = combo
        .iter()
        .filter(|(card, _)| card.suit == Suit::Hearts && card.rank >= Rank::Ten)
        .count();
    let passed_low_hearts = combo
        .iter()
        .filter(|(card, _)| card.suit == Suit::Hearts && card.rank < Rank::Ten)
        .count();
    let passed_off_suit = combo
        .iter()
        .filter(|(card, _)| card.suit != Suit::Hearts)
        .count();

    let highest_remaining_heart = remaining_high_hearts
        .iter()
        .map(|card| card.rank)
        .max()
        .map(|rank| rank.value() as f32);
    let highest_passed_heart = passed_high_heart_cards
        .iter()
        .map(|card| card.rank)
        .max()
        .map(|rank| rank.value() as f32)
        .unwrap_or(0.0);

    let urgency_scale = ((urgency - 0.55) / 0.45).clamp(0.0, 1.0);
    let direction_scale = if matches!(
        input.direction,
        PassingDirection::Left | PassingDirection::Across
    ) {
        1.0
    } else {
        0.7
    };

    let mut penalty = 0.0;

    if queen_spades_remaining && !queen_spades_passed {
        let queen_scale = 0.55 + 0.45 * urgency_scale;
        penalty += input.weights.queen_liability_bonus * queen_scale * direction_scale;
    }

    if high_hearts_remaining >= 2 && high_hearts_passed == 0 {
        let severity = ((high_hearts_remaining as f32) - 1.0).clamp(0.5, 3.0) / 2.5;
        let handshake_scale = 0.65 + 0.35 * urgency_scale;
        penalty +=
            input.weights.liability_base * 4.2 * severity * handshake_scale * direction_scale;
    } else if remaining_high_hearts.len() >= 2 && high_hearts_passed == 1 {
        let deficit_scale = 0.65 + 0.35 * urgency_scale;
        let mass_scale = 1.0 + 0.15 * (remaining_high_hearts.len() as f32 - 1.0).max(0.0);
        penalty +=
            input.weights.liability_base * 1.5 * deficit_scale * mass_scale * direction_scale;
    }

    if urgency >= 0.55 {
        let total_high_hearts = high_hearts_remaining + high_hearts_passed;
        let shooter_scale = if matches!(input.direction, PassingDirection::Left) {
            left_shooter_pressure(input)
        } else {
            1.0
        };

        if total_high_hearts >= 2 {
            if high_hearts_passed < 2 {
                let deficit = (2usize.saturating_sub(high_hearts_passed)) as f32;
                let mut coverage_scale = 0.6 + 0.4 * urgency_scale;
                if matches!(input.direction, PassingDirection::Left)
                    && matches!(
                        input.seat,
                        PlayerPosition::North | PlayerPosition::West | PlayerPosition::South
                    )
                {
                    coverage_scale *= 1.25 * shooter_scale;
                }
                let total_scale = (total_high_hearts as f32).min(4.0) / 2.0;
                penalty += input.weights.liability_base
                    * 3.1
                    * deficit
                    * coverage_scale
                    * total_scale
                    * direction_scale;
            }

            if total_high_hearts >= 3 && high_hearts_passed < 2 {
                let deficit = (2usize.saturating_sub(high_hearts_passed)).max(1) as f32;
                let mut coverage_scale = 0.6 + 0.4 * urgency_scale;
                if matches!(input.direction, PassingDirection::Left)
                    && matches!(
                        input.seat,
                        PlayerPosition::North | PlayerPosition::West | PlayerPosition::South
                    )
                {
                    coverage_scale *= 1.35 * shooter_scale;
                }
                let total_scale = (total_high_hearts as f32).min(4.0) / 2.0;
                penalty += input.weights.liability_base
                    * 3.6
                    * deficit
                    * coverage_scale
                    * total_scale
                    * direction_scale;
            }

            if high_hearts_passed >= 1 && high_hearts_remaining >= 1 {
                let remainder = high_hearts_remaining as f32;
                let mut share_scale = 0.55 + 0.45 * urgency_scale;
                if matches!(input.direction, PassingDirection::Left)
                    && matches!(
                        input.seat,
                        PlayerPosition::North | PlayerPosition::West | PlayerPosition::South
                    )
                {
                    share_scale *= 1.2 * shooter_scale;
                }
                penalty +=
                    input.weights.liability_base * 1.4 * remainder * share_scale * direction_scale;
            }
        }

        if matches!(input.direction, PassingDirection::Left) {
            let seat_scale = match input.seat {
                PlayerPosition::North | PlayerPosition::South => 1.35,
                PlayerPosition::East | PlayerPosition::West => 1.2,
            };
            let pass_has_ace = combo
                .iter()
                .any(|(card, _)| card.suit == Suit::Hearts && card.rank == Rank::Ace);
            let pass_has_king = combo
                .iter()
                .any(|(card, _)| card.suit == Suit::Hearts && card.rank == Rank::King);
            let pass_premium_count = combo
                .iter()
                .filter(|(card, _)| card.suit == Suit::Hearts && card.rank >= Rank::Queen)
                .count();
            let passed_ten_plus = combo
                .iter()
                .filter(|(card, _)| card.suit == Suit::Hearts && card.rank >= Rank::Ten)
                .count();
            let passed_heart_count = combo
                .iter()
                .filter(|(card, _)| card.suit == Suit::Hearts)
                .count();
            let passed_support_hearts = combo
                .iter()
                .filter(|(card, _)| {
                    card.suit == Suit::Hearts && card.rank >= Rank::Ten && card.rank < Rank::Queen
                })
                .count();
            let shooter_split_threshold = 1.25;

            if urgency >= 0.6 {
                if pass_has_ace && passed_ten_plus < 3 {
                    return HARD_REJECTION_PENALTY + 1.0;
                }
                if pass_has_king && passed_ten_plus < 3 {
                    return HARD_REJECTION_PENALTY + 1.0;
                }
            }

            if shooter_scale >= shooter_split_threshold && high_hearts_passed >= 2 {
                let severity = 1.0 + (high_hearts_passed as f32 - 1.0).max(0.0);
                let shooter_bias = 1.0 + (shooter_scale - shooter_split_threshold).max(0.0);
                let ace_bias = if pass_has_ace { 1.3 } else { 1.0 };
                penalty += input.weights.liability_base
                    * 180.0
                    * severity
                    * shooter_bias
                    * ace_bias
                    * seat_scale
                    * direction_scale;
            }

            if high_hearts_passed >= 2 {
                let left_scale = 0.55 + 0.45 * urgency_scale;
                penalty += input.weights.liability_base
                    * 2.8
                    * (high_hearts_passed as f32 - 1.0)
                    * left_scale
                    * seat_scale
                    * shooter_scale
                    * direction_scale;
            }

            if high_hearts_passed <= 1 && passed_mid_hearts >= 2 {
                let left_scale = 0.45 + 0.55 * urgency_scale;
                penalty += input.weights.liability_base
                    * 1.9
                    * (passed_mid_hearts as f32 - high_hearts_passed as f32)
                    * left_scale
                    * seat_scale
                    * shooter_scale
                    * direction_scale;
            }

            if shooter_scale > 1.15 && pass_has_ace && pass_has_king {
                penalty += input.weights.liability_base
                    * 5.0
                    * (shooter_scale - 1.0)
                    * seat_scale
                    * left_scale_factor(urgency_scale)
                    * direction_scale;
            }

            if pass_has_ace && pass_premium_count <= 1 && remaining_mid_hearts >= 1 {
                let urgency_bias = 0.55 + 0.45 * urgency_scale;
                let remaining_scale = 1.4 + 0.35 * (remaining_mid_hearts as f32).min(4.0);
                let shooter_bias = 1.0 + 0.6 * (shooter_scale - 1.0).max(0.0);
                let passed_mid_bias = 0.8 + 0.3 * (passed_ten_plus as f32);
                penalty += input.weights.liability_base
                    * 3.8
                    * remaining_scale
                    * passed_mid_bias
                    * urgency_bias
                    * shooter_bias
                    * seat_scale
                    * direction_scale;
            }

            if pass_has_ace && passed_heart_count <= 1 && urgency >= 0.6 {
                let shooter_bias = 1.0 + 1.25 * (shooter_scale - 1.0).max(0.0);
                let urgency_bias = 0.75 + 0.25 * urgency_scale;
                let off_suit_bias = 1.0 + 0.75 * (passed_off_suit as f32);
                penalty += input.weights.liability_base
                    * 16.0
                    * shooter_bias
                    * urgency_bias
                    * off_suit_bias
                    * seat_scale
                    * direction_scale;
            }

            if pass_has_ace && pass_premium_count == 0 && urgency >= 0.6 {
                let shooter_bias = 1.0 + 1.4 * (shooter_scale - 1.0).max(0.0);
                let urgency_bias = 0.78 + 0.22 * urgency_scale;
                let low_bias = 1.0 + 0.45 * (remaining_low_hearts as f32).min(4.0);
                let off_suit_bias =
                    1.0 + 0.8 * (passed_off_suit as f32 + (1 - passed_heart_count) as f32);
                penalty += input.weights.liability_base
                    * 32.0
                    * shooter_bias
                    * urgency_bias
                    * low_bias
                    * off_suit_bias
                    * seat_scale
                    * direction_scale;
            }

            if pass_has_king
                && pass_premium_count == 1
                && passed_support_hearts <= 1
                && urgency >= 0.6
            {
                let support_deficit = (1usize.saturating_sub(passed_support_hearts)) as f32 + 0.5;
                let shooter_bias = 1.0 + 1.5 * (shooter_scale - 1.0).max(0.0);
                let urgency_bias = 0.78 + 0.22 * urgency_scale;
                let off_suit_bias = 1.0 + 0.75 * (passed_off_suit as f32);
                let remainder_bias = 1.0 + 0.45 * (remaining_low_hearts as f32).min(4.0);
                penalty += input.weights.liability_base
                    * 26.0
                    * support_deficit
                    * shooter_bias
                    * urgency_bias
                    * off_suit_bias
                    * remainder_bias
                    * seat_scale
                    * direction_scale;
            }

            if pass_has_king && pass_premium_count == 1 && passed_support_hearts == 0 {
                let shooter_bias = 1.0 + 0.7 * (shooter_scale - 1.0).max(0.0);
                let urgency_bias = 0.6 + 0.4 * urgency_scale;
                penalty += input.weights.liability_base
                    * 5.8
                    * (passed_low_hearts as f32 + 1.0)
                    * shooter_bias
                    * urgency_bias
                    * seat_scale
                    * direction_scale;
            }

            if pass_has_king && pass_premium_count == 1 && passed_support_hearts >= 1 {
                let shooter_bias = 1.0 + 0.75 * (shooter_scale - 1.0).max(0.0);
                let urgency_bias = 0.55 + 0.45 * urgency_scale;
                let support_bias = 1.0 + 0.25 * (passed_support_hearts as f32);
                penalty += input.weights.liability_base
                    * 4.2
                    * support_bias
                    * shooter_bias
                    * urgency_bias
                    * seat_scale
                    * direction_scale;
            }

            if pass_has_king
                && passed_off_suit >= 1
                && remaining_low_hearts >= passed_low_hearts + 1
                && urgency >= 0.6
            {
                let shooter_bias = 1.0 + 1.2 * (shooter_scale - 1.0).max(0.0);
                let urgency_bias = 0.7 + 0.3 * urgency_scale;
                let remaining_bias = 1.0 + 0.5 * (remaining_low_hearts as f32).min(4.0);
                penalty += input.weights.liability_base
                    * 10.2
                    * (passed_off_suit as f32 + 1.0)
                    * shooter_bias
                    * urgency_bias
                    * remaining_bias
                    * seat_scale
                    * direction_scale;
            }

            if pass_has_ace
                && pass_premium_count == 1
                && passed_support_hearts == 0
                && passed_low_hearts >= 1
            {
                let shooter_bias = 1.0 + 0.9 * (shooter_scale - 1.0).max(0.0);
                let urgency_bias = 0.74 + 0.26 * urgency_scale;
                let remaining_scale = 1.4 + 0.55 * (remaining_mid_hearts as f32).min(4.0);
                penalty += input.weights.liability_base
                    * 8.4
                    * (passed_low_hearts as f32 + 1.4)
                    * remaining_scale
                    * shooter_bias
                    * urgency_bias
                    * seat_scale
                    * direction_scale;
            }
        }
    }

    if urgency >= 0.55 && remaining_mid_hearts >= 2 {
        let deficit =
            ((remaining_mid_hearts as isize) - (passed_mid_hearts as isize)).max(1) as f32;
        let rank_pressure = remaining
            .iter()
            .filter(|card| card.suit == Suit::Hearts && card.rank >= Rank::Ten)
            .map(|card| card.rank.value() as f32)
            .sum::<f32>()
            .max(20.0);
        let mid_scale = 0.45 + 0.55 * urgency_scale;
        penalty += input.weights.liability_base
            * 0.9
            * mid_scale
            * deficit
            * (rank_pressure / 40.0)
            * direction_scale;
    }

    if let Some(rem_rank) = highest_remaining_heart {
        if rem_rank >= Rank::Queen.value() as f32 && rem_rank > highest_passed_heart {
            let rank_gap = (rem_rank - highest_passed_heart.max(12.0)).max(1.0);
            let remaining_mass = remaining_high_hearts.len() as f32;
            let handoff_scale = 0.45 + 0.55 * urgency_scale;
            let mass_scale = 1.0 + 0.25 * remaining_mass;
            penalty += input.weights.liability_base
                * 0.8
                * rank_gap
                * handoff_scale
                * mass_scale
                * direction_scale;
        }
    }

    penalty
}

fn build_pool(
    singles: &[(Card, PassScoreBreakdown)],
    config: &PassOptimizerConfig,
) -> Vec<(Card, PassScoreBreakdown)> {
    let mut pool = Vec::with_capacity(config.max_card_pool);
    for &(card, breakdown) in singles.iter() {
        if pool.len() >= config.max_card_pool {
            break;
        }
        if breakdown.total >= config.min_single_score || pool.len() < 3 {
            pool.push((card, breakdown));
        }
    }

    if pool.len() < 3 {
        pool.extend(
            singles
                .iter()
                .skip(pool.len())
                .take(3 - pool.len())
                .copied(),
        );
    }

    ensure_key_cards_present(&mut pool, singles, config.max_card_pool);
    pool
}

fn ensure_key_cards_present(
    pool: &mut Vec<(Card, PassScoreBreakdown)>,
    singles: &[(Card, PassScoreBreakdown)],
    max_card_pool: usize,
) {
    if !pool.iter().any(|(card, _)| card.is_queen_of_spades()) {
        if let Some(entry) = singles.iter().find(|(card, _)| card.is_queen_of_spades()) {
            if pool.len() < max_card_pool {
                pool.push(*entry);
            } else if let Some(idx) = find_lowest_score_index(pool) {
                pool[idx] = *entry;
            }
        }
    }

    // Ensure at least one high heart candidate for moon-blocking considerations.
    if !pool
        .iter()
        .any(|(card, _)| card.suit == Suit::Hearts && card.rank >= Rank::Queen)
    {
        if let Some(entry) = singles
            .iter()
            .find(|(card, _)| card.suit == Suit::Hearts && card.rank >= Rank::Queen)
        {
            if pool.len() < max_card_pool {
                pool.push(*entry);
            } else if let Some(idx) = find_lowest_score_index(pool) {
                pool[idx] = *entry;
            }
        }
    }
}

fn find_lowest_score_index(pool: &[(Card, PassScoreBreakdown)]) -> Option<usize> {
    pool.iter()
        .enumerate()
        .min_by(|(_, a), (_, b)| a.1.total.total_cmp(&b.1.total))
        .map(|(idx, _)| idx)
}

fn contains_liability_combo(combo: &[(Card, PassScoreBreakdown); 3]) -> bool {
    combo.iter().any(|(card, _)| {
        card.is_queen_of_spades()
            || (card.suit == Suit::Hearts && card.rank >= Rank::King)
            || (card.suit == Suit::Spades && card.rank >= Rank::King)
    })
}

fn left_shooter_pressure(input: &PassScoreInput<'_>) -> f32 {
    if !matches!(input.direction, PassingDirection::Left) {
        return 1.0;
    }
    let Some(belief) = input.belief else {
        return 1.05;
    };
    let target = input.direction.target(input.seat);
    let pressure: f32 = [Rank::Ace, Rank::King, Rank::Queen, Rank::Jack, Rank::Ten]
        .iter()
        .map(|rank| belief.prob_card(target, Card::new(*rank, Suit::Hearts)))
        .sum();

    (1.0 + pressure * 1.3).min(2.5)
}

fn left_scale_factor(urgency_scale: f32) -> f32 {
    0.5 + 0.5 * urgency_scale
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::belief::Belief;
    use crate::model::deck::Deck;
    use crate::model::hand::Hand;
    use crate::model::passing::{PassingDirection, PassingState};
    use crate::model::player::PlayerPosition;
    use crate::model::rank::Rank;
    use crate::model::round::{RoundPhase, RoundState};
    use crate::model::score::ScoreBoard;
    use crate::moon::{MoonEstimate, MoonEstimator, MoonFeatures, MoonObjective};
    use crate::pass::direction::DirectionProfile;
    use crate::pass::score_card_components;
    use crate::pass::scoring::PassWeights;

    fn make_input<'a>(round: &'a RoundState, belief: Option<&'a Belief>) -> PassScoreInput<'a> {
        let scores = Box::leak(Box::new(ScoreBoard::new()));
        let moon_estimator = MoonEstimator::default();
        let moon_features =
            MoonFeatures::from_state(PlayerPosition::North, round, scores, PassingDirection::Left);
        let moon_estimate = moon_estimator.estimate(moon_features);
        PassScoreInput {
            seat: PlayerPosition::North,
            hand: round.hand(PlayerPosition::North),
            round,
            scores,
            belief,
            weights: PassWeights::default(),
            direction: PassingDirection::Left,
            direction_profile: DirectionProfile::from_direction(PassingDirection::Left),
            moon_estimate,
        }
    }

    #[test]
    fn returns_candidates_even_without_belief() {
        let deck = Deck::shuffled_with_seed(99);
        let round = RoundState::deal(&deck, PlayerPosition::North, PassingDirection::Left);
        let input = make_input(&round, None);
        let candidates = enumerate_pass_triples(&input);
        assert!(!candidates.is_empty());
    }

    #[test]
    fn prioritises_queen_of_spades_combos() {
        let seat = PlayerPosition::North;
        let passing = PassingDirection::Left;
        let mut hands = [Hand::new(), Hand::new(), Hand::new(), Hand::new()];
        hands[seat.index()] = Hand::with_cards(vec![
            Card::new(Rank::Queen, Suit::Spades),
            Card::new(Rank::King, Suit::Spades),
            Card::new(Rank::Ace, Suit::Spades),
            Card::new(Rank::King, Suit::Hearts),
            Card::new(Rank::Queen, Suit::Hearts),
            Card::new(Rank::Jack, Suit::Hearts),
            Card::new(Rank::Ten, Suit::Hearts),
            Card::new(Rank::Two, Suit::Clubs),
            Card::new(Rank::Three, Suit::Clubs),
            Card::new(Rank::Four, Suit::Clubs),
            Card::new(Rank::Five, Suit::Diamonds),
            Card::new(Rank::Six, Suit::Diamonds),
            Card::new(Rank::Seven, Suit::Diamonds),
        ]);
        let round = RoundState::from_hands(
            hands,
            seat,
            passing,
            RoundPhase::Passing(PassingState::new(passing)),
        );
        let input = make_input(&round, None);
        let candidates = enumerate_pass_triples(&input);
        assert!(!candidates.is_empty());
        let best = candidates.first().unwrap();
        assert!(best.cards.iter().any(|card| card.is_queen_of_spades()));
    }

    #[test]
    fn respects_candidate_limit_when_configured() {
        let deck = Deck::shuffled_with_seed(321);
        let round = RoundState::deal(&deck, PlayerPosition::West, PassingDirection::Right);
        let input = make_input(&round, None);
        let config = PassOptimizerConfig {
            max_card_pool: 6,
            max_candidates: 5,
            min_single_score: -100.0,
        };
        let candidates = enumerate_pass_triples_with_config(&input, &config);
        assert!(candidates.len() <= 5);
    }

    #[test]
    fn build_pool_respects_limits_and_keeps_key_cards() {
        let config = PassOptimizerConfig {
            max_card_pool: 5,
            max_candidates: 5,
            min_single_score: 40.0,
        };

        let singles = vec![
            (
                Card::new(Rank::Two, Suit::Clubs),
                PassScoreBreakdown {
                    total: 80.0,
                    ..PassScoreBreakdown::default()
                },
            ),
            (
                Card::new(Rank::Three, Suit::Diamonds),
                PassScoreBreakdown {
                    total: 70.0,
                    ..PassScoreBreakdown::default()
                },
            ),
            (
                Card::new(Rank::Four, Suit::Clubs),
                PassScoreBreakdown {
                    total: 65.0,
                    ..PassScoreBreakdown::default()
                },
            ),
            (
                Card::new(Rank::Queen, Suit::Spades),
                PassScoreBreakdown {
                    total: -80.0,
                    ..PassScoreBreakdown::default()
                },
            ),
            (
                Card::new(Rank::King, Suit::Hearts),
                PassScoreBreakdown {
                    total: -75.0,
                    ..PassScoreBreakdown::default()
                },
            ),
        ];

        let pool = super::build_pool(&singles, &config);
        assert!(
            pool.len() <= config.max_card_pool,
            "pool len {} exceeds {}",
            pool.len(),
            config.max_card_pool
        );
        assert!(
            pool.iter().any(|(card, _)| card.is_queen_of_spades()),
            "queen of spades must be present: {:?}",
            pool
        );
        assert!(
            pool.iter()
                .any(|(card, _)| card.suit == Suit::Hearts && card.rank >= Rank::Queen),
            "high heart must be present: {:?}",
            pool
        );
    }

    #[test]
    fn high_threshold_still_returns_candidates() {
        let seat = PlayerPosition::North;
        let passing = PassingDirection::Left;
        let mut hands = [Hand::new(), Hand::new(), Hand::new(), Hand::new()];
        hands[seat.index()] = Hand::with_cards(vec![
            Card::new(Rank::Queen, Suit::Spades),
            Card::new(Rank::King, Suit::Spades),
            Card::new(Rank::Ace, Suit::Spades),
            Card::new(Rank::Ace, Suit::Hearts),
            Card::new(Rank::King, Suit::Hearts),
            Card::new(Rank::Queen, Suit::Hearts),
            Card::new(Rank::Two, Suit::Clubs),
            Card::new(Rank::Three, Suit::Clubs),
            Card::new(Rank::Four, Suit::Clubs),
            Card::new(Rank::Five, Suit::Diamonds),
            Card::new(Rank::Six, Suit::Diamonds),
            Card::new(Rank::Seven, Suit::Diamonds),
            Card::new(Rank::Eight, Suit::Diamonds),
        ]);
        let round = RoundState::from_hands(
            hands,
            seat,
            passing,
            RoundPhase::Passing(PassingState::new(passing)),
        );
        let input = make_input(&round, None);
        let config = PassOptimizerConfig {
            max_card_pool: 4,
            max_candidates: 6,
            min_single_score: 1_000.0,
        };
        let candidates = enumerate_pass_triples_with_config(&input, &config);
        assert!(
            !candidates.is_empty(),
            "expected fallback candidates even with high threshold"
        );
    }

    #[test]
    fn handshake_penalty_applies_when_high_hearts_are_kept() {
        let seat = PlayerPosition::West;
        let passing = PassingDirection::Left;
        let mut hands = [Hand::new(), Hand::new(), Hand::new(), Hand::new()];
        hands[seat.index()] = Hand::with_cards(vec![
            Card::new(Rank::Queen, Suit::Spades),
            Card::new(Rank::Ace, Suit::Hearts),
            Card::new(Rank::King, Suit::Hearts),
            Card::new(Rank::Queen, Suit::Hearts),
            Card::new(Rank::Jack, Suit::Hearts),
            Card::new(Rank::Ten, Suit::Hearts),
            Card::new(Rank::Two, Suit::Clubs),
            Card::new(Rank::Three, Suit::Clubs),
            Card::new(Rank::Four, Suit::Clubs),
            Card::new(Rank::Five, Suit::Diamonds),
            Card::new(Rank::Six, Suit::Diamonds),
            Card::new(Rank::Seven, Suit::Diamonds),
            Card::new(Rank::Eight, Suit::Diamonds),
        ]);

        let round = RoundState::from_hands(
            hands,
            seat,
            passing,
            RoundPhase::Passing(PassingState::new(passing)),
        );
        let scores = ScoreBoard::new();
        let moon_estimate = MoonEstimate {
            probability: 0.82,
            raw_score: 1.4,
            objective: MoonObjective::BlockShooter,
        };

        let input = PassScoreInput {
            seat,
            hand: round.hand(seat),
            round: &round,
            scores: &scores,
            belief: None,
            weights: PassWeights::default(),
            direction: passing,
            direction_profile: DirectionProfile::from_direction(passing),
            moon_estimate,
        };

        let candidates = enumerate_pass_triples(&input);
        assert!(
            !candidates.is_empty(),
            "expected pass candidates in handshake penalty scenario"
        );

        let mut penalty_detected = false;
        let mut min_penalty_with_high = f32::MAX;
        let mut min_penalty_without_high = f32::MAX;

        for candidate in &candidates {
            let high_hearts_passed = candidate
                .cards
                .iter()
                .filter(|card| card.suit == Suit::Hearts && card.rank >= Rank::Queen)
                .count();
            if high_hearts_passed == 0 && candidate.moon_liability_penalty > 0.1 {
                penalty_detected = true;
                min_penalty_without_high =
                    min_penalty_without_high.min(candidate.moon_liability_penalty);
            }
            if high_hearts_passed >= 1 {
                min_penalty_with_high = min_penalty_with_high.min(candidate.moon_liability_penalty);
            }
        }

        if !penalty_detected {
            assert!(
                candidates.iter().all(|candidate| candidate
                    .cards
                    .iter()
                    .any(|card| card.suit == Suit::Hearts && card.rank >= Rank::Queen)),
                "expected either penalties or that all candidates pass high hearts"
            );
        }
        assert!(
            min_penalty_with_high.is_finite(),
            "expected at least one high-heart candidate"
        );
        assert!(
            min_penalty_with_high < min_penalty_without_high,
            "passing a high heart should reduce penalty (with={} without={})",
            min_penalty_with_high,
            min_penalty_without_high
        );
    }

    #[test]
    fn handshake_prefers_passing_high_heart_combo() {
        let seat = PlayerPosition::West;
        let passing = PassingDirection::Across;
        let mut hands = [Hand::new(), Hand::new(), Hand::new(), Hand::new()];
        hands[seat.index()] = Hand::with_cards(vec![
            Card::new(Rank::Queen, Suit::Spades),
            Card::new(Rank::Ace, Suit::Hearts),
            Card::new(Rank::King, Suit::Hearts),
            Card::new(Rank::Queen, Suit::Hearts),
            Card::new(Rank::Two, Suit::Hearts),
            Card::new(Rank::Three, Suit::Hearts),
            Card::new(Rank::Four, Suit::Hearts),
            Card::new(Rank::Five, Suit::Hearts),
            Card::new(Rank::Two, Suit::Clubs),
            Card::new(Rank::Three, Suit::Clubs),
            Card::new(Rank::Four, Suit::Clubs),
            Card::new(Rank::Five, Suit::Diamonds),
            Card::new(Rank::Six, Suit::Diamonds),
        ]);

        let round = RoundState::from_hands(
            hands,
            seat,
            passing,
            RoundPhase::Passing(PassingState::new(passing)),
        );
        let scores = ScoreBoard::new();
        let moon_estimate = MoonEstimate {
            probability: 0.78,
            raw_score: 1.1,
            objective: MoonObjective::BlockShooter,
        };

        let input = PassScoreInput {
            seat,
            hand: round.hand(seat),
            round: &round,
            scores: &scores,
            belief: None,
            weights: PassWeights::default(),
            direction: passing,
            direction_profile: DirectionProfile::from_direction(passing),
            moon_estimate,
        };

        let queen_heart = Card::new(Rank::Queen, Suit::Hearts);
        let king_heart = Card::new(Rank::King, Suit::Hearts);
        let ace_heart = Card::new(Rank::Ace, Suit::Hearts);
        let candidates = enumerate_pass_triples(&input);
        let queen_breakdown = score_card_components(&input, queen_heart);
        let king_breakdown = score_card_components(&input, king_heart);
        let ace_breakdown = score_card_components(&input, ace_heart);
        let has_high_heart_combo = candidates.iter().any(|cand| {
            cand.cards
                .iter()
                .any(|card| card.suit == Suit::Hearts && card.rank >= Rank::Queen)
        });
        assert!(
            has_high_heart_combo,
            "expected at least one candidate that passes a high heart"
        );
        let best = candidates.first().expect("candidate exists");
        let high_hearts_passed = best
            .cards
            .iter()
            .filter(|card| card.suit == Suit::Hearts && card.rank >= Rank::Queen)
            .count();
        assert!(
            high_hearts_passed >= 1,
            "best combo should pass at least one high heart, found {:?}",
            best.cards
        );
        let mut one_high_penalty: Option<f32> = None;
        let mut two_high_penalty: Option<f32> = None;
        for candidate in &candidates {
            let count = candidate
                .cards
                .iter()
                .filter(|card| card.suit == Suit::Hearts && card.rank >= Rank::Queen)
                .count();
            match count {
                0 => {}
                1 => {
                    let value = candidate.moon_liability_penalty;
                    one_high_penalty = Some(one_high_penalty.map_or(value, |v| v.min(value)));
                }
                _ => {
                    let value = candidate.moon_liability_penalty;
                    two_high_penalty = Some(two_high_penalty.map_or(value, |v| v.min(value)));
                }
            }
        }
        if let (Some(two_high), Some(one_high)) = (two_high_penalty, one_high_penalty) {
            assert!(
                two_high < one_high,
                "passing two high hearts should incur lower penalty (two={two_high}, one={one_high})"
            );
        }
        assert!(
            queen_breakdown.total_score() > ace_breakdown.total_score(),
            "sanity check: queen hearts should score higher than ace for liability focus"
        );
        assert!(
            king_breakdown.total_score() > ace_breakdown.total_score() - 5.0,
            "king hearts should have competitive score for evaluation"
        );
    }

    #[test]
    fn penalty_triggers_when_passing_single_premium() {
        let seat = PlayerPosition::North;
        let passing = PassingDirection::Left;
        let mut hands = [Hand::new(), Hand::new(), Hand::new(), Hand::new()];
        hands[seat.index()] = Hand::with_cards(vec![
            Card::new(Rank::Ace, Suit::Hearts),
            Card::new(Rank::King, Suit::Hearts),
            Card::new(Rank::Queen, Suit::Hearts),
            Card::new(Rank::Jack, Suit::Hearts),
            Card::new(Rank::Ten, Suit::Hearts),
            Card::new(Rank::Nine, Suit::Hearts),
            Card::new(Rank::Two, Suit::Clubs),
            Card::new(Rank::Three, Suit::Clubs),
            Card::new(Rank::Four, Suit::Clubs),
            Card::new(Rank::Five, Suit::Diamonds),
            Card::new(Rank::Six, Suit::Diamonds),
            Card::new(Rank::Seven, Suit::Diamonds),
            Card::new(Rank::Eight, Suit::Diamonds),
        ]);

        let round = RoundState::from_hands(
            hands,
            seat,
            passing,
            RoundPhase::Passing(PassingState::new(passing)),
        );
        let scores = ScoreBoard::new();
        let moon_estimate = MoonEstimate {
            probability: 0.68,
            raw_score: 1.1,
            objective: MoonObjective::BlockShooter,
        };

        let input = PassScoreInput {
            seat,
            hand: round.hand(seat),
            round: &round,
            scores: &scores,
            belief: None,
            weights: PassWeights::default(),
            direction: passing,
            direction_profile: DirectionProfile::from_direction(passing),
            moon_estimate,
        };

        let ace = Card::new(Rank::Ace, Suit::Hearts);
        let jack = Card::new(Rank::Jack, Suit::Hearts);
        let ten = Card::new(Rank::Ten, Suit::Hearts);
        let combo = [
            (ace, score_card_components(&input, ace)),
            (jack, score_card_components(&input, jack)),
            (ten, score_card_components(&input, ten)),
        ];

        let penalty = super::moon_liability_penalty(&input, &combo);
        assert!(
            penalty > 0.0,
            "expected non-zero penalty when only one premium heart is passed"
        );
    }

    #[test]
    fn left_pass_multiple_premiums_penalised() {
        let seat = PlayerPosition::South;
        let passing = PassingDirection::Left;
        let mut hands = [Hand::new(), Hand::new(), Hand::new(), Hand::new()];
        hands[seat.index()] = Hand::with_cards(vec![
            Card::new(Rank::Ace, Suit::Hearts),
            Card::new(Rank::King, Suit::Hearts),
            Card::new(Rank::Queen, Suit::Hearts),
            Card::new(Rank::Ten, Suit::Hearts),
            Card::new(Rank::Nine, Suit::Hearts),
            Card::new(Rank::Eight, Suit::Hearts),
            Card::new(Rank::Two, Suit::Clubs),
            Card::new(Rank::Three, Suit::Clubs),
            Card::new(Rank::Four, Suit::Clubs),
            Card::new(Rank::Five, Suit::Diamonds),
            Card::new(Rank::Six, Suit::Diamonds),
            Card::new(Rank::Seven, Suit::Diamonds),
            Card::new(Rank::Jack, Suit::Spades),
        ]);

        let round = RoundState::from_hands(
            hands,
            seat,
            passing,
            RoundPhase::Passing(PassingState::new(passing)),
        );
        let scores = ScoreBoard::new();
        let moon_estimate = MoonEstimate {
            probability: 0.7,
            raw_score: 1.2,
            objective: MoonObjective::BlockShooter,
        };

        let input = PassScoreInput {
            seat,
            hand: round.hand(seat),
            round: &round,
            scores: &scores,
            belief: None,
            weights: PassWeights::default(),
            direction: passing,
            direction_profile: DirectionProfile::from_direction(passing),
            moon_estimate,
        };

        let candidates = enumerate_pass_triples(&input);
        let triple_premium = candidates
            .iter()
            .find(|cand| {
                cand.cards.contains(&Card::new(Rank::Ace, Suit::Hearts))
                    && cand.cards.contains(&Card::new(Rank::King, Suit::Hearts))
                    && cand.cards.contains(&Card::new(Rank::Queen, Suit::Hearts))
            })
            .expect("triple premium candidate");

        assert!(
            triple_premium.moon_liability_penalty > 0.0,
            "expected penalty when passing multiple premium hearts to the left"
        );
    }

    #[test]
    fn left_pass_ace_only_penalised() {
        let seat = PlayerPosition::North;
        let passing = PassingDirection::Left;
        let mut hands = [Hand::new(), Hand::new(), Hand::new(), Hand::new()];
        hands[seat.index()] = Hand::with_cards(vec![
            Card::new(Rank::Ace, Suit::Hearts),
            Card::new(Rank::Jack, Suit::Hearts),
            Card::new(Rank::Ten, Suit::Hearts),
            Card::new(Rank::Eight, Suit::Hearts),
            Card::new(Rank::Seven, Suit::Hearts),
            Card::new(Rank::Six, Suit::Hearts),
            Card::new(Rank::Two, Suit::Clubs),
            Card::new(Rank::Three, Suit::Clubs),
            Card::new(Rank::Four, Suit::Clubs),
            Card::new(Rank::Five, Suit::Diamonds),
            Card::new(Rank::Six, Suit::Diamonds),
            Card::new(Rank::Seven, Suit::Diamonds),
            Card::new(Rank::Eight, Suit::Diamonds),
        ]);

        let round = RoundState::from_hands(
            hands,
            seat,
            passing,
            RoundPhase::Passing(PassingState::new(passing)),
        );
        let scores = ScoreBoard::new();
        let moon_estimate = MoonEstimate {
            probability: 0.64,
            raw_score: 1.0,
            objective: MoonObjective::BlockShooter,
        };

        let input = PassScoreInput {
            seat,
            hand: round.hand(seat),
            round: &round,
            scores: &scores,
            belief: None,
            weights: PassWeights::default(),
            direction: passing,
            direction_profile: DirectionProfile::from_direction(passing),
            moon_estimate,
        };

        let ace = Card::new(Rank::Ace, Suit::Hearts);
        let three = Card::new(Rank::Three, Suit::Hearts);
        let four = Card::new(Rank::Four, Suit::Hearts);
        let combo = [
            (ace, score_card_components(&input, ace)),
            (three, score_card_components(&input, three)),
            (four, score_card_components(&input, four)),
        ];

        let penalty = super::moon_liability_penalty(&input, &combo);
        assert!(
            penalty > 0.0,
            "expected penalty when only the Ace of hearts moves to the left"
        );
    }

    #[test]
    fn left_pass_king_low_support_penalised() {
        let seat = PlayerPosition::North;
        let passing = PassingDirection::Left;
        let mut hands = [Hand::new(), Hand::new(), Hand::new(), Hand::new()];
        hands[seat.index()] = Hand::with_cards(vec![
            Card::new(Rank::King, Suit::Hearts),
            Card::new(Rank::Four, Suit::Hearts),
            Card::new(Rank::Three, Suit::Hearts),
            Card::new(Rank::Ten, Suit::Hearts),
            Card::new(Rank::Nine, Suit::Clubs),
            Card::new(Rank::Eight, Suit::Clubs),
            Card::new(Rank::Seven, Suit::Diamonds),
            Card::new(Rank::Six, Suit::Diamonds),
            Card::new(Rank::Five, Suit::Diamonds),
            Card::new(Rank::Four, Suit::Spades),
            Card::new(Rank::Three, Suit::Spades),
            Card::new(Rank::Two, Suit::Spades),
            Card::new(Rank::Two, Suit::Clubs),
        ]);

        let round = RoundState::from_hands(
            hands,
            seat,
            passing,
            RoundPhase::Passing(PassingState::new(passing)),
        );
        let scores = ScoreBoard::new();
        let moon_estimate = MoonEstimate {
            probability: 0.68,
            raw_score: 1.05,
            objective: MoonObjective::BlockShooter,
        };

        let input = PassScoreInput {
            seat,
            hand: round.hand(seat),
            round: &round,
            scores: &scores,
            belief: None,
            weights: PassWeights::default(),
            direction: passing,
            direction_profile: DirectionProfile::from_direction(passing),
            moon_estimate,
        };

        let king = Card::new(Rank::King, Suit::Hearts);
        let four = Card::new(Rank::Four, Suit::Hearts);
        let three = Card::new(Rank::Three, Suit::Hearts);
        let ten = Card::new(Rank::Ten, Suit::Hearts);
        let jack_hearts = Card::new(Rank::Jack, Suit::Hearts);
        let seven_club = Card::new(Rank::Seven, Suit::Clubs);

        let king_breakdown = score_card_components(&input, king);
        let four_breakdown = score_card_components(&input, four);
        let three_breakdown = score_card_components(&input, three);
        let ten_breakdown = score_card_components(&input, ten);
        let jack_hearts_breakdown = score_card_components(&input, jack_hearts);
        let club_breakdown = score_card_components(&input, seven_club);

        let low_support_combo = [
            (king, king_breakdown),
            (four, four_breakdown),
            (three, three_breakdown),
        ];
        let off_suit_combo = [
            (king, king_breakdown),
            (ten, ten_breakdown),
            (seven_club, club_breakdown),
        ];

        let compliant_combo = [
            (king, king_breakdown),
            (ten, ten_breakdown),
            (jack_hearts, jack_hearts_breakdown),
        ];

        let low_penalty = super::moon_liability_penalty(&input, &low_support_combo);
        let off_suit_penalty = super::moon_liability_penalty(&input, &off_suit_combo);
        let compliant_penalty = super::moon_liability_penalty(&input, &compliant_combo);
        assert!(
            low_penalty >= HARD_REJECTION_PENALTY,
            "expected hard rejection when king of hearts moves with only low hearts (penalty={low_penalty})"
        );
        assert!(
            off_suit_penalty >= HARD_REJECTION_PENALTY,
            "expected hard rejection when king of hearts moves with off-suit filler (penalty={off_suit_penalty})"
        );
        assert!(
            compliant_penalty < HARD_REJECTION_PENALTY,
            "expected finite penalty when king travels with two Ten+ hearts (penalty={compliant_penalty})"
        );
    }

    #[test]
    fn left_pass_king_with_support_penalised() {
        let seat = PlayerPosition::North;
        let passing = PassingDirection::Left;
        let mut hands = [Hand::new(), Hand::new(), Hand::new(), Hand::new()];
        hands[seat.index()] = Hand::with_cards(vec![
            Card::new(Rank::King, Suit::Hearts),
            Card::new(Rank::Ten, Suit::Hearts),
            Card::new(Rank::Three, Suit::Hearts),
            Card::new(Rank::Two, Suit::Hearts),
            Card::new(Rank::Ace, Suit::Clubs),
            Card::new(Rank::Queen, Suit::Diamonds),
            Card::new(Rank::Jack, Suit::Diamonds),
            Card::new(Rank::Nine, Suit::Spades),
            Card::new(Rank::Eight, Suit::Spades),
            Card::new(Rank::Seven, Suit::Spades),
            Card::new(Rank::Six, Suit::Spades),
            Card::new(Rank::Five, Suit::Spades),
            Card::new(Rank::Four, Suit::Spades),
        ]);

        let round = RoundState::from_hands(
            hands,
            seat,
            passing,
            RoundPhase::Passing(PassingState::new(passing)),
        );
        let scores = ScoreBoard::new();
        let moon_estimate = MoonEstimate {
            probability: 0.66,
            raw_score: 1.1,
            objective: MoonObjective::BlockShooter,
        };

        let input = PassScoreInput {
            seat,
            hand: round.hand(seat),
            round: &round,
            scores: &scores,
            belief: None,
            weights: PassWeights::default(),
            direction: passing,
            direction_profile: DirectionProfile::from_direction(passing),
            moon_estimate,
        };

        let king = Card::new(Rank::King, Suit::Hearts);
        let ten = Card::new(Rank::Ten, Suit::Hearts);
        let three = Card::new(Rank::Three, Suit::Hearts);
        let two = Card::new(Rank::Two, Suit::Hearts);

        let king_breakdown = score_card_components(&input, king);
        let ten_breakdown = score_card_components(&input, ten);
        let three_breakdown = score_card_components(&input, three);
        let two_breakdown = score_card_components(&input, two);

        let king_combo = [
            (king, king_breakdown),
            (ten, ten_breakdown),
            (two, two_breakdown),
        ];
        let no_king_combo = [
            (ten, ten_breakdown),
            (three, three_breakdown),
            (two, two_breakdown),
        ];

        let king_penalty = super::moon_liability_penalty(&input, &king_combo);
        let no_king_penalty = super::moon_liability_penalty(&input, &no_king_combo);
        assert!(
            king_penalty > no_king_penalty + input.weights.liability_base,
            "expected stronger penalty when king of hearts passes with supporting tens (king={king_penalty}, alt={no_king_penalty})"
        );
    }

    #[test]
    fn left_pass_king_offsuit_penalised_when_low_hearts_available() {
        let seat = PlayerPosition::South;
        let passing = PassingDirection::Left;
        let mut hands = [Hand::new(), Hand::new(), Hand::new(), Hand::new()];
        hands[seat.index()] = Hand::with_cards(vec![
            Card::new(Rank::King, Suit::Hearts),
            Card::new(Rank::Jack, Suit::Hearts),
            Card::new(Rank::Four, Suit::Hearts),
            Card::new(Rank::Three, Suit::Hearts),
            Card::new(Rank::Two, Suit::Hearts),
            Card::new(Rank::Ten, Suit::Clubs),
            Card::new(Rank::Ace, Suit::Spades),
            Card::new(Rank::Queen, Suit::Spades),
            Card::new(Rank::Jack, Suit::Spades),
            Card::new(Rank::Ten, Suit::Spades),
            Card::new(Rank::Nine, Suit::Diamonds),
            Card::new(Rank::Eight, Suit::Diamonds),
            Card::new(Rank::Seven, Suit::Diamonds),
        ]);

        let round = RoundState::from_hands(
            hands,
            seat,
            passing,
            RoundPhase::Passing(PassingState::new(passing)),
        );
        let scores = ScoreBoard::new();
        let moon_estimate = MoonEstimate {
            probability: 0.64,
            raw_score: 1.0,
            objective: MoonObjective::BlockShooter,
        };

        let input = PassScoreInput {
            seat,
            hand: round.hand(seat),
            round: &round,
            scores: &scores,
            belief: None,
            weights: PassWeights::default(),
            direction: passing,
            direction_profile: DirectionProfile::from_direction(passing),
            moon_estimate,
        };

        let king = Card::new(Rank::King, Suit::Hearts);
        let four = Card::new(Rank::Four, Suit::Hearts);
        let three = Card::new(Rank::Three, Suit::Hearts);
        let club_ten = Card::new(Rank::Ten, Suit::Clubs);

        let king_breakdown = score_card_components(&input, king);
        let four_breakdown = score_card_components(&input, four);
        let three_breakdown = score_card_components(&input, three);
        let club_breakdown = score_card_components(&input, club_ten);

        let offsuit_combo = [
            (king, king_breakdown),
            (four, four_breakdown),
            (club_ten, club_breakdown),
        ];
        let all_hearts_combo = [
            (king, king_breakdown),
            (four, four_breakdown),
            (three, three_breakdown),
        ];

        let offsuit_penalty = super::moon_liability_penalty(&input, &offsuit_combo);
        let all_hearts_penalty = super::moon_liability_penalty(&input, &all_hearts_combo);
        assert!(
            offsuit_penalty >= HARD_REJECTION_PENALTY,
            "expected hard rejection when king of hearts leaves with off-suit filler (penalty={offsuit_penalty})"
        );
        assert!(
            all_hearts_penalty >= HARD_REJECTION_PENALTY,
            "expected hard rejection when king of hearts leaves without enough Ten+ support (penalty={all_hearts_penalty})"
        );
    }

    #[test]
    fn left_pass_ace_with_low_hearts_penalised() {
        let seat = PlayerPosition::North;
        let passing = PassingDirection::Left;
        let mut hands = [Hand::new(), Hand::new(), Hand::new(), Hand::new()];
        hands[seat.index()] = Hand::with_cards(vec![
            Card::new(Rank::Ace, Suit::Hearts),
            Card::new(Rank::Four, Suit::Hearts),
            Card::new(Rank::Three, Suit::Hearts),
            Card::new(Rank::Ten, Suit::Hearts),
            Card::new(Rank::Jack, Suit::Hearts),
            Card::new(Rank::Nine, Suit::Clubs),
            Card::new(Rank::Eight, Suit::Clubs),
            Card::new(Rank::Seven, Suit::Diamonds),
            Card::new(Rank::Six, Suit::Diamonds),
            Card::new(Rank::Five, Suit::Diamonds),
            Card::new(Rank::Four, Suit::Spades),
            Card::new(Rank::Three, Suit::Spades),
            Card::new(Rank::Two, Suit::Spades),
        ]);

        let round = RoundState::from_hands(
            hands,
            seat,
            passing,
            RoundPhase::Passing(PassingState::new(passing)),
        );
        let scores = ScoreBoard::new();
        let moon_estimate = MoonEstimate {
            probability: 0.7,
            raw_score: 1.2,
            objective: MoonObjective::BlockShooter,
        };

        let input = PassScoreInput {
            seat,
            hand: round.hand(seat),
            round: &round,
            scores: &scores,
            belief: None,
            weights: PassWeights::default(),
            direction: passing,
            direction_profile: DirectionProfile::from_direction(passing),
            moon_estimate,
        };

        let ace = Card::new(Rank::Ace, Suit::Hearts);
        let four = Card::new(Rank::Four, Suit::Hearts);
        let three = Card::new(Rank::Three, Suit::Hearts);
        let ten = Card::new(Rank::Ten, Suit::Hearts);
        let jack = Card::new(Rank::Jack, Suit::Hearts);

        let ace_breakdown = score_card_components(&input, ace);
        let four_breakdown = score_card_components(&input, four);
        let three_breakdown = score_card_components(&input, three);
        let ten_breakdown = score_card_components(&input, ten);
        let jack_breakdown = score_card_components(&input, jack);

        let low_combo = [
            (ace, ace_breakdown),
            (four, four_breakdown),
            (three, three_breakdown),
        ];
        let supported_combo = [
            (ace, ace_breakdown),
            (ten, ten_breakdown),
            (jack, jack_breakdown),
        ];

        let low_penalty = super::moon_liability_penalty(&input, &low_combo);
        let supported_penalty = super::moon_liability_penalty(&input, &supported_combo);
        assert!(
            low_penalty >= HARD_REJECTION_PENALTY,
            "expected hard rejection when ace of hearts passes with only low support (penalty={low_penalty})"
        );
        assert!(
            supported_penalty < HARD_REJECTION_PENALTY,
            "expected finite penalty when ace travels with two Ten+ hearts (penalty={supported_penalty})"
        );
    }

    #[test]
    fn left_pass_ace_offsuit_penalised() {
        let seat = PlayerPosition::North;
        let passing = PassingDirection::Left;
        let mut hands = [Hand::new(), Hand::new(), Hand::new(), Hand::new()];
        hands[seat.index()] = Hand::with_cards(vec![
            Card::new(Rank::Ace, Suit::Hearts),
            Card::new(Rank::Queen, Suit::Clubs),
            Card::new(Rank::Jack, Suit::Clubs),
            Card::new(Rank::Ten, Suit::Clubs),
            Card::new(Rank::Nine, Suit::Spades),
            Card::new(Rank::Eight, Suit::Spades),
            Card::new(Rank::Seven, Suit::Spades),
            Card::new(Rank::Six, Suit::Spades),
            Card::new(Rank::Five, Suit::Diamonds),
            Card::new(Rank::Four, Suit::Diamonds),
            Card::new(Rank::Three, Suit::Diamonds),
            Card::new(Rank::Two, Suit::Diamonds),
            Card::new(Rank::Two, Suit::Clubs),
        ]);

        let round = RoundState::from_hands(
            hands,
            seat,
            passing,
            RoundPhase::Passing(PassingState::new(passing)),
        );
        let mut belief = Belief::new_uninitialized(seat);
        let target = passing.target(seat);
        belief.scale_suit_for_seat(target, Suit::Hearts, 7.5);

        let scores = ScoreBoard::new();
        let moon_estimate = MoonEstimate {
            probability: 0.7,
            raw_score: 1.25,
            objective: MoonObjective::BlockShooter,
        };

        let input = PassScoreInput {
            seat,
            hand: round.hand(seat),
            round: &round,
            scores: &scores,
            belief: Some(&belief),
            weights: PassWeights::default(),
            direction: passing,
            direction_profile: DirectionProfile::from_direction(passing),
            moon_estimate,
        };

        let ace = Card::new(Rank::Ace, Suit::Hearts);
        let queen_club = Card::new(Rank::Queen, Suit::Clubs);
        let jack_club = Card::new(Rank::Jack, Suit::Clubs);
        let ten_club = Card::new(Rank::Ten, Suit::Clubs);

        let ace_breakdown = score_card_components(&input, ace);
        let queen_breakdown = score_card_components(&input, queen_club);
        let jack_breakdown = score_card_components(&input, jack_club);
        let ten_breakdown = score_card_components(&input, ten_club);

        let ace_offsuit_combo = [
            (ace, ace_breakdown),
            (queen_club, queen_breakdown),
            (jack_club, jack_breakdown),
        ];
        let offsuit_only_combo = [
            (queen_club, queen_breakdown),
            (jack_club, jack_breakdown),
            (ten_club, ten_breakdown),
        ];

        let ace_penalty = super::moon_liability_penalty(&input, &ace_offsuit_combo);
        let offsuit_penalty = super::moon_liability_penalty(&input, &offsuit_only_combo);
        assert!(
            ace_penalty >= HARD_REJECTION_PENALTY,
            "expected hard rejection when ace departs with only off-suit fillers (penalty={ace_penalty})"
        );
        assert!(
            offsuit_penalty < HARD_REJECTION_PENALTY,
            "expected finite penalty when ace is retained and only off-suit cards move (penalty={offsuit_penalty})"
        );
    }

    #[test]
    fn left_pass_ace_single_support_penalised() {
        let seat = PlayerPosition::North;
        let passing = PassingDirection::Left;
        let mut hands = [Hand::new(), Hand::new(), Hand::new(), Hand::new()];
        hands[seat.index()] = Hand::with_cards(vec![
            Card::new(Rank::Ace, Suit::Hearts),
            Card::new(Rank::Ten, Suit::Hearts),
            Card::new(Rank::Three, Suit::Hearts),
            Card::new(Rank::Queen, Suit::Spades),
            Card::new(Rank::Jack, Suit::Spades),
            Card::new(Rank::Ten, Suit::Spades),
            Card::new(Rank::Nine, Suit::Diamonds),
            Card::new(Rank::Eight, Suit::Diamonds),
            Card::new(Rank::Seven, Suit::Diamonds),
            Card::new(Rank::Six, Suit::Diamonds),
            Card::new(Rank::Five, Suit::Clubs),
            Card::new(Rank::Four, Suit::Clubs),
            Card::new(Rank::Three, Suit::Clubs),
        ]);

        let round = RoundState::from_hands(
            hands,
            seat,
            passing,
            RoundPhase::Passing(PassingState::new(passing)),
        );
        let scores = ScoreBoard::new();
        let moon_estimate = MoonEstimate {
            probability: 0.68,
            raw_score: 1.2,
            objective: MoonObjective::BlockShooter,
        };

        let input = PassScoreInput {
            seat,
            hand: round.hand(seat),
            round: &round,
            scores: &scores,
            belief: None,
            weights: PassWeights::default(),
            direction: passing,
            direction_profile: DirectionProfile::from_direction(passing),
            moon_estimate,
        };

        let ace = Card::new(Rank::Ace, Suit::Hearts);
        let ten = Card::new(Rank::Ten, Suit::Hearts);
        let three = Card::new(Rank::Three, Suit::Hearts);
        let queen_spade = Card::new(Rank::Queen, Suit::Spades);

        let ace_breakdown = score_card_components(&input, ace);
        let ten_breakdown = score_card_components(&input, ten);
        let three_breakdown = score_card_components(&input, three);
        let queen_breakdown = score_card_components(&input, queen_spade);

        let ace_combo = [
            (ace, ace_breakdown),
            (ten, ten_breakdown),
            (three, three_breakdown),
        ];
        let alt_combo = [
            (ten, ten_breakdown),
            (three, three_breakdown),
            (queen_spade, queen_breakdown),
        ];

        let ace_penalty = super::moon_liability_penalty(&input, &ace_combo);
        let alt_penalty = super::moon_liability_penalty(&input, &alt_combo);
        assert!(
            ace_penalty >= HARD_REJECTION_PENALTY,
            "expected hard rejection when ace leaves with only one supporting Ten+ heart (penalty={ace_penalty})"
        );
        assert!(
            alt_penalty < HARD_REJECTION_PENALTY,
            "expected finite penalty when ace is retained (penalty={alt_penalty})"
        );
    }

    #[test]
    fn left_pass_king_single_support_penalised() {
        let seat = PlayerPosition::East;
        let passing = PassingDirection::Left;
        let mut hands = [Hand::new(), Hand::new(), Hand::new(), Hand::new()];
        hands[seat.index()] = Hand::with_cards(vec![
            Card::new(Rank::King, Suit::Hearts),
            Card::new(Rank::Jack, Suit::Hearts),
            Card::new(Rank::Four, Suit::Hearts),
            Card::new(Rank::Ace, Suit::Clubs),
            Card::new(Rank::Queen, Suit::Clubs),
            Card::new(Rank::Jack, Suit::Clubs),
            Card::new(Rank::Ten, Suit::Clubs),
            Card::new(Rank::Nine, Suit::Spades),
            Card::new(Rank::Eight, Suit::Spades),
            Card::new(Rank::Seven, Suit::Spades),
            Card::new(Rank::Six, Suit::Spades),
            Card::new(Rank::Five, Suit::Diamonds),
            Card::new(Rank::Four, Suit::Diamonds),
        ]);

        let round = RoundState::from_hands(
            hands,
            seat,
            passing,
            RoundPhase::Passing(PassingState::new(passing)),
        );
        let scores = ScoreBoard::new();
        let moon_estimate = MoonEstimate {
            probability: 0.7,
            raw_score: 1.3,
            objective: MoonObjective::BlockShooter,
        };

        let input = PassScoreInput {
            seat,
            hand: round.hand(seat),
            round: &round,
            scores: &scores,
            belief: None,
            weights: PassWeights::default(),
            direction: passing,
            direction_profile: DirectionProfile::from_direction(passing),
            moon_estimate,
        };

        let king = Card::new(Rank::King, Suit::Hearts);
        let jack = Card::new(Rank::Jack, Suit::Hearts);
        let four = Card::new(Rank::Four, Suit::Hearts);
        let ace_club = Card::new(Rank::Ace, Suit::Clubs);

        let king_breakdown = score_card_components(&input, king);
        let jack_breakdown = score_card_components(&input, jack);
        let four_breakdown = score_card_components(&input, four);
        let ace_breakdown = score_card_components(&input, ace_club);

        let king_combo = [
            (king, king_breakdown),
            (jack, jack_breakdown),
            (four, four_breakdown),
        ];
        let alt_combo = [
            (jack, jack_breakdown),
            (four, four_breakdown),
            (ace_club, ace_breakdown),
        ];

        let king_penalty = super::moon_liability_penalty(&input, &king_combo);
        let alt_penalty = super::moon_liability_penalty(&input, &alt_combo);
        assert!(
            king_penalty >= HARD_REJECTION_PENALTY,
            "expected hard rejection when king leaves with only one Ten+ heart (penalty={king_penalty})"
        );
        assert!(
            alt_penalty < HARD_REJECTION_PENALTY,
            "expected finite penalty when king is retained (penalty={alt_penalty})"
        );
    }

    #[test]
    fn enumerate_passes_require_support_when_premium_present() {
        let seat = PlayerPosition::West;
        let passing = PassingDirection::Left;
        let mut hands = [Hand::new(), Hand::new(), Hand::new(), Hand::new()];
        hands[seat.index()] = Hand::with_cards(vec![
            Card::new(Rank::Ace, Suit::Hearts),
            Card::new(Rank::King, Suit::Hearts),
            Card::new(Rank::Queen, Suit::Hearts),
            Card::new(Rank::Jack, Suit::Hearts),
            Card::new(Rank::Ten, Suit::Hearts),
            Card::new(Rank::Nine, Suit::Hearts),
            Card::new(Rank::Eight, Suit::Clubs),
            Card::new(Rank::Seven, Suit::Clubs),
            Card::new(Rank::Six, Suit::Clubs),
            Card::new(Rank::Five, Suit::Diamonds),
            Card::new(Rank::Four, Suit::Diamonds),
            Card::new(Rank::Three, Suit::Spades),
            Card::new(Rank::Two, Suit::Spades),
        ]);

        let round = RoundState::from_hands(
            hands,
            seat,
            passing,
            RoundPhase::Passing(PassingState::new(passing)),
        );
        let scores = ScoreBoard::new();
        let moon_estimate = MoonEstimate {
            probability: 0.7,
            raw_score: 1.2,
            objective: MoonObjective::BlockShooter,
        };

        let input = PassScoreInput {
            seat,
            hand: round.hand(seat),
            round: &round,
            scores: &scores,
            belief: None,
            weights: PassWeights::default(),
            direction: passing,
            direction_profile: DirectionProfile::from_direction(passing),
            moon_estimate,
        };

        for candidate in enumerate_pass_triples(&input) {
            let has_premium = candidate.cards.iter().any(|card| {
                card.suit == Suit::Hearts && matches!(card.rank, Rank::Ace | Rank::King)
            });
            if has_premium {
                let support = candidate
                    .cards
                    .iter()
                    .filter(|card| card.suit == Suit::Hearts && card.rank >= Rank::Ten)
                    .count();
                assert!(
                    support >= 3,
                    "expected at least three Ten+ hearts when passing premium hearts: {:?}",
                    candidate.cards
                );
            }
        }
    }

    #[test]
    fn ace_removed_when_support_insufficient() {
        let seat = PlayerPosition::North;
        let passing = PassingDirection::Left;
        let mut hands = [Hand::new(), Hand::new(), Hand::new(), Hand::new()];
        hands[seat.index()] = Hand::with_cards(vec![
            Card::new(Rank::Ace, Suit::Hearts),
            Card::new(Rank::Ten, Suit::Hearts),
            Card::new(Rank::Three, Suit::Hearts),
            Card::new(Rank::Two, Suit::Hearts),
            Card::new(Rank::Queen, Suit::Spades),
            Card::new(Rank::Jack, Suit::Spades),
            Card::new(Rank::Ten, Suit::Spades),
            Card::new(Rank::Nine, Suit::Diamonds),
            Card::new(Rank::Eight, Suit::Diamonds),
            Card::new(Rank::Seven, Suit::Diamonds),
            Card::new(Rank::Six, Suit::Diamonds),
            Card::new(Rank::Five, Suit::Clubs),
            Card::new(Rank::Four, Suit::Clubs),
        ]);

        let round = RoundState::from_hands(
            hands,
            seat,
            passing,
            RoundPhase::Passing(PassingState::new(passing)),
        );
        let scores = ScoreBoard::new();
        let moon_estimate = MoonEstimate {
            probability: 0.72,
            raw_score: 1.25,
            objective: MoonObjective::BlockShooter,
        };

        let input = PassScoreInput {
            seat,
            hand: round.hand(seat),
            round: &round,
            scores: &scores,
            belief: None,
            weights: PassWeights::default(),
            direction: passing,
            direction_profile: DirectionProfile::from_direction(passing),
            moon_estimate,
        };

        let combos = enumerate_pass_triples(&input);
        assert!(combos.iter().all(|candidate| {
            !candidate
                .cards
                .iter()
                .any(|card| card.suit == Suit::Hearts && card.rank == Rank::Ace)
        }));
    }

    #[test]
    fn king_removed_when_support_insufficient() {
        let seat = PlayerPosition::East;
        let passing = PassingDirection::Left;
        let mut hands = [Hand::new(), Hand::new(), Hand::new(), Hand::new()];
        hands[seat.index()] = Hand::with_cards(vec![
            Card::new(Rank::King, Suit::Hearts),
            Card::new(Rank::Jack, Suit::Hearts),
            Card::new(Rank::Four, Suit::Hearts),
            Card::new(Rank::Ace, Suit::Clubs),
            Card::new(Rank::Queen, Suit::Clubs),
            Card::new(Rank::Jack, Suit::Clubs),
            Card::new(Rank::Ten, Suit::Clubs),
            Card::new(Rank::Nine, Suit::Spades),
            Card::new(Rank::Eight, Suit::Spades),
            Card::new(Rank::Seven, Suit::Spades),
            Card::new(Rank::Six, Suit::Spades),
            Card::new(Rank::Five, Suit::Diamonds),
            Card::new(Rank::Four, Suit::Diamonds),
        ]);

        let round = RoundState::from_hands(
            hands,
            seat,
            passing,
            RoundPhase::Passing(PassingState::new(passing)),
        );
        let scores = ScoreBoard::new();
        let moon_estimate = MoonEstimate {
            probability: 0.7,
            raw_score: 1.3,
            objective: MoonObjective::BlockShooter,
        };

        let input = PassScoreInput {
            seat,
            hand: round.hand(seat),
            round: &round,
            scores: &scores,
            belief: None,
            weights: PassWeights::default(),
            direction: passing,
            direction_profile: DirectionProfile::from_direction(passing),
            moon_estimate,
        };

        let combos = enumerate_pass_triples(&input);
        assert!(combos.iter().all(|candidate| {
            !candidate
                .cards
                .iter()
                .any(|card| card.suit == Suit::Hearts && card.rank == Rank::King)
        }));
    }

    #[test]
    fn left_pass_single_premium_penalised_under_pressure() {
        let seat = PlayerPosition::North;
        let passing = PassingDirection::Left;
        let mut hands = [Hand::new(), Hand::new(), Hand::new(), Hand::new()];
        hands[seat.index()] = Hand::with_cards(vec![
            Card::new(Rank::King, Suit::Hearts),
            Card::new(Rank::Ten, Suit::Hearts),
            Card::new(Rank::Two, Suit::Hearts),
            Card::new(Rank::Ace, Suit::Clubs),
            Card::new(Rank::Queen, Suit::Clubs),
            Card::new(Rank::Jack, Suit::Clubs),
            Card::new(Rank::Ten, Suit::Clubs),
            Card::new(Rank::Nine, Suit::Diamonds),
            Card::new(Rank::Eight, Suit::Diamonds),
            Card::new(Rank::Seven, Suit::Diamonds),
            Card::new(Rank::Six, Suit::Spades),
            Card::new(Rank::Five, Suit::Spades),
            Card::new(Rank::Four, Suit::Spades),
        ]);

        let round = RoundState::from_hands(
            hands,
            seat,
            passing,
            RoundPhase::Passing(PassingState::new(passing)),
        );
        let mut belief = Belief::new_uninitialized(seat);
        let target = passing.target(seat);
        belief.scale_suit_for_seat(target, Suit::Hearts, 8.0);

        let scores = ScoreBoard::new();
        let moon_estimate = MoonEstimate {
            probability: 0.68,
            raw_score: 1.1,
            objective: MoonObjective::BlockShooter,
        };

        let input = PassScoreInput {
            seat,
            hand: round.hand(seat),
            round: &round,
            scores: &scores,
            belief: Some(&belief),
            weights: PassWeights::default(),
            direction: passing,
            direction_profile: DirectionProfile::from_direction(passing),
            moon_estimate,
        };

        let king = Card::new(Rank::King, Suit::Hearts);
        let ten_heart = Card::new(Rank::Ten, Suit::Hearts);
        let two_heart = Card::new(Rank::Two, Suit::Hearts);
        let jack_club = Card::new(Rank::Jack, Suit::Clubs);

        let king_breakdown = score_card_components(&input, king);
        let ten_breakdown = score_card_components(&input, ten_heart);
        let two_breakdown = score_card_components(&input, two_heart);
        let jack_breakdown = score_card_components(&input, jack_club);

        let king_combo = [
            (king, king_breakdown),
            (ten_heart, ten_breakdown),
            (two_heart, two_breakdown),
        ];
        let alt_combo = [
            (ten_heart, ten_breakdown),
            (two_heart, two_breakdown),
            (jack_club, jack_breakdown),
        ];

        let king_penalty = super::moon_liability_penalty(&input, &king_combo);
        let alt_penalty = super::moon_liability_penalty(&input, &alt_combo);
        assert!(
            king_penalty > alt_penalty + input.weights.liability_base,
            "expected stronger penalty when only the king plus minimal support passes left (king={king_penalty}, alt={alt_penalty})"
        );
    }

    #[test]
    fn left_pass_multiple_premiums_strongly_penalised_under_pressure() {
        let seat = PlayerPosition::South;
        let passing = PassingDirection::Left;
        let mut hands = [Hand::new(), Hand::new(), Hand::new(), Hand::new()];
        hands[seat.index()] = Hand::with_cards(vec![
            Card::new(Rank::Ace, Suit::Hearts),
            Card::new(Rank::King, Suit::Hearts),
            Card::new(Rank::Queen, Suit::Hearts),
            Card::new(Rank::Ten, Suit::Hearts),
            Card::new(Rank::Nine, Suit::Hearts),
            Card::new(Rank::Eight, Suit::Hearts),
            Card::new(Rank::Two, Suit::Clubs),
            Card::new(Rank::Three, Suit::Clubs),
            Card::new(Rank::Four, Suit::Clubs),
            Card::new(Rank::Five, Suit::Diamonds),
            Card::new(Rank::Six, Suit::Diamonds),
            Card::new(Rank::Seven, Suit::Diamonds),
            Card::new(Rank::Jack, Suit::Spades),
        ]);

        let round = RoundState::from_hands(
            hands,
            seat,
            passing,
            RoundPhase::Passing(PassingState::new(passing)),
        );
        let mut belief = Belief::new_uninitialized(seat);
        let target = passing.target(seat);
        belief.scale_suit_for_seat(target, Suit::Hearts, 6.0);

        let scores = ScoreBoard::new();
        let moon_estimate = MoonEstimate {
            probability: 0.72,
            raw_score: 1.3,
            objective: MoonObjective::BlockShooter,
        };

        let input = PassScoreInput {
            seat,
            hand: round.hand(seat),
            round: &round,
            scores: &scores,
            belief: Some(&belief),
            weights: PassWeights::default(),
            direction: passing,
            direction_profile: DirectionProfile::from_direction(passing),
            moon_estimate,
        };

        let ace = Card::new(Rank::Ace, Suit::Hearts);
        let king = Card::new(Rank::King, Suit::Hearts);
        let eight = Card::new(Rank::Eight, Suit::Hearts);
        let queen = Card::new(Rank::Queen, Suit::Hearts);
        let club_two = Card::new(Rank::Two, Suit::Clubs);

        let ace_breakdown = score_card_components(&input, ace);
        let king_breakdown = score_card_components(&input, king);
        let eight_breakdown = score_card_components(&input, eight);
        let queen_breakdown = score_card_components(&input, queen);
        let club_breakdown = score_card_components(&input, club_two);

        let double_premium_combo = [
            (ace, ace_breakdown),
            (king, king_breakdown),
            (club_two, club_breakdown),
        ];
        let split_combo = [
            (ace, ace_breakdown),
            (eight, eight_breakdown),
            (club_two, club_breakdown),
        ];

        let shooter_pressure = super::left_shooter_pressure(&input);
        assert!(
            shooter_pressure >= 1.25,
            "expected elevated shooter pressure, got {shooter_pressure}"
        );

        let high_hearts_count = double_premium_combo
            .iter()
            .filter(|(card, _)| card.suit == Suit::Hearts && card.rank >= Rank::Queen)
            .count() as f32;
        let severity = 1.0 + (high_hearts_count - 1.0).max(0.0);
        let shooter_threshold = 1.25_f32;
        let shooter_bias = 1.0 + (shooter_pressure - shooter_threshold).max(0.0);
        let ace_bias = if double_premium_combo
            .iter()
            .any(|(card, _)| card.suit == Suit::Hearts && card.rank == Rank::Ace)
        {
            1.3
        } else {
            1.0
        };
        let seat_multiplier = match seat {
            PlayerPosition::North | PlayerPosition::South => 1.35,
            PlayerPosition::East | PlayerPosition::West => 1.2,
        };
        let forced_penalty = input.weights.liability_base
            * 180.0
            * severity
            * shooter_bias
            * ace_bias
            * seat_multiplier;
        assert!(
            forced_penalty > 400.0,
            "expected forced penalty scaling > 400, got {forced_penalty}"
        );

        let high_penalty = super::moon_liability_penalty(&input, &double_premium_combo);
        let split_penalty = super::moon_liability_penalty(&input, &split_combo);
        let compliant_combo = [
            (ace, ace_breakdown),
            (king, king_breakdown),
            (queen, queen_breakdown),
        ];
        let compliant_penalty = super::moon_liability_penalty(&input, &compliant_combo);
        assert!(
            high_penalty >= HARD_REJECTION_PENALTY && split_penalty >= HARD_REJECTION_PENALTY,
            "expected hard rejection when multiple premium hearts move without two additional Ten+ hearts (double={high_penalty}, split={split_penalty})"
        );
        assert!(
            compliant_penalty < HARD_REJECTION_PENALTY,
            "expected finite penalty when multiple premium hearts include sufficient Ten+ support (penalty={compliant_penalty})"
        );
    }

    #[test]
    fn left_pass_ace_only_penalty_scales_with_remaining_tens() {
        let passing = PassingDirection::Left;
        let seat = PlayerPosition::North;

        // Scenario A: Ten-plus hearts remain after passing only the Ace.
        let mut hands_high = [Hand::new(), Hand::new(), Hand::new(), Hand::new()];
        hands_high[seat.index()] = Hand::with_cards(vec![
            Card::new(Rank::Ace, Suit::Hearts),
            Card::new(Rank::Ten, Suit::Hearts),
            Card::new(Rank::Nine, Suit::Hearts),
            Card::new(Rank::Eight, Suit::Hearts),
            Card::new(Rank::Seven, Suit::Hearts),
            Card::new(Rank::Six, Suit::Hearts),
            Card::new(Rank::Two, Suit::Clubs),
            Card::new(Rank::Three, Suit::Clubs),
            Card::new(Rank::Four, Suit::Clubs),
            Card::new(Rank::Five, Suit::Diamonds),
            Card::new(Rank::Six, Suit::Diamonds),
            Card::new(Rank::Seven, Suit::Diamonds),
            Card::new(Rank::Eight, Suit::Diamonds),
        ]);
        let round_high = RoundState::from_hands(
            hands_high,
            seat,
            passing,
            RoundPhase::Passing(PassingState::new(passing)),
        );
        let scores_high = ScoreBoard::new();
        let moon_estimate = MoonEstimate {
            probability: 0.66,
            raw_score: 1.05,
            objective: MoonObjective::BlockShooter,
        };
        let input_high = PassScoreInput {
            seat,
            hand: round_high.hand(seat),
            round: &round_high,
            scores: &scores_high,
            belief: None,
            weights: PassWeights::default(),
            direction: passing,
            direction_profile: DirectionProfile::from_direction(passing),
            moon_estimate,
        };

        let ace = Card::new(Rank::Ace, Suit::Hearts);
        let three = Card::new(Rank::Three, Suit::Hearts);
        let four = Card::new(Rank::Four, Suit::Hearts);

        let high_combo = [
            (ace, score_card_components(&input_high, ace)),
            (three, score_card_components(&input_high, three)),
            (four, score_card_components(&input_high, four)),
        ];
        let penalty_with_tens = super::moon_liability_penalty(&input_high, &high_combo);

        // Scenario B: No Ten-plus hearts remain after passing the Ace.
        let mut hands_low = [Hand::new(), Hand::new(), Hand::new(), Hand::new()];
        hands_low[seat.index()] = Hand::with_cards(vec![
            Card::new(Rank::Ace, Suit::Hearts),
            Card::new(Rank::Nine, Suit::Hearts),
            Card::new(Rank::Eight, Suit::Hearts),
            Card::new(Rank::Seven, Suit::Hearts),
            Card::new(Rank::Six, Suit::Hearts),
            Card::new(Rank::Five, Suit::Hearts),
            Card::new(Rank::Two, Suit::Clubs),
            Card::new(Rank::Three, Suit::Clubs),
            Card::new(Rank::Four, Suit::Clubs),
            Card::new(Rank::Five, Suit::Diamonds),
            Card::new(Rank::Six, Suit::Diamonds),
            Card::new(Rank::Seven, Suit::Diamonds),
            Card::new(Rank::Eight, Suit::Diamonds),
        ]);
        let round_low = RoundState::from_hands(
            hands_low,
            seat,
            passing,
            RoundPhase::Passing(PassingState::new(passing)),
        );
        let scores_low = ScoreBoard::new();
        let input_low = PassScoreInput {
            seat,
            hand: round_low.hand(seat),
            round: &round_low,
            scores: &scores_low,
            belief: None,
            weights: PassWeights::default(),
            direction: passing,
            direction_profile: DirectionProfile::from_direction(passing),
            moon_estimate,
        };
        let low_combo = [
            (ace, score_card_components(&input_low, ace)),
            (three, score_card_components(&input_low, three)),
            (four, score_card_components(&input_low, four)),
        ];
        let penalty_without_tens = super::moon_liability_penalty(&input_low, &low_combo);

        assert!(
            penalty_with_tens >= HARD_REJECTION_PENALTY,
            "expected hard rejection when Ace-only pass leaves additional Ten+ hearts (penalty={penalty_with_tens})"
        );
        assert!(
            penalty_without_tens >= HARD_REJECTION_PENALTY,
            "expected hard rejection when Ace-only pass leaves no extra Ten+ hearts (penalty={penalty_without_tens})"
        );
    }

    #[test]
    fn left_pass_mid_hearts_penalised() {
        let seat = PlayerPosition::East;
        let passing = PassingDirection::Left;
        let mut hands = [Hand::new(), Hand::new(), Hand::new(), Hand::new()];
        hands[seat.index()] = Hand::with_cards(vec![
            Card::new(Rank::Jack, Suit::Hearts),
            Card::new(Rank::Ten, Suit::Hearts),
            Card::new(Rank::Nine, Suit::Hearts),
            Card::new(Rank::Eight, Suit::Hearts),
            Card::new(Rank::Seven, Suit::Hearts),
            Card::new(Rank::Six, Suit::Hearts),
            Card::new(Rank::Two, Suit::Clubs),
            Card::new(Rank::Three, Suit::Clubs),
            Card::new(Rank::Four, Suit::Clubs),
            Card::new(Rank::Five, Suit::Diamonds),
            Card::new(Rank::Six, Suit::Diamonds),
            Card::new(Rank::Seven, Suit::Diamonds),
            Card::new(Rank::Eight, Suit::Diamonds),
        ]);

        let round = RoundState::from_hands(
            hands,
            seat,
            passing,
            RoundPhase::Passing(PassingState::new(passing)),
        );
        let scores = ScoreBoard::new();
        let moon_estimate = MoonEstimate {
            probability: 0.6,
            raw_score: 0.85,
            objective: MoonObjective::BlockShooter,
        };

        let input = PassScoreInput {
            seat,
            hand: round.hand(seat),
            round: &round,
            scores: &scores,
            belief: None,
            weights: PassWeights::default(),
            direction: passing,
            direction_profile: DirectionProfile::from_direction(passing),
            moon_estimate,
        };

        let jack = Card::new(Rank::Jack, Suit::Hearts);
        let ten = Card::new(Rank::Ten, Suit::Hearts);
        let nine = Card::new(Rank::Nine, Suit::Hearts);
        let combo = [
            (jack, score_card_components(&input, jack)),
            (ten, score_card_components(&input, ten)),
            (nine, score_card_components(&input, nine)),
        ];

        let penalty = super::moon_liability_penalty(&input, &combo);
        assert!(
            penalty > 0.0,
            "expected penalty when only mid hearts are passed to the left"
        );
    }
}
