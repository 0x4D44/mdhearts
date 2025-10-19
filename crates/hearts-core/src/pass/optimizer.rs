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
    let mut low_priority = Vec::new();
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

                while let Some(adjusted) = combos_to_eval.pop() {
                    if violates_ten_plus_safety(input, &adjusted) {
                        continue;
                    }
                    if violates_support_guard(input, &adjusted) {
                        continue;
                    }
                    let missing_ten_plus = missing_ten_plus_support(input, &adjusted);
                    if missing_ten_plus {
                        let ten_plus_replacements = build_ten_plus_replacements(input, &adjusted);
                        if !ten_plus_replacements.is_empty() {
                            let scored = score_replacement_combos(input, ten_plus_replacements);
                            combos_to_eval.extend(scored);
                            continue;
                        }
                        let single_heart_upgrades =
                            build_single_heart_ten_plus_replacements(input, &adjusted);
                        if !single_heart_upgrades.is_empty() {
                            let scored = score_replacement_combos(input, single_heart_upgrades);
                            combos_to_eval.extend(scored);
                            continue;
                        }
                        continue;
                    }
                    let eval = evaluate_combo(input, &adjusted);
                    if eval.moon_penalty >= HARD_REJECTION_PENALTY {
                        continue;
                    }
                    let is_triple_low_hearts = adjusted
                        .iter()
                        .filter(|(card, _)| card.suit == Suit::Hearts)
                        .count()
                        == 3
                        && adjusted
                            .iter()
                            .filter(|(card, _)| card.suit == Suit::Hearts && card.rank >= Rank::Ten)
                            .count()
                            <= 1;
                    let low_heart_pair = adjusted
                        .iter()
                        .filter(|(card, _)| card.suit == Suit::Hearts)
                        .count()
                        >= 2
                        && adjusted
                            .iter()
                            .filter(|(card, _)| card.suit == Suit::Hearts && card.rank >= Rank::Ten)
                            .count()
                            == 0;
                    if eval.total < 0.5 && !contains_liability_combo(&adjusted) {
                        let candidate_cards = [adjusted[0].0, adjusted[1].0, adjusted[2].0];
                        if low_priority
                            .iter()
                            .any(|existing: &PassCandidate| existing.cards == candidate_cards)
                            || candidates
                                .iter()
                                .any(|existing: &PassCandidate| existing.cards == candidate_cards)
                        {
                            continue;
                        }
                        low_priority.push(PassCandidate {
                            cards: [adjusted[0].0, adjusted[1].0, adjusted[2].0],
                            score: eval.total,
                            void_score: eval.void_sum,
                            liability_score: eval.liability_sum,
                            moon_score: eval.moon_sum,
                            synergy: eval.synergy,
                            direction_bonus: eval.direction_bonus,
                            moon_liability_penalty: eval.moon_penalty,
                        });
                        continue;
                    }
                    if violates_all_offsuit_guard(input, &adjusted) {
                        let candidate_cards = [adjusted[0].0, adjusted[1].0, adjusted[2].0];
                        if low_priority
                            .iter()
                            .any(|existing: &PassCandidate| existing.cards == candidate_cards)
                            || candidates
                                .iter()
                                .any(|existing: &PassCandidate| existing.cards == candidate_cards)
                        {
                            continue;
                        }
                        low_priority.push(PassCandidate {
                            cards: candidate_cards,
                            score: eval.total,
                            void_score: eval.void_sum,
                            liability_score: eval.liability_sum,
                            moon_score: eval.moon_sum,
                            synergy: eval.synergy,
                            direction_bonus: eval.direction_bonus,
                            moon_liability_penalty: eval.moon_penalty,
                        });
                        continue;
                    }
                    if is_triple_low_hearts {
                        let candidate_cards = [adjusted[0].0, adjusted[1].0, adjusted[2].0];
                        if low_priority
                            .iter()
                            .any(|existing: &PassCandidate| existing.cards == candidate_cards)
                            || candidates
                                .iter()
                                .any(|existing: &PassCandidate| existing.cards == candidate_cards)
                        {
                            continue;
                        }
                        low_priority.push(PassCandidate {
                            cards: candidate_cards,
                            score: eval.total,
                            void_score: eval.void_sum,
                            liability_score: eval.liability_sum,
                            moon_score: eval.moon_sum,
                            synergy: eval.synergy,
                            direction_bonus: eval.direction_bonus,
                            moon_liability_penalty: eval.moon_penalty,
                        });
                        continue;
                    }
                    if low_heart_pair {
                        let candidate_cards = [adjusted[0].0, adjusted[1].0, adjusted[2].0];
                        if low_priority
                            .iter()
                            .any(|existing: &PassCandidate| existing.cards == candidate_cards)
                            || candidates
                                .iter()
                                .any(|existing: &PassCandidate| existing.cards == candidate_cards)
                        {
                            continue;
                        }
                        low_priority.push(PassCandidate {
                            cards: candidate_cards,
                            score: eval.total,
                            void_score: eval.void_sum,
                            liability_score: eval.liability_sum,
                            moon_score: eval.moon_sum,
                            synergy: eval.synergy,
                            direction_bonus: eval.direction_bonus,
                            moon_liability_penalty: eval.moon_penalty,
                        });
                        continue;
                    }
                    let candidate_cards = [adjusted[0].0, adjusted[1].0, adjusted[2].0];
                    if candidates
                        .iter()
                        .any(|existing: &PassCandidate| existing.cards == candidate_cards)
                    {
                        continue;
                    }
                    candidates.push(PassCandidate {
                        cards: candidate_cards,
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
    if candidates.is_empty() && !low_priority.is_empty() {
        low_priority.sort_by(|a, b| b.score.total_cmp(&a.score));
        candidates.extend(low_priority.into_iter().take(config.max_candidates));
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
    let pass_has_qspade = combo.iter().any(|(card, _)| card.is_queen_of_spades());
    let pass_has_qheart = combo
        .iter()
        .any(|(card, _)| card.suit == Suit::Hearts && card.rank == Rank::Queen);
    let pass_has_jheart = combo
        .iter()
        .any(|(card, _)| card.suit == Suit::Hearts && card.rank == Rank::Jack);
    let hearts_in_combo = combo
        .iter()
        .filter(|(card, _)| card.suit == Suit::Hearts)
        .count();
    let liability_offsuit = combo
        .iter()
        .filter(|(card, _)| is_offsuit_liability(card))
        .count();
    let base_guard = pass_has_ace || pass_has_king || pass_has_qspade;
    let passed_ten_plus = combo
        .iter()
        .filter(|(card, _)| card.suit == Suit::Hearts && card.rank >= Rank::Ten)
        .count();
    let total_ten_plus_in_hand = input
        .hand
        .iter()
        .filter(|card| card.suit == Suit::Hearts && card.rank >= Rank::Ten)
        .count();
    let ten_plus_remaining_after = total_ten_plus_in_hand.saturating_sub(passed_ten_plus);
    let premium_passed = combo
        .iter()
        .filter(|(card, _)| card.suit == Suit::Hearts && card.rank >= Rank::Queen)
        .count();
    let passed_support_hearts = combo
        .iter()
        .filter(|(card, _)| {
            card.suit == Suit::Hearts && card.rank >= Rank::Ten && card.rank < Rank::Queen
        })
        .count();
    let passed_mid_support_hearts = combo
        .iter()
        .filter(|(card, _)| card.suit == Suit::Hearts && is_mid_support_heart(card))
        .count();
    let support_total_in_combo = passed_support_hearts + passed_mid_support_hearts;
    let support_needed = if pass_has_qheart {
        2
    } else if pass_has_jheart {
        1
    } else {
        0
    };
    if !combo.iter().any(|(card, _)| card.suit == Suit::Hearts) {
        let off_guard = build_offsuit_heart_replacements(input, combo);
        if !off_guard.is_empty() {
            return PremiumSupportResolution::Replacements(score_replacement_combos(
                input, off_guard,
            ));
        }
    }

    if pass_has_ace && passed_ten_plus < 3 {
        let ace_alternatives = build_ace_guard_replacements(input, combo, total_ten_plus_in_hand);
        if !ace_alternatives.is_empty() {
            return PremiumSupportResolution::Replacements(score_replacement_combos(
                input,
                ace_alternatives,
            ));
        }
    }

    if pass_has_ace && ten_plus_remaining_after < 2 {
        let ace_alternatives = build_ace_guard_replacements(input, combo, total_ten_plus_in_hand);
        if !ace_alternatives.is_empty() {
            return PremiumSupportResolution::Replacements(score_replacement_combos(
                input,
                ace_alternatives,
            ));
        } else {
            return PremiumSupportResolution::Discard;
        }
    }

    if passed_ten_plus >= 3
        || (premium_passed >= 2 && (ten_plus_remaining_after < 2 || passed_support_hearts == 0))
    {
        let premium_alternatives = build_premium_split_replacements(input, combo);
        if !premium_alternatives.is_empty() {
            return PremiumSupportResolution::Replacements(score_replacement_combos(
                input,
                premium_alternatives,
            ));
        }
        let demoted = build_premium_demote_replacements(input, combo);
        if !demoted.is_empty() {
            return PremiumSupportResolution::Replacements(score_replacement_combos(
                input, demoted,
            ));
        }
        return PremiumSupportResolution::Valid;
    }

    let raw_required = 3usize.saturating_sub(passed_ten_plus);
    struct HeartOption {
        card: Card,
        breakdown: PassScoreBreakdown,
        is_support: bool,
        is_mid: bool,
    }

    let mut available_any: Vec<HeartOption> = input
        .hand
        .iter()
        .filter_map(|card| {
            if card.suit != Suit::Hearts {
                return None;
            }
            if combo.iter().any(|(existing, _)| existing == card) {
                return None;
            }
            let breakdown = score_single_card(input, *card);
            Some(HeartOption {
                card: *card,
                breakdown,
                is_support: is_high_support_heart(card),
                is_mid: is_mid_support_heart(card),
            })
        })
        .collect();

    let available_support_high = available_any
        .iter()
        .filter(|entry| entry.is_support)
        .count();
    let available_mid_support = available_any.iter().filter(|entry| entry.is_mid).count();
    let available_total_support = available_support_high + available_mid_support;
    let total_support_capacity = support_total_in_combo + available_total_support;
    let has_offsuit = combo.iter().any(|(card, _)| card.suit != Suit::Hearts);
    if pass_has_qheart
        && has_offsuit
        && support_total_in_combo <= 1
        && available_total_support == 0
        && ten_plus_remaining_after >= 2
        && total_support_capacity <= 1
    {
        return PremiumSupportResolution::Valid;
    }
    let liability_anchor = pass_has_qheart
        && !pass_has_ace
        && support_total_in_combo == 1
        && liability_offsuit >= 1
        && ten_plus_remaining_after >= 1
        && total_support_capacity <= 2
        && (available_total_support == 0 || passed_mid_support_hearts > 0);
    let supportless_liability = pass_has_qheart
        && !pass_has_ace
        && support_total_in_combo == 0
        && total_support_capacity == 0
        && liability_offsuit >= 1
        && hearts_in_combo >= 2
        && ten_plus_remaining_after >= 1;
    let supportless_premium = pass_has_qheart
        && support_total_in_combo == 1
        && available_total_support == 0
        && ten_plus_remaining_after >= 2;
    if liability_anchor || supportless_liability || supportless_premium {
        return PremiumSupportResolution::Valid;
    }
    let needs_support = support_needed > 0;
    if needs_support && total_support_capacity < support_needed {
        return PremiumSupportResolution::Discard;
    }

    if pass_has_qheart
        && available_total_support == 0
        && support_total_in_combo == 0
        && !supportless_liability
    {
        return PremiumSupportResolution::Discard;
    }
    let pass_has_no_hearts = combo.iter().all(|(card, _)| card.suit != Suit::Hearts);
    let pass_has_only_low_hearts = combo.iter().any(|(card, _)| card.suit == Suit::Hearts)
        && combo
            .iter()
            .all(|(card, _)| card.suit != Suit::Hearts || card.rank < Rank::Ten);
    let requires_support_injection = needs_support && support_total_in_combo < support_needed;
    let support_capacity = support_total_in_combo + available_total_support;
    let guard_support_threshold = support_needed.max(2);
    let requires_guard = base_guard
        || (pass_has_qheart && support_capacity >= support_needed)
        || (pass_has_jheart && support_capacity >= support_needed)
        || (pass_has_only_low_hearts && available_total_support > 0)
        || (pass_has_no_hearts
            && available_any.len() >= 2
            && support_capacity >= guard_support_threshold);
    let requires_guard = requires_guard || requires_support_injection;
    if !requires_guard {
        return PremiumSupportResolution::Valid;
    }
    if available_any.is_empty() {
        return PremiumSupportResolution::Discard;
    }
    available_any.sort_by(|a, b| {
        let rank_order = b.card.rank.cmp(&a.card.rank);
        if rank_order != Ordering::Equal {
            return rank_order;
        }
        b.breakdown
            .total
            .partial_cmp(&a.breakdown.total)
            .unwrap_or(Ordering::Equal)
    });
    let required = raw_required.min(available_any.len());
    if required == 0 {
        return PremiumSupportResolution::Valid;
    }
    let mut replaceable: Vec<usize> = combo
        .iter()
        .enumerate()
        .filter(|(_, (card, _))| card.suit != Suit::Hearts || card.rank < Rank::Ten)
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

    let target_slots: Vec<usize> = replaceable.into_iter().take(required).collect();
    if target_slots.len() < required {
        return PremiumSupportResolution::Discard;
    }

    let base_heart_count = combo
        .iter()
        .filter(|(card, _)| card.suit == Suit::Hearts)
        .count();
    let target_hearts = (base_heart_count + available_any.len()).min(3);
    if target_hearts <= base_heart_count && base_heart_count < 3 {
        // No way to add additional hearts to reach the guard threshold.
        return PremiumSupportResolution::Discard;
    }
    let target_ten_plus = if needs_support {
        (passed_support_hearts + available_support_high).min(3)
    } else {
        (passed_ten_plus + available_support_high).min(3)
    };

    let heart_index_combos = choose_index_combinations(available_any.len(), target_slots.len());
    let mut replacements = Vec::new();
    for heart_indices in heart_index_combos {
        if heart_indices.len() != required {
            continue;
        }
        let mut adjusted = combo.clone();
        for (slot_idx, heart_idx) in target_slots.iter().zip(heart_indices.iter()) {
            if let Some(entry) = available_any.get(*heart_idx) {
                adjusted[*slot_idx] = (entry.card, entry.breakdown);
            }
        }
        let final_hearts = adjusted
            .iter()
            .filter(|(card, _)| card.suit == Suit::Hearts)
            .count();
        let ten_plus_count = adjusted
            .iter()
            .filter(|(card, _)| card.suit == Suit::Hearts && card.rank >= Rank::Ten)
            .count();
        if final_hearts < target_hearts {
            continue;
        }
        if ten_plus_count < target_ten_plus {
            continue;
        }
        if needs_support {
            let support_high = adjusted
                .iter()
                .filter(|(card, _)| is_high_support_heart(card))
                .count();
            let support_mid = adjusted
                .iter()
                .filter(|(card, _)| is_mid_support_heart(card))
                .count();
            if support_high + support_mid < support_needed {
                continue;
            }
        }
        if replacements
            .iter()
            .any(|existing: &[(Card, PassScoreBreakdown); 3]| {
                existing
                    .iter()
                    .map(|(card, _)| card)
                    .eq(adjusted.iter().map(|(card, _)| card))
            })
        {
            continue;
        }
        replacements.push(adjusted);
    }

    if replacements.is_empty() {
        if available_any.len() < required {
            return PremiumSupportResolution::Discard;
        }
        let mut forced = combo.clone();
        for (slot_idx, heart_entry) in target_slots.iter().zip(available_any.iter()) {
            forced[*slot_idx] = (heart_entry.card, heart_entry.breakdown);
        }
        replacements.push(forced);
    }

    PremiumSupportResolution::Replacements(replacements)
}

fn score_replacement_combos(
    input: &PassScoreInput<'_>,
    combos: Vec<[Card; 3]>,
) -> Vec<[(Card, PassScoreBreakdown); 3]> {
    combos
        .into_iter()
        .map(|cards| {
            [
                (cards[0], score_single_card(input, cards[0])),
                (cards[1], score_single_card(input, cards[1])),
                (cards[2], score_single_card(input, cards[2])),
            ]
        })
        .collect()
}

fn build_ace_guard_replacements(
    input: &PassScoreInput<'_>,
    combo: &[(Card, PassScoreBreakdown); 3],
    total_ten_plus_in_hand: usize,
) -> Vec<[Card; 3]> {
    if !matches!(input.direction, PassingDirection::Left) {
        return Vec::new();
    }
    let urgency = input.moon_estimate.defensive_urgency();
    if urgency < 0.6 {
        return Vec::new();
    }

    let combo_signature = combo_signature_from_pairs(combo);
    let contains_ace = combo
        .iter()
        .any(|(card, _)| card.suit == Suit::Hearts && card.rank == Rank::Ace);
    if !contains_ace {
        return Vec::new();
    }

    let premium_partners = combo.iter().any(|(card, _)| {
        card.suit == Suit::Hearts && (card.rank == Rank::King || card.rank == Rank::Queen)
    });
    let support_in_hand = input
        .hand
        .iter()
        .copied()
        .filter(|card| {
            card.suit == Suit::Hearts
                && card.rank >= Rank::Ten
                && card.rank < Rank::Queen
                && *card != Card::new(Rank::Ace, Suit::Hearts)
        })
        .count();

    if !premium_partners && total_ten_plus_in_hand >= 3 && support_in_hand >= 2 {
        return Vec::new();
    }

    let mut heart_candidates: Vec<Card> = input
        .hand
        .iter()
        .copied()
        .filter(|card| card.suit == Suit::Hearts && card.rank != Rank::Ace)
        .collect();

    if heart_candidates.len() < 2 {
        return Vec::new();
    }

    heart_candidates.sort_by(|a, b| b.rank.cmp(&a.rank));
    heart_candidates.dedup();

    let mut combos = Vec::new();
    if heart_candidates.len() >= 3 {
        let limit = heart_candidates.len().min(5);
        'outer: for i in 0..limit {
            for j in (i + 1)..limit {
                for k in (j + 1)..limit {
                    let candidate = [
                        heart_candidates[i],
                        heart_candidates[j],
                        heart_candidates[k],
                    ];
                    if combo_signature_cards(&candidate) == combo_signature {
                        continue;
                    }
                    push_unique_cards(&mut combos, candidate);
                    if combos.len() >= 6 {
                        break 'outer;
                    }
                }
            }
        }
    } else {
        let mut off_candidates: Vec<(Card, PassScoreBreakdown)> = input
            .hand
            .iter()
            .copied()
            .filter(|card| card.suit != Suit::Hearts)
            .map(|card| {
                let breakdown = score_single_card(input, card);
                (card, breakdown)
            })
            .collect();
        if off_candidates.is_empty() {
            return combos;
        }

        off_candidates.sort_by(|a, b| b.1.total.partial_cmp(&a.1.total).unwrap_or(Ordering::Equal));
        off_candidates.dedup_by(|a, b| a.0 == b.0);

        let off_limit = off_candidates.len().min(3);
        for idx in 0..off_limit {
            let candidate = [
                heart_candidates[0],
                heart_candidates[1],
                off_candidates[idx].0,
            ];
            if combo_signature_cards(&candidate) == combo_signature {
                continue;
            }
            push_unique_cards(&mut combos, candidate);
        }
    }

    combos
}

fn build_offsuit_heart_replacements(
    input: &PassScoreInput<'_>,
    combo: &[(Card, PassScoreBreakdown); 3],
) -> Vec<[Card; 3]> {
    if !matches!(input.direction, PassingDirection::Left) {
        return Vec::new();
    }
    let urgency = input.moon_estimate.defensive_urgency();
    if urgency < 0.6 {
        return Vec::new();
    }
    if combo.iter().any(|(card, _)| card.suit == Suit::Hearts) {
        return Vec::new();
    }

    let mut available_hearts: Vec<Card> = input
        .hand
        .iter()
        .copied()
        .filter(|card| card.suit == Suit::Hearts)
        .filter(|card| combo.iter().all(|(existing, _)| existing != card))
        .collect();
    if available_hearts.is_empty() {
        return Vec::new();
    }
    available_hearts.sort_by(|a, b| b.rank.cmp(&a.rank));

    let mut strong_hearts: Vec<Card> = available_hearts
        .iter()
        .copied()
        .filter(|card| card.rank >= Rank::Eight)
        .collect();
    if strong_hearts.is_empty() {
        strong_hearts = available_hearts.clone();
    }

    let mut off_indices: Vec<(usize, f32)> = combo
        .iter()
        .enumerate()
        .map(|(idx, (_, breakdown))| (idx, breakdown.total))
        .collect();
    off_indices.sort_by(|a, b| a.1.partial_cmp(&b.1).unwrap_or(Ordering::Equal));

    let target_idx = off_indices.first().map(|(idx, _)| *idx).unwrap_or(0);

    let mut combos = Vec::new();
    for heart in strong_hearts.iter().take(3) {
        let mut cards = [combo[0].0, combo[1].0, combo[2].0];
        cards[target_idx] = *heart;
        push_unique_cards(&mut combos, cards);
        if combos.len() >= 3 {
            break;
        }
    }
    combos
}

fn build_premium_demote_replacements(
    input: &PassScoreInput<'_>,
    combo: &[(Card, PassScoreBreakdown); 3],
) -> Vec<[Card; 3]> {
    if !matches!(input.direction, PassingDirection::Left) {
        return Vec::new();
    }
    let urgency = input.moon_estimate.defensive_urgency();
    if urgency < 0.6 {
        return Vec::new();
    }
    let mut premium_indices: Vec<usize> = combo
        .iter()
        .enumerate()
        .filter_map(|(idx, (card, _))| {
            if card.suit == Suit::Hearts && card.rank >= Rank::Queen {
                Some(idx)
            } else {
                None
            }
        })
        .collect();
    if premium_indices.len() < 2 {
        return Vec::new();
    }
    premium_indices.sort_by(|&a, &b| combo[b].0.rank.cmp(&combo[a].0.rank));
    let replace_slots: Vec<usize> = premium_indices.iter().skip(1).copied().collect();
    if replace_slots.is_empty() {
        return Vec::new();
    }

    let heart_pool: Vec<Card> = input
        .hand
        .iter()
        .copied()
        .filter(|card| {
            card.suit == Suit::Hearts && combo.iter().all(|(existing, _)| existing != card)
        })
        .collect();

    let mut strong_hearts: Vec<Card> = heart_pool
        .iter()
        .copied()
        .filter(|card| card.rank >= Rank::Eight && card.rank < Rank::Queen)
        .collect();
    strong_hearts.sort_by(|a, b| b.rank.cmp(&a.rank));

    let mut filler_hearts: Vec<Card> = heart_pool
        .iter()
        .copied()
        .filter(|card| card.rank < Rank::Eight)
        .collect();
    filler_hearts.sort_by(|a, b| b.rank.cmp(&a.rank));

    let mut off_candidates: Vec<(Card, PassScoreBreakdown)> = input
        .hand
        .iter()
        .copied()
        .filter(|card| card.suit != Suit::Hearts)
        .filter(|card| combo.iter().all(|(existing, _)| existing != card))
        .map(|card| {
            let breakdown = score_single_card(input, card);
            (card, breakdown)
        })
        .collect();
    off_candidates.sort_by(|a, b| b.1.total.partial_cmp(&a.1.total).unwrap_or(Ordering::Equal));

    let mut candidate_cards: Vec<Card> = Vec::new();
    candidate_cards.extend(strong_hearts.iter().copied().take(4));
    candidate_cards.extend(filler_hearts.iter().copied().take(2));
    candidate_cards.extend(off_candidates.iter().map(|(card, _)| *card).take(3));
    candidate_cards.retain(|card| combo.iter().all(|(existing, _)| existing != card));
    candidate_cards.sort_by(|a, b| b.rank.cmp(&a.rank));
    candidate_cards.dedup();
    if candidate_cards.len() < replace_slots.len() {
        return Vec::new();
    }

    let choose = replace_slots.len();
    let mut combos = Vec::new();
    let combo_signature = combo_signature_from_pairs(combo);
    for replacement_indices in choose_index_combinations(candidate_cards.len(), choose) {
        if replacement_indices.len() != choose {
            continue;
        }
        let mut adjusted = combo.clone();
        for (slot_idx, card_idx) in replace_slots.iter().zip(replacement_indices.iter()) {
            let card = candidate_cards[*card_idx];
            adjusted[*slot_idx] = (card, score_single_card(input, card));
        }
        let premium_remaining = adjusted
            .iter()
            .filter(|(card, _)| card.suit == Suit::Hearts && card.rank >= Rank::Queen)
            .count();
        if premium_remaining >= 2 {
            continue;
        }
        let heart_count = adjusted
            .iter()
            .filter(|(card, _)| card.suit == Suit::Hearts)
            .count();
        if heart_count == 0 {
            continue;
        }
        let cards_only = [adjusted[0].0, adjusted[1].0, adjusted[2].0];
        if combo_signature_cards(&cards_only) == combo_signature {
            continue;
        }
        push_unique_cards(&mut combos, cards_only);
        if combos.len() >= 6 {
            break;
        }
    }

    combos
}

fn build_ten_plus_replacements(
    input: &PassScoreInput<'_>,
    combo: &[(Card, PassScoreBreakdown); 3],
) -> Vec<[Card; 3]> {
    let hearts_in_combo: Vec<(usize, Card)> = combo
        .iter()
        .enumerate()
        .filter_map(|(idx, (card, _))| {
            if card.suit == Suit::Hearts {
                Some((idx, *card))
            } else {
                None
            }
        })
        .collect();
    if hearts_in_combo.len() < 2 {
        return Vec::new();
    }
    let ten_plus_remaining: Vec<Card> = input
        .hand
        .iter()
        .copied()
        .filter(|card| card.suit == Suit::Hearts && card.rank >= Rank::Ten)
        .filter(|card| combo.iter().all(|(existing, _)| existing != card))
        .collect();
    if ten_plus_remaining.is_empty() {
        return Vec::new();
    }

    let mut slots: Vec<(usize, Card)> = hearts_in_combo
        .into_iter()
        .filter(|(_, card)| card.rank < Rank::Ten)
        .collect();
    if slots.is_empty() {
        return Vec::new();
    }
    slots.sort_by(|a, b| a.1.rank.cmp(&b.1.rank));

    let mut ten_plus_sorted = ten_plus_remaining;
    ten_plus_sorted.sort_by(|a, b| b.rank.cmp(&a.rank));

    let mut replacements = Vec::new();
    let limit = ten_plus_sorted.len().min(3);
    let slot_indices: Vec<usize> = slots.iter().map(|(idx, _)| *idx).collect();

    for ten in ten_plus_sorted.iter().take(limit) {
        for &slot in &slot_indices {
            let mut cards = [combo[0].0, combo[1].0, combo[2].0];
            cards[slot] = *ten;
            if cards
                .iter()
                .filter(|card| card.suit == Suit::Hearts && card.rank >= Rank::Ten)
                .count()
                == 0
            {
                continue;
            }
            push_unique_cards(&mut replacements, cards);
            if replacements.len() >= 4 {
                return replacements;
            }
        }
    }

    replacements
}

fn build_single_heart_ten_plus_replacements(
    input: &PassScoreInput<'_>,
    combo: &[(Card, PassScoreBreakdown); 3],
) -> Vec<[Card; 3]> {
    let hearts: Vec<(usize, Card)> = combo
        .iter()
        .enumerate()
        .filter_map(|(idx, (card, _))| (card.suit == Suit::Hearts).then(|| (idx, *card)))
        .collect();
    if hearts.len() != 1 {
        return Vec::new();
    }
    let ten_plus_available: Vec<Card> = input
        .hand
        .iter()
        .copied()
        .filter(|card| card.suit == Suit::Hearts && card.rank >= Rank::Ten)
        .filter(|card| combo.iter().all(|(existing, _)| existing != card))
        .collect();
    if ten_plus_available.is_empty() {
        return Vec::new();
    }

    let heart_index = hearts[0].0;
    let mut replacements = Vec::new();
    for ten_plus in ten_plus_available {
        let mut cards = [combo[0].0, combo[1].0, combo[2].0];
        cards[heart_index] = ten_plus;
        push_unique_cards(&mut replacements, cards);
        if replacements.len() >= 4 {
            break;
        }
    }

    replacements
}

fn build_premium_split_replacements(
    input: &PassScoreInput<'_>,
    combo: &[(Card, PassScoreBreakdown); 3],
) -> Vec<[Card; 3]> {
    if !matches!(input.direction, PassingDirection::Left) {
        return Vec::new();
    }
    let urgency = input.moon_estimate.defensive_urgency();
    if urgency < 0.6 {
        return Vec::new();
    }

    let premium_indices: Vec<usize> = combo
        .iter()
        .enumerate()
        .filter_map(|(idx, (card, _))| {
            if card.suit == Suit::Hearts && card.rank >= Rank::Queen {
                Some(idx)
            } else {
                None
            }
        })
        .collect();
    if premium_indices.len() < 2 {
        return Vec::new();
    }

    let mut support_candidates: Vec<Card> = input
        .hand
        .iter()
        .copied()
        .filter(|card| {
            card.suit == Suit::Hearts && card.rank >= Rank::Ten && card.rank < Rank::Queen
        })
        .collect();
    support_candidates.retain(|card| !combo.iter().any(|(existing, _)| existing == card));
    if support_candidates.is_empty() {
        let mut off_candidates: Vec<(Card, PassScoreBreakdown)> = input
            .hand
            .iter()
            .copied()
            .filter(|card| card.suit != Suit::Hearts)
            .filter(|card| !combo.iter().any(|(existing, _)| existing == card))
            .map(|card| {
                let breakdown = score_single_card(input, card);
                (card, breakdown)
            })
            .collect();
        off_candidates.sort_by(|a, b| b.1.total.partial_cmp(&a.1.total).unwrap_or(Ordering::Equal));
        off_candidates.dedup_by(|a, b| a.0 == b.0);
        support_candidates.extend(off_candidates.into_iter().take(3).map(|(card, _)| card));
    }

    if support_candidates.is_empty() {
        return Vec::new();
    }

    support_candidates.sort_by(|a, b| b.rank.cmp(&a.rank));
    support_candidates.dedup();

    let mut combos = Vec::new();
    let combo_signature = combo_signature_from_pairs(combo);
    let support_limit = support_candidates.len().min(4);

    let mut premium_slots = premium_indices.clone();
    premium_slots.sort_by(|&a, &b| combo[b].0.rank.cmp(&combo[a].0.rank));

    'outer: for &slot in &premium_slots {
        for support_idx in 0..support_limit {
            let support_card = support_candidates[support_idx];
            let mut cards = [combo[0].0, combo[1].0, combo[2].0];
            cards[slot] = support_card;

            if combo_signature_cards(&cards) == combo_signature {
                continue;
            }

            let premium_remaining = cards
                .iter()
                .filter(|card| card.suit == Suit::Hearts && card.rank >= Rank::Queen)
                .count();
            if premium_remaining == 0 {
                continue;
            }

            let heart_count = cards
                .iter()
                .filter(|card| card.suit == Suit::Hearts)
                .count();
            if heart_count < 2 {
                continue;
            }

            push_unique_cards(&mut combos, cards);
            if combos.len() >= 6 {
                break 'outer;
            }
        }
    }

    combos
}

fn combo_signature_from_pairs(combo: &[(Card, PassScoreBreakdown); 3]) -> [u8; 3] {
    let cards = [combo[0].0, combo[1].0, combo[2].0];
    combo_signature_cards(&cards)
}

fn combo_signature_cards(cards: &[Card; 3]) -> [u8; 3] {
    let mut ids = [cards[0].to_id(), cards[1].to_id(), cards[2].to_id()];
    ids.sort();
    ids
}

fn push_unique_cards(storage: &mut Vec<[Card; 3]>, candidate: [Card; 3]) {
    let signature = combo_signature_cards(&candidate);
    if storage
        .iter()
        .any(|existing| combo_signature_cards(existing) == signature)
    {
        return;
    }
    storage.push(candidate);
}

fn build_liability_combo(input: &PassScoreInput<'_>) -> Option<[Card; 3]> {
    use std::cmp::Ordering;

    let mut scored: Vec<(Card, PassScoreBreakdown)> = input
        .hand
        .iter()
        .copied()
        .map(|card| (card, score_single_card(input, card)))
        .collect();
    scored.sort_by(|a, b| b.1.total.partial_cmp(&a.1.total).unwrap_or(Ordering::Equal));
    let limit = scored.len().min(9);
    if limit < 3 {
        return None;
    }

    let mut best: Option<([Card; 3], f32)> = None;
    for indices in choose_index_combinations(limit, 3) {
        if indices.len() != 3 {
            continue;
        }
        let cards = [
            scored[indices[0]].0,
            scored[indices[1]].0,
            scored[indices[2]].0,
        ];
        let breakdown = [
            (cards[0], score_single_card(input, cards[0])),
            (cards[1], score_single_card(input, cards[1])),
            (cards[2], score_single_card(input, cards[2])),
        ];
        if violates_ten_plus_safety(input, &breakdown) {
            continue;
        }
        if violates_support_guard(input, &breakdown) {
            continue;
        }
        if violates_all_offsuit_guard(input, &breakdown) {
            continue;
        }
        if missing_ten_plus_support_cards(input, &cards) {
            continue;
        }
        if cards.iter().all(|card| card.suit == Suit::Hearts) {
            continue;
        }
        let eval = evaluate_combo(input, &breakdown);
        if eval.moon_penalty >= HARD_REJECTION_PENALTY {
            continue;
        }
        match &mut best {
            None => best = Some((cards, eval.total)),
            Some((current_cards, current_score)) => {
                if eval.total > *current_score {
                    *current_cards = cards;
                    *current_score = eval.total;
                }
            }
        }
    }

    best.map(|(cards, _)| cards)
}

fn build_supportless_premium_combo(input: &PassScoreInput<'_>) -> Option<[Card; 3]> {
    use std::cmp::Ordering;

    if !matches!(input.direction, PassingDirection::Left) {
        return None;
    }

    let hearts: Vec<Card> = input
        .hand
        .iter()
        .copied()
        .filter(|card| card.suit == Suit::Hearts)
        .collect();
    if hearts.len() < 2 {
        return None;
    }

    let queen = match hearts.iter().copied().find(|card| card.rank == Rank::Queen) {
        Some(card) => card,
        None => return None,
    };
    let jack = match hearts.iter().copied().find(|card| card.rank == Rank::Jack) {
        Some(card) => card,
        None => return None,
    };

    let support_remaining = hearts
        .iter()
        .filter(|card| {
            (*card).rank >= Rank::Ten
                && (*card).rank < Rank::Queen
                && **card != queen
                && **card != jack
        })
        .count()
        + hearts
            .iter()
            .filter(|card| is_mid_support_heart(card) && **card != queen && **card != jack)
            .count();
    if support_remaining > 0 {
        return None;
    }

    let remaining_premiums = hearts
        .iter()
        .filter(|card| card.rank >= Rank::Queen && **card != queen && **card != jack)
        .count();
    if remaining_premiums < 1 {
        return None;
    }

    let mut off_cards: Vec<(Card, PassScoreBreakdown)> = input
        .hand
        .iter()
        .copied()
        .filter(|card| card.suit != Suit::Hearts)
        .map(|card| {
            let breakdown = score_single_card(input, card);
            (card, breakdown)
        })
        .collect();
    if off_cards.is_empty() {
        return None;
    }
    off_cards.sort_by(|a, b| b.1.total.partial_cmp(&a.1.total).unwrap_or(Ordering::Equal));

    let off_choice = off_cards[0].0;
    Some([queen, jack, off_choice])
}

fn build_ten_plus_liability_combo(input: &PassScoreInput<'_>) -> Option<[Card; 3]> {
    use std::cmp::Ordering;

    let mut ten_plus_hearts: Vec<Card> = input
        .hand
        .iter()
        .copied()
        .filter(|card| card.suit == Suit::Hearts && card.rank >= Rank::Ten)
        .collect();
    if ten_plus_hearts.is_empty() {
        return None;
    }
    ten_plus_hearts.sort_by(|a, b| a.rank.cmp(&b.rank));

    let mut off_cards: Vec<(Card, PassScoreBreakdown)> = input
        .hand
        .iter()
        .copied()
        .filter(|card| card.suit != Suit::Hearts)
        .map(|card| {
            let breakdown = score_single_card(input, card);
            (card, breakdown)
        })
        .collect();
    if off_cards.len() < 2 {
        return None;
    }
    off_cards.sort_by(|a, b| b.1.total.partial_cmp(&a.1.total).unwrap_or(Ordering::Equal));

    let off_limit = off_cards.len().min(6);

    for heart in ten_plus_hearts.into_iter() {
        for pair in choose_index_combinations(off_limit, 2) {
            if pair.len() != 2 {
                continue;
            }
            let combo = [heart, off_cards[pair[0]].0, off_cards[pair[1]].0];
            if combo.iter().collect::<std::collections::HashSet<_>>().len() < 3 {
                continue;
            }
            let breakdown = [
                (combo[0], score_single_card(input, combo[0])),
                (combo[1], score_single_card(input, combo[1])),
                (combo[2], score_single_card(input, combo[2])),
            ];
            if violates_ten_plus_safety(input, &breakdown) {
                continue;
            }
            if violates_support_guard(input, &breakdown) {
                continue;
            }
            if violates_all_offsuit_guard(input, &breakdown) {
                continue;
            }
            if missing_ten_plus_support_cards(input, &combo) {
                continue;
            }
            return Some(combo);
        }
    }

    None
}

fn build_single_heart_liability_combo(input: &PassScoreInput<'_>) -> Option<[Card; 3]> {
    use std::cmp::Ordering;

    let mut ten_plus_hearts: Vec<Card> = input
        .hand
        .iter()
        .copied()
        .filter(|card| card.suit == Suit::Hearts && card.rank >= Rank::Ten)
        .collect();
    if ten_plus_hearts.is_empty() {
        return None;
    }
    ten_plus_hearts.sort_by(|a, b| a.rank.cmp(&b.rank));

    let mut off_cards: Vec<(Card, PassScoreBreakdown)> = input
        .hand
        .iter()
        .copied()
        .filter(|card| card.suit != Suit::Hearts)
        .map(|card| {
            let breakdown = score_single_card(input, card);
            (card, breakdown)
        })
        .collect();
    if off_cards.len() < 2 {
        return None;
    }
    off_cards.sort_by(|a, b| b.1.total.partial_cmp(&a.1.total).unwrap_or(Ordering::Equal));

    let off_limit = off_cards.len().min(6);

    for heart in ten_plus_hearts.into_iter() {
        for pair in choose_index_combinations(off_limit, 2) {
            if pair.len() != 2 {
                continue;
            }
            let combo = [heart, off_cards[pair[0]].0, off_cards[pair[1]].0];
            if combo.iter().collect::<std::collections::HashSet<_>>().len() < 3 {
                continue;
            }
            let breakdown = [
                (combo[0], score_single_card(input, combo[0])),
                (combo[1], score_single_card(input, combo[1])),
                (combo[2], score_single_card(input, combo[2])),
            ];
            if violates_ten_plus_safety(input, &breakdown) {
                continue;
            }
            if violates_support_guard(input, &breakdown) {
                continue;
            }
            if violates_all_offsuit_guard(input, &breakdown) {
                continue;
            }
            if missing_ten_plus_support_cards(input, &combo) {
                continue;
            }
            return Some(combo);
        }
    }

    None
}

fn upgrade_all_heart_candidate(
    input: &PassScoreInput<'_>,
    cards: [Card; 3],
) -> Option<(PassCandidate, ComboEval)> {
    if cards.iter().any(|card| card.suit != Suit::Hearts) {
        return None;
    }
    let support_high = cards
        .iter()
        .filter(|card| is_high_support_heart(card))
        .count();
    let support_mid = cards
        .iter()
        .filter(|card| is_mid_support_heart(card))
        .count();
    let support_total = support_high + support_mid;
    let passes_q = cards
        .iter()
        .any(|card| card.suit == Suit::Hearts && card.rank == Rank::Queen);
    let passes_j = cards
        .iter()
        .any(|card| card.suit == Suit::Hearts && card.rank == Rank::Jack);
    let support_needed = if passes_q {
        2
    } else if passes_j {
        1
    } else {
        0
    };
    if support_total >= support_needed {
        return None;
    }
    let mut off_candidates: Vec<(Card, PassScoreBreakdown)> = input
        .hand
        .iter()
        .copied()
        .filter(|card| cards.iter().all(|existing| existing != card))
        .filter(|card| card.suit != Suit::Hearts)
        .map(|card| {
            let breakdown = score_single_card(input, card);
            (card, breakdown)
        })
        .collect();
    if off_candidates.is_empty() {
        return None;
    }
    off_candidates.sort_by(|a, b| b.1.total.partial_cmp(&a.1.total).unwrap_or(Ordering::Equal));

    let mut heart_indices: Vec<usize> = (0..3)
        .filter(|idx| cards[*idx].suit == Suit::Hearts && cards[*idx].rank < Rank::Queen)
        .collect();
    if heart_indices.is_empty() {
        return None;
    }
    heart_indices.sort_by_key(|idx| cards[*idx].rank);

    for (off_card, _) in off_candidates.iter().take(6) {
        for &idx in &heart_indices {
            let mut new_cards = cards;
            new_cards[idx] = *off_card;
            if new_cards.iter().all(|card| card.suit == Suit::Hearts) {
                continue;
            }
            let breakdown = [
                (new_cards[0], score_single_card(input, new_cards[0])),
                (new_cards[1], score_single_card(input, new_cards[1])),
                (new_cards[2], score_single_card(input, new_cards[2])),
            ];
            if violates_ten_plus_safety(input, &breakdown) {
                continue;
            }
            if violates_support_guard(input, &breakdown) {
                continue;
            }
            if violates_all_offsuit_guard(input, &breakdown) {
                continue;
            }
            if missing_ten_plus_support_cards(input, &new_cards) {
                continue;
            }
            let eval = evaluate_combo(input, &breakdown);
            if eval.moon_penalty >= HARD_REJECTION_PENALTY {
                continue;
            }
            let candidate = PassCandidate {
                cards: new_cards,
                score: eval.total,
                void_score: eval.void_sum,
                liability_score: eval.liability_sum,
                moon_score: eval.moon_sum,
                synergy: eval.synergy,
                direction_bonus: eval.direction_bonus,
                moon_liability_penalty: eval.moon_penalty,
            };
            return Some((candidate, eval));
        }
    }

    None
}

fn demote_queen_candidate(
    input: &PassScoreInput<'_>,
    candidate: PassCandidate,
) -> Option<(PassCandidate, ComboEval)> {
    if !candidate
        .cards
        .iter()
        .any(|card| card.suit == Suit::Hearts && card.rank == Rank::Queen)
    {
        return None;
    }
    let support_high = candidate
        .cards
        .iter()
        .filter(|card| is_high_support_heart(card))
        .count();
    let support_mid = candidate
        .cards
        .iter()
        .filter(|card| is_mid_support_heart(card))
        .count();
    if support_high + support_mid >= 2 {
        return None;
    }

    let mut replacements: Vec<Card> = input
        .hand
        .iter()
        .copied()
        .filter(|card| {
            card.suit == Suit::Hearts
                && card.rank >= Rank::Ten
                && card.rank < Rank::Queen
                && !candidate.cards.contains(card)
        })
        .collect();
    if replacements.is_empty() {
        return None;
    }
    replacements.sort_by(|a, b| b.rank.cmp(&a.rank));

    for replacement in replacements {
        let mut new_cards = candidate.cards;
        if let Some(slot) = new_cards
            .iter()
            .position(|card| card.suit == Suit::Hearts && card.rank == Rank::Queen)
        {
            new_cards[slot] = replacement;
        } else {
            continue;
        }
        let breakdown = [
            (new_cards[0], score_single_card(input, new_cards[0])),
            (new_cards[1], score_single_card(input, new_cards[1])),
            (new_cards[2], score_single_card(input, new_cards[2])),
        ];
        if violates_ten_plus_safety(input, &breakdown) {
            continue;
        }
        if violates_support_guard(input, &breakdown) {
            continue;
        }
        if violates_all_offsuit_guard(input, &breakdown) {
            continue;
        }
        if missing_ten_plus_support_cards(input, &new_cards) {
            continue;
        }
        let eval = evaluate_combo(input, &breakdown);
        if eval.moon_penalty >= HARD_REJECTION_PENALTY {
            continue;
        }
        let demoted = PassCandidate {
            cards: new_cards,
            score: eval.total,
            void_score: eval.void_sum,
            liability_score: eval.liability_sum,
            moon_score: eval.moon_sum,
            synergy: eval.synergy,
            direction_bonus: eval.direction_bonus,
            moon_liability_penalty: eval.moon_penalty,
        };
        return Some((demoted, eval));
    }

    None
}

fn contains_combo(list: &[[Card; 3]], candidate: &[Card; 3]) -> bool {
    let signature = combo_signature_cards(candidate);
    list.iter()
        .any(|existing| combo_signature_cards(existing) == signature)
}

fn is_high_support_heart(card: &Card) -> bool {
    card.suit == Suit::Hearts && card.rank >= Rank::Ten && card.rank < Rank::Queen
}

fn is_mid_support_heart(card: &Card) -> bool {
    card.suit == Suit::Hearts && card.rank >= Rank::Eight && card.rank < Rank::Ten
}

fn build_premium_relief_combo(input: &PassScoreInput<'_>) -> Option<[Card; 3]> {
    let mut premium: Vec<Card> = input
        .hand
        .iter()
        .copied()
        .filter(|card| card.suit == Suit::Hearts && card.rank >= Rank::Queen)
        .collect();
    if premium.len() < 2 {
        return None;
    }
    premium.sort_by(|a, b| a.rank.cmp(&b.rank));
    let premium_to_pass = premium.first().copied().unwrap();

    let mut support_high: Vec<Card> = input
        .hand
        .iter()
        .copied()
        .filter(|card| {
            card.suit == Suit::Hearts && card.rank >= Rank::Ten && card.rank < Rank::Queen
        })
        .collect();
    support_high.sort_by(|a, b| a.rank.cmp(&b.rank));

    let mut support_mid: Vec<Card> = input
        .hand
        .iter()
        .copied()
        .filter(|card| {
            card.suit == Suit::Hearts && card.rank >= Rank::Eight && card.rank < Rank::Ten
        })
        .collect();
    support_mid.sort_by(|a, b| b.rank.cmp(&a.rank));

    let support_choice = support_high
        .iter()
        .copied()
        .find(|card| *card != premium_to_pass)
        .or_else(|| {
            support_mid
                .iter()
                .copied()
                .find(|card| *card != premium_to_pass)
        })
        .or_else(|| {
            input
                .hand
                .iter()
                .copied()
                .filter(|card| card.suit == Suit::Hearts)
                .find(|card| *card != premium_to_pass)
        })?;

    let mut off_candidates: Vec<(Card, PassScoreBreakdown)> = input
        .hand
        .iter()
        .copied()
        .filter(|card| card.suit != Suit::Hearts)
        .map(|card| {
            let breakdown = score_single_card(input, card);
            (card, breakdown)
        })
        .collect();
    off_candidates.sort_by(|a, b| b.1.total.partial_cmp(&a.1.total).unwrap_or(Ordering::Equal));

    let off_choice = off_candidates
        .iter()
        .copied()
        .map(|(card, _)| card)
        .find(|card| *card != support_choice)
        .or_else(|| {
            support_high
                .iter()
                .copied()
                .find(|card| *card != premium_to_pass && *card != support_choice)
        })
        .or_else(|| {
            support_mid
                .iter()
                .copied()
                .find(|card| *card != premium_to_pass && *card != support_choice)
        })?;

    Some([premium_to_pass, support_choice, off_choice])
}

fn build_premium_anchor_combo(input: &PassScoreInput<'_>) -> Option<[Card; 3]> {
    if !matches!(input.direction, PassingDirection::Left) {
        return None;
    }

    let mut premium: Vec<Card> = input
        .hand
        .iter()
        .copied()
        .filter(|card| card.suit == Suit::Hearts && card.rank >= Rank::Queen)
        .collect();
    if premium.len() < 2 {
        return None;
    }
    premium.sort_by(|a, b| a.rank.cmp(&b.rank));
    let premium_choice = premium.first().copied()?;

    let mut preferred_support: Vec<Card> = input
        .hand
        .iter()
        .copied()
        .filter(|card| {
            card.suit == Suit::Hearts
                && card.rank >= Rank::Eight
                && card.rank < Rank::Queen
                && *card != premium_choice
                && card.rank != Rank::Jack
        })
        .collect();
    preferred_support.sort_by(|a, b| a.rank.cmp(&b.rank));

    let mut jack_support: Vec<Card> = input
        .hand
        .iter()
        .copied()
        .filter(|card| {
            card.suit == Suit::Hearts && card.rank == Rank::Jack && *card != premium_choice
        })
        .collect();
    jack_support.sort_by(|a, b| a.rank.cmp(&b.rank));

    let mut support_mid: Vec<Card> = input
        .hand
        .iter()
        .copied()
        .filter(|card| {
            card.suit == Suit::Hearts
                && card.rank >= Rank::Eight
                && card.rank < Rank::Ten
                && *card != premium_choice
        })
        .collect();
    support_mid.sort_by(|a, b| b.rank.cmp(&a.rank));

    let support_choice = preferred_support
        .iter()
        .copied()
        .next()
        .or_else(|| support_mid.iter().copied().next())
        .or_else(|| jack_support.iter().copied().next())?;

    let support_choice = if support_choice.rank == Rank::Jack {
        let mut low_hearts: Vec<Card> = input
            .hand
            .iter()
            .copied()
            .filter(|card| {
                card.suit == Suit::Hearts && card.rank < Rank::Eight && *card != premium_choice
            })
            .collect();
        if !low_hearts.is_empty() {
            low_hearts.sort_by(|a, b| b.rank.cmp(&a.rank));
            low_hearts.first().copied().unwrap()
        } else {
            support_choice
        }
    } else {
        support_choice
    };

    let mut liability_off: Vec<(Card, PassScoreBreakdown)> = input
        .hand
        .iter()
        .copied()
        .filter(|card| card.suit != Suit::Hearts && is_offsuit_liability(card))
        .map(|card| {
            let breakdown = score_single_card(input, card);
            (card, breakdown)
        })
        .collect();
    if liability_off.is_empty() {
        return None;
    }
    liability_off.sort_by(|a, b| b.1.total.partial_cmp(&a.1.total).unwrap_or(Ordering::Equal));

    let anchor_choice = liability_off
        .iter()
        .copied()
        .map(|(card, _)| card)
        .find(|card| *card != support_choice)?;

    Some([premium_choice, support_choice, anchor_choice])
}

fn build_supportive_heart_combo(input: &PassScoreInput<'_>) -> Option<[Card; 3]> {
    let mut hearts: Vec<Card> = input
        .hand
        .iter()
        .copied()
        .filter(|card| card.suit == Suit::Hearts)
        .collect();
    if hearts.len() < 2 {
        return None;
    }
    hearts.sort_by(|a, b| b.rank.cmp(&a.rank));

    let mut support: Vec<Card> = hearts
        .iter()
        .copied()
        .filter(|card| card.rank < Rank::Queen)
        .collect();
    let support_guard_hearts = hearts
        .iter()
        .filter(|card| is_high_support_heart(card) || is_mid_support_heart(card))
        .count();
    let premium_hearts: Vec<Card> = hearts
        .iter()
        .copied()
        .filter(|card| card.rank >= Rank::Queen)
        .collect();
    if premium_hearts.len() >= 3 && support_guard_hearts == 0 {
        if let Some(queen) = premium_hearts
            .iter()
            .copied()
            .find(|card| card.rank == Rank::Queen)
        {
            let mut low_hearts: Vec<Card> = hearts
                .iter()
                .copied()
                .filter(|card| *card != queen && card.rank < Rank::Queen)
                .collect();
            low_hearts.sort_by(|a, b| b.rank.cmp(&a.rank));
            if let Some(low) = low_hearts.first().copied() {
                let mut off_candidates: Vec<Card> = input
                    .hand
                    .iter()
                    .copied()
                    .filter(|card| card.suit != Suit::Hearts)
                    .collect();
                off_candidates.sort_by(|a, b| {
                    let score_a = score_single_card(input, *a).total;
                    let score_b = score_single_card(input, *b).total;
                    score_b
                        .total_cmp(&score_a)
                        .then_with(|| b.rank.cmp(&a.rank))
                });
                if let Some(off_card) = off_candidates
                    .iter()
                    .copied()
                    .find(|card| is_offsuit_liability(card) && *card != low)
                    .or_else(|| {
                        off_candidates
                            .iter()
                            .copied()
                            .find(|card| *card != low && *card != queen)
                    })
                {
                    return Some([queen, low, off_card]);
                }
            }
        }
    }
    support.sort_by(|a, b| b.rank.cmp(&a.rank));
    support.dedup();
    if support.len() < 2 {
        return None;
    }

    let first = support[0];
    let second = support[1];

    let mut off_candidates: Vec<(Card, PassScoreBreakdown)> = input
        .hand
        .iter()
        .copied()
        .filter(|card| card.suit != Suit::Hearts)
        .filter(|card| *card != first && *card != second)
        .map(|card| {
            let breakdown = score_single_card(input, card);
            (card, breakdown)
        })
        .collect();
    off_candidates.sort_by(|a, b| b.1.total.partial_cmp(&a.1.total).unwrap_or(Ordering::Equal));

    if let Some((off_card, _)) = off_candidates.first() {
        Some([first, second, *off_card])
    } else if support.len() > 2 {
        Some([first, second, support[2]])
    } else {
        None
    }
}

fn synthesize_guarded_combos(input: &PassScoreInput<'_>) -> Vec<[Card; 3]> {
    let mut combos = Vec::new();
    let mut hearts: Vec<Card> = input
        .hand
        .iter()
        .copied()
        .filter(|card| card.suit == Suit::Hearts)
        .collect();
    if hearts.is_empty() {
        return combos;
    }
    hearts.sort_by(|a, b| b.rank.cmp(&a.rank));

    let total_ten_plus = hearts.iter().filter(|card| card.rank >= Rank::Ten).count();
    let mut hearts_without_ace: Vec<Card> = hearts
        .iter()
        .copied()
        .filter(|card| card.rank != Rank::Ace)
        .collect();
    hearts_without_ace.sort_by(|a, b| b.rank.cmp(&a.rank));

    let insufficient_support = total_ten_plus < 3;
    let mut safe_hearts: Vec<Card> = if insufficient_support {
        hearts_without_ace
            .iter()
            .copied()
            .filter(|card| card.rank < Rank::King)
            .collect()
    } else {
        hearts_without_ace.clone()
    };
    safe_hearts.sort_by(|a, b| b.rank.cmp(&a.rank));

    let mut off_cards: Vec<Card> = input
        .hand
        .iter()
        .copied()
        .filter(|card| card.suit != Suit::Hearts)
        .collect();
    off_cards.sort_by(|a, b| {
        let score_a = score_single_card(input, *a).total;
        let score_b = score_single_card(input, *b).total;
        score_b
            .total_cmp(&score_a)
            .then_with(|| b.rank.cmp(&a.rank))
    });
    off_cards.dedup();

    if safe_hearts.len() >= 3 {
        let limit = safe_hearts.len().min(5);
        'outer: for i in 0..limit {
            for j in (i + 1)..limit {
                for k in (j + 1)..limit {
                    let candidate = [safe_hearts[i], safe_hearts[j], safe_hearts[k]];
                    push_unique_cards(&mut combos, candidate);
                    if combos.len() >= 6 {
                        break 'outer;
                    }
                }
            }
        }
    }

    if combos.is_empty() && hearts_without_ace.len() >= 3 {
        let limit = hearts_without_ace.len().min(4);
        'outer_all: for i in 0..limit {
            for j in (i + 1)..limit {
                for k in (j + 1)..limit {
                    let candidate = [
                        hearts_without_ace[i],
                        hearts_without_ace[j],
                        hearts_without_ace[k],
                    ];
                    push_unique_cards(&mut combos, candidate);
                    if combos.len() >= 6 {
                        break 'outer_all;
                    }
                }
            }
        }
    }

    if combos.is_empty() && total_ten_plus >= 2 && hearts.len() >= 3 {
        push_unique_cards(&mut combos, [hearts[0], hearts[1], hearts[2]]);
    }

    if !off_cards.is_empty() {
        if safe_hearts.len() >= 2 {
            for off in off_cards.iter().take(4) {
                push_unique_cards(&mut combos, [safe_hearts[0], safe_hearts[1], *off]);
                if combos.len() >= 6 {
                    break;
                }
            }
        }
        if combos.len() < 6 && safe_hearts.len() >= 1 && off_cards.len() >= 2 {
            for indices in choose_index_combinations(off_cards.len().min(4), 2) {
                if indices.len() != 2 {
                    continue;
                }
                let candidate = [safe_hearts[0], off_cards[indices[0]], off_cards[indices[1]]];
                push_unique_cards(&mut combos, candidate);
                if combos.len() >= 6 {
                    break;
                }
            }
        }
        if combos.len() < 6 && hearts.len() >= 1 && off_cards.len() >= 2 {
            let candidate = [hearts[0], off_cards[0], off_cards[1]];
            push_unique_cards(&mut combos, candidate);
        }
    }

    combos
}

enum PremiumSupportResolution {
    Valid,
    Replacements(Vec<[(Card, PassScoreBreakdown); 3]>),
    Discard,
}

pub fn force_guarded_pass(input: &PassScoreInput<'_>) -> Option<PassCandidate> {
    if input.hand.len() < 3 {
        return None;
    }

    let mut hearts: Vec<Card> = input
        .hand
        .iter()
        .copied()
        .filter(|card| card.suit == Suit::Hearts)
        .collect();
    if hearts.is_empty() {
        return None;
    }
    hearts.sort_by(|a, b| b.rank.cmp(&a.rank));

    let mut others: Vec<Card> = input
        .hand
        .iter()
        .copied()
        .filter(|card| card.suit != Suit::Hearts)
        .collect();
    others.sort_by(|a, b| {
        let score_a = score_single_card(input, *a).total;
        let score_b = score_single_card(input, *b).total;
        score_b
            .total_cmp(&score_a)
            .then_with(|| b.rank.cmp(&a.rank))
    });

    let mut raw_candidates: Vec<[Card; 3]> = Vec::new();
    if hearts.len() >= 3 {
        raw_candidates.push([hearts[0], hearts[1], hearts[2]]);
    }
    if hearts.len() >= 2 && !others.is_empty() {
        raw_candidates.push([hearts[0], hearts[1], others[0]]);
    }
    if hearts.len() >= 1 && others.len() >= 2 {
        raw_candidates.push([hearts[0], others[0], others[1]]);
    }

    raw_candidates.retain(|combo| {
        let mut unique = std::collections::HashSet::new();
        combo.iter().all(|card| unique.insert(card.clone()))
    });

    let mut synthesized = synthesize_guarded_combos(input);
    for combo in synthesized.drain(..) {
        if !contains_combo(&raw_candidates, &combo) {
            raw_candidates.push(combo);
        }
    }

    if raw_candidates.is_empty() {
        return None;
    }

    let mut best: Option<(PassCandidate, ComboEval)> = None;
    for combo_cards in raw_candidates {
        let breakdown = [
            (combo_cards[0], score_single_card(input, combo_cards[0])),
            (combo_cards[1], score_single_card(input, combo_cards[1])),
            (combo_cards[2], score_single_card(input, combo_cards[2])),
        ];
        if violates_ten_plus_safety(input, &breakdown) {
            continue;
        }
        if violates_support_guard(input, &breakdown) {
            continue;
        }
        if violates_all_offsuit_guard(input, &breakdown) {
            continue;
        }
        if missing_ten_plus_support_cards(input, &combo_cards) {
            continue;
        }
        let eval = evaluate_combo(input, &breakdown);
        if eval.moon_penalty >= HARD_REJECTION_PENALTY {
            continue;
        }
        let candidate = PassCandidate {
            cards: combo_cards,
            score: eval.total,
            void_score: eval.void_sum,
            liability_score: eval.liability_sum,
            moon_score: eval.moon_sum,
            synergy: eval.synergy,
            direction_bonus: eval.direction_bonus,
            moon_liability_penalty: eval.moon_penalty,
        };
        match &best {
            None => best = Some((candidate, eval)),
            Some((current, _)) => {
                if candidate.score > current.score {
                    best = Some((candidate, eval));
                }
            }
        }
    }

    if best.is_none() {
        if let Some(combo_cards) = build_premium_anchor_combo(input) {
            let breakdown = [
                (combo_cards[0], score_single_card(input, combo_cards[0])),
                (combo_cards[1], score_single_card(input, combo_cards[1])),
                (combo_cards[2], score_single_card(input, combo_cards[2])),
            ];
            if !violates_ten_plus_safety(input, &breakdown)
                && !violates_support_guard(input, &breakdown)
                && !violates_all_offsuit_guard(input, &breakdown)
                && !missing_ten_plus_support_cards(input, &combo_cards)
            {
                let eval = evaluate_combo(input, &breakdown);
                if eval.moon_penalty < HARD_REJECTION_PENALTY {
                    let candidate = PassCandidate {
                        cards: combo_cards,
                        score: eval.total,
                        void_score: eval.void_sum,
                        liability_score: eval.liability_sum,
                        moon_score: eval.moon_sum,
                        synergy: eval.synergy,
                        direction_bonus: eval.direction_bonus,
                        moon_liability_penalty: eval.moon_penalty,
                    };
                    best = Some((candidate, eval));
                }
            }
        }
    }

    if best.is_none() {
        if let Some(combo_cards) = build_liability_combo(input) {
            let breakdown = [
                (combo_cards[0], score_single_card(input, combo_cards[0])),
                (combo_cards[1], score_single_card(input, combo_cards[1])),
                (combo_cards[2], score_single_card(input, combo_cards[2])),
            ];
            if !violates_ten_plus_safety(input, &breakdown)
                && !violates_support_guard(input, &breakdown)
                && !violates_all_offsuit_guard(input, &breakdown)
                && !missing_ten_plus_support_cards(input, &combo_cards)
            {
                let eval = evaluate_combo(input, &breakdown);
                if eval.moon_penalty < HARD_REJECTION_PENALTY {
                    let candidate = PassCandidate {
                        cards: combo_cards,
                        score: eval.total,
                        void_score: eval.void_sum,
                        liability_score: eval.liability_sum,
                        moon_score: eval.moon_sum,
                        synergy: eval.synergy,
                        direction_bonus: eval.direction_bonus,
                        moon_liability_penalty: eval.moon_penalty,
                    };
                    best = Some((candidate, eval));
                }
            }
        }
    }

    if best.is_none() {
        if let Some(combo_cards) = build_premium_relief_combo(input) {
            let breakdown = [
                (combo_cards[0], score_single_card(input, combo_cards[0])),
                (combo_cards[1], score_single_card(input, combo_cards[1])),
                (combo_cards[2], score_single_card(input, combo_cards[2])),
            ];
            if !violates_ten_plus_safety(input, &breakdown)
                && !violates_support_guard(input, &breakdown)
                && !violates_all_offsuit_guard(input, &breakdown)
                && !missing_ten_plus_support_cards(input, &combo_cards)
            {
                let eval = evaluate_combo(input, &breakdown);
                if eval.moon_penalty < HARD_REJECTION_PENALTY {
                    let candidate = PassCandidate {
                        cards: combo_cards,
                        score: eval.total,
                        void_score: eval.void_sum,
                        liability_score: eval.liability_sum,
                        moon_score: eval.moon_sum,
                        synergy: eval.synergy,
                        direction_bonus: eval.direction_bonus,
                        moon_liability_penalty: eval.moon_penalty,
                    };
                    best = Some((candidate, eval));
                }
            }
        }
    }

    if best.is_none() {
        if let Some(combo_cards) = build_supportless_premium_combo(input) {
            let breakdown = [
                (combo_cards[0], score_single_card(input, combo_cards[0])),
                (combo_cards[1], score_single_card(input, combo_cards[1])),
                (combo_cards[2], score_single_card(input, combo_cards[2])),
            ];
            if !violates_ten_plus_safety(input, &breakdown)
                && !violates_support_guard(input, &breakdown)
                && !violates_all_offsuit_guard(input, &breakdown)
                && !missing_ten_plus_support_cards(input, &combo_cards)
            {
                let eval = evaluate_combo(input, &breakdown);
                if eval.moon_penalty < HARD_REJECTION_PENALTY {
                    let candidate = PassCandidate {
                        cards: combo_cards,
                        score: eval.total,
                        void_score: eval.void_sum,
                        liability_score: eval.liability_sum,
                        moon_score: eval.moon_sum,
                        synergy: eval.synergy,
                        direction_bonus: eval.direction_bonus,
                        moon_liability_penalty: eval.moon_penalty,
                    };
                    best = Some((candidate, eval));
                }
            }
        }
    }

    if best.is_none() {
        if let Some(combo_cards) = build_ten_plus_liability_combo(input) {
            let breakdown = [
                (combo_cards[0], score_single_card(input, combo_cards[0])),
                (combo_cards[1], score_single_card(input, combo_cards[1])),
                (combo_cards[2], score_single_card(input, combo_cards[2])),
            ];
            if !violates_ten_plus_safety(input, &breakdown)
                && !violates_support_guard(input, &breakdown)
                && !violates_all_offsuit_guard(input, &breakdown)
                && !missing_ten_plus_support_cards(input, &combo_cards)
            {
                let eval = evaluate_combo(input, &breakdown);
                if eval.moon_penalty < HARD_REJECTION_PENALTY {
                    let candidate = PassCandidate {
                        cards: combo_cards,
                        score: eval.total,
                        void_score: eval.void_sum,
                        liability_score: eval.liability_sum,
                        moon_score: eval.moon_sum,
                        synergy: eval.synergy,
                        direction_bonus: eval.direction_bonus,
                        moon_liability_penalty: eval.moon_penalty,
                    };
                    best = Some((candidate, eval));
                }
            }
        }
    }

    if best.is_none() {
        if let Some(combo_cards) = build_single_heart_liability_combo(input) {
            let breakdown = [
                (combo_cards[0], score_single_card(input, combo_cards[0])),
                (combo_cards[1], score_single_card(input, combo_cards[1])),
                (combo_cards[2], score_single_card(input, combo_cards[2])),
            ];
            if violates_ten_plus_safety(input, &breakdown) {
                println!(
                    "single-heart fallback rejected by ten_plus_safety {:?}",
                    combo_cards
                );
            } else if violates_support_guard(input, &breakdown) {
                println!(
                    "single-heart fallback rejected by support_guard {:?}",
                    combo_cards
                );
            } else if violates_all_offsuit_guard(input, &breakdown) {
                println!(
                    "single-heart fallback rejected by all_offsuit {:?}",
                    combo_cards
                );
            } else if missing_ten_plus_support_cards(input, &combo_cards) {
                println!(
                    "single-heart fallback rejected by missing_ten_plus {:?}",
                    combo_cards
                );
            } else {
                {
                    let eval = evaluate_combo(input, &breakdown);
                    if eval.moon_penalty < HARD_REJECTION_PENALTY
                        || combo_cards
                            .iter()
                            .any(|card| is_mid_support_heart(card) || is_high_support_heart(card))
                    {
                        #[cfg(test)]
                        println!("single-heart fallback {:?}", combo_cards);
                        let candidate = PassCandidate {
                            cards: combo_cards,
                            score: eval.total,
                            void_score: eval.void_sum,
                            liability_score: eval.liability_sum,
                            moon_score: eval.moon_sum,
                            synergy: eval.synergy,
                            direction_bonus: eval.direction_bonus,
                            moon_liability_penalty: eval.moon_penalty,
                        };
                        best = Some((candidate, eval));
                    }
                }
            }
        }
    }

    if best.is_none() {
        if let Some(combo_cards) = build_supportive_heart_combo(input) {
            let breakdown = [
                (combo_cards[0], score_single_card(input, combo_cards[0])),
                (combo_cards[1], score_single_card(input, combo_cards[1])),
                (combo_cards[2], score_single_card(input, combo_cards[2])),
            ];
            if !violates_ten_plus_safety(input, &breakdown)
                && !violates_support_guard(input, &breakdown)
                && !violates_all_offsuit_guard(input, &breakdown)
                && !missing_ten_plus_support_cards(input, &combo_cards)
            {
                let eval = evaluate_combo(input, &breakdown);
                if eval.moon_penalty < HARD_REJECTION_PENALTY {
                    let candidate = PassCandidate {
                        cards: combo_cards,
                        score: eval.total,
                        void_score: eval.void_sum,
                        liability_score: eval.liability_sum,
                        moon_score: eval.moon_sum,
                        synergy: eval.synergy,
                        direction_bonus: eval.direction_bonus,
                        moon_liability_penalty: eval.moon_penalty,
                    };
                    best = Some((candidate, eval));
                }
            }
        }
    }

    if let Some((mut candidate, _)) = best {
        if candidate.cards.iter().all(|card| card.suit == Suit::Hearts) {
            if let Some((upgraded, _upgraded_eval)) =
                upgrade_all_heart_candidate(input, candidate.cards)
            {
                candidate = upgraded;
            }
        }
        if candidate
            .cards
            .iter()
            .any(|card| card.suit == Suit::Hearts && card.rank == Rank::Queen)
        {
            if let Some((demoted, _)) = demote_queen_candidate(input, candidate.clone()) {
                candidate = demoted;
            }
        }
        Some(candidate)
    } else {
        None
    }
}

fn choose_index_combinations(len: usize, choose: usize) -> Vec<Vec<usize>> {
    if choose == 0 {
        return vec![Vec::new()];
    }
    if choose > len {
        return Vec::new();
    }
    fn backtrack(
        start: usize,
        len: usize,
        choose: usize,
        current: &mut Vec<usize>,
        output: &mut Vec<Vec<usize>>,
    ) {
        if current.len() == choose {
            output.push(current.clone());
            return;
        }
        for idx in start..len {
            current.push(idx);
            backtrack(idx + 1, len, choose, current, output);
            current.pop();
        }
    }

    let mut output = Vec::new();
    backtrack(0, len, choose, &mut Vec::new(), &mut output);
    output
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
    let heart_count = combo
        .iter()
        .filter(|(card, _)| card.suit == Suit::Hearts)
        .count();
    let ten_plus_count = combo
        .iter()
        .filter(|(card, _)| card.suit == Suit::Hearts && card.rank >= Rank::Ten)
        .count();
    let guard_penalty = if heart_count == 3 && ten_plus_count <= 1 {
        let urgency = input.moon_estimate.defensive_urgency().max(0.4);
        let shortage = (2isize - ten_plus_count as isize).max(1) as f32;
        input.weights.liability_base * 90.0 * shortage * urgency
    } else {
        0.0
    };

    ComboEval {
        total: base_total + direction_bonus - moon_penalty - guard_penalty,
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

            let ten_plus_remaining = remaining_mid_hearts;
            if pass_has_ace && ten_plus_remaining < 2 {
                let urgency_bias = 0.68 + 0.32 * urgency_scale;
                let shooter_bias = 1.0 + 1.05 * (shooter_scale - 1.0).max(0.0);
                let deficit = ((2isize - ten_plus_remaining as isize).max(1)) as f32;
                penalty += input.weights.liability_base
                    * 220.0
                    * deficit
                    * urgency_bias
                    * shooter_bias
                    * seat_scale
                    * direction_scale;
            } else if pass_premium_count >= 2 && ten_plus_remaining < 2 && passed_ten_plus >= 2 {
                let urgency_bias = 0.64 + 0.36 * urgency_scale;
                let shooter_bias = 1.0 + 0.9 * (shooter_scale - 1.0).max(0.0);
                let deficit = ((2isize - ten_plus_remaining as isize).max(1)) as f32;
                penalty += input.weights.liability_base
                    * 140.0
                    * deficit
                    * urgency_bias
                    * shooter_bias
                    * seat_scale
                    * direction_scale;
            }

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

            let remaining_high_heart_count = remaining_high_hearts.len();
            if passed_heart_count == 0 && (remaining_mid_hearts + remaining_high_heart_count) >= 2 {
                let heart_mass = (remaining_mid_hearts + remaining_high_heart_count) as f32;
                let urgency_bias = 0.7 + 0.3 * urgency_scale;
                let shooter_bias = 1.0 + 1.1 * (shooter_scale - 1.0).max(0.0);
                penalty += input.weights.liability_base
                    * 48.0
                    * heart_mass
                    * urgency_bias
                    * shooter_bias
                    * seat_scale
                    * direction_scale;
            }

            if passed_off_suit > 0 && remaining_mid_hearts >= 2 && passed_mid_hearts == 0 {
                let urgency_bias = 0.75 + 0.25 * urgency_scale;
                let remainder_bias = 1.6 + 0.45 * (remaining_mid_hearts as f32).min(4.0);
                let off_suit_bias = 1.2 + 0.8 * (passed_off_suit as f32);
                penalty += input.weights.liability_base
                    * 18.0
                    * urgency_bias
                    * remainder_bias
                    * off_suit_bias
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

fn is_offsuit_liability(card: &Card) -> bool {
    card.suit != Suit::Hearts && (card.is_queen_of_spades() || card.rank >= Rank::King)
}

fn violates_support_guard(
    input: &PassScoreInput<'_>,
    combo: &[(Card, PassScoreBreakdown); 3],
) -> bool {
    if !matches!(input.direction, PassingDirection::Left) {
        return false;
    }
    let passes_q = combo
        .iter()
        .any(|(card, _)| card.suit == Suit::Hearts && card.rank == Rank::Queen);
    let passes_j = combo
        .iter()
        .any(|(card, _)| card.suit == Suit::Hearts && card.rank == Rank::Jack);
    let passes_ten = combo
        .iter()
        .any(|(card, _)| card.suit == Suit::Hearts && card.rank == Rank::Ten);
    let support_high = combo
        .iter()
        .filter(|(card, _)| is_high_support_heart(card))
        .count();
    let support_mid = combo
        .iter()
        .filter(|(card, _)| is_mid_support_heart(card))
        .count();
    let support_total = support_high + support_mid;
    let strong_liability_support = combo
        .iter()
        .filter(|(card, _)| is_offsuit_liability(card))
        .count();
    let strong_liability_available = input
        .hand
        .iter()
        .any(|card| card.suit != Suit::Hearts && is_offsuit_liability(card));
    let soft_liability_available = input
        .hand
        .iter()
        .any(|card| card.suit != Suit::Hearts && card.rank == Rank::Queen);
    let soft_liability_support = combo
        .iter()
        .filter(|(card, _)| card.suit != Suit::Hearts && card.rank == Rank::Queen)
        .count();
    let liability_anchor_count = if strong_liability_support > 0 {
        strong_liability_support
    } else if !strong_liability_available && soft_liability_support > 0 {
        soft_liability_support
    } else {
        0
    };
    let any_anchor_available = strong_liability_available || soft_liability_available;
    let passed_ten_plus = combo
        .iter()
        .filter(|(card, _)| card.suit == Suit::Hearts && card.rank >= Rank::Ten)
        .count();
    let support_available = input
        .hand
        .iter()
        .filter(|card| {
            card.suit == Suit::Hearts
                && (is_high_support_heart(card) || is_mid_support_heart(card))
                && combo.iter().all(|(passed, _)| passed != *card)
        })
        .count();
    let support_high_excl_jack = if passes_j {
        support_high.saturating_sub(1)
    } else {
        support_high
    };
    let support_high_excl_ten = if passes_ten {
        support_high.saturating_sub(1)
    } else {
        support_high
    };
    let premium_remaining = input
        .hand
        .iter()
        .filter(|card| card.suit == Suit::Hearts && card.rank >= Rank::Queen)
        .filter(|card| combo.iter().all(|(passed, _)| passed != *card))
        .count();

    if passed_ten_plus >= 3
        && liability_anchor_count == 0
        && support_available == 0
        && premium_remaining > 0
        && any_anchor_available
    {
        return true;
    }
    if passed_ten_plus >= 2
        && liability_anchor_count == 0
        && support_available == 0
        && premium_remaining > 0
        && any_anchor_available
    {
        return true;
    }

    if passes_q {
        if support_total == 0 {
            if liability_anchor_count > 0 && support_available == 0 {
                return false;
            }
            return true;
        }
        if support_total >= 2 {
            return false;
        }
        if liability_anchor_count == 0 {
            if !any_anchor_available {
                return false;
            }
            return true;
        }
        let uses_soft_anchor_only = strong_liability_support == 0 && soft_liability_support > 0;
        if uses_soft_anchor_only && support_total < 2 {
            return true;
        }
        if support_total == 1 && support_high == 1 {
            let sole_support_is_jack = combo
                .iter()
                .any(|(card, _)| card.suit == Suit::Hearts && card.rank == Rank::Jack)
                && !combo.iter().any(|(card, _)| {
                    card.suit == Suit::Hearts
                        && card.rank >= Rank::Ten
                        && card.rank < Rank::Queen
                        && card.rank != Rank::Jack
                });
            if sole_support_is_jack {
                let remaining_support = input
                    .hand
                    .iter()
                    .filter(|card| is_high_support_heart(card))
                    .filter(|card| combo.iter().all(|(passed, _)| passed != *card))
                    .count();
                if remaining_support > 0 {
                    return true;
                }
            }
        }
        return false;
    }

    if passes_j {
        let has_high_support_anchor = support_high_excl_jack > 0;
        if !has_high_support_anchor && liability_anchor_count == 0 {
            return true;
        }
        return false;
    }

    if passes_ten {
        let supplemental_support = support_high_excl_ten + support_mid;
        if supplemental_support == 0 && liability_anchor_count == 0 {
            return true;
        }
        if combo
            .iter()
            .any(|(card, _)| card.suit == Suit::Hearts && card.rank < Rank::Eight)
        {
            let remaining_high = input
                .hand
                .iter()
                .filter(|card| {
                    card.suit == Suit::Hearts
                        && card.rank >= Rank::Ten
                        && combo.iter().all(|(passed, _)| passed != *card)
                })
                .count();
            if remaining_high >= 1 {
                return true;
            }
        }
    }

    let passed_premium = combo
        .iter()
        .filter(|(card, _)| card.suit == Suit::Hearts && card.rank >= Rank::Queen)
        .count();
    if passed_premium == 0 {
        return false;
    }
    if support_high > 0 {
        return false;
    }
    input
        .hand
        .iter()
        .filter(|card| is_high_support_heart(card))
        .filter(|card| combo.iter().all(|(passed, _)| passed != *card))
        .count()
        > 0
}

fn violates_all_offsuit_guard(
    input: &PassScoreInput<'_>,
    combo: &[(Card, PassScoreBreakdown); 3],
) -> bool {
    if !matches!(input.direction, PassingDirection::Left) {
        return false;
    }
    if combo.iter().any(|(card, _)| card.suit == Suit::Hearts) {
        return false;
    }
    let urgency = input.moon_estimate.defensive_urgency();
    if urgency < 0.6 {
        return false;
    }
    let hearts_in_hand: Vec<Card> = input
        .hand
        .iter()
        .copied()
        .filter(|card| card.suit == Suit::Hearts)
        .collect();
    if hearts_in_hand.len() < 3 {
        return false;
    }
    let ten_plus = hearts_in_hand
        .iter()
        .filter(|card| card.rank >= Rank::Ten)
        .count();
    let premium = hearts_in_hand
        .iter()
        .filter(|card| card.rank >= Rank::Queen)
        .count();
    ten_plus >= 2 || premium >= 1
}

fn violates_ten_plus_safety(
    input: &PassScoreInput<'_>,
    combo: &[(Card, PassScoreBreakdown); 3],
) -> bool {
    if !matches!(input.direction, PassingDirection::Left) {
        return false;
    }
    let passed_ten_plus = combo
        .iter()
        .filter(|(card, _)| card.suit == Suit::Hearts && card.rank >= Rank::Ten)
        .count();
    if passed_ten_plus == 0 {
        return false;
    }
    let premium_passed = combo
        .iter()
        .filter(|(card, _)| card.suit == Suit::Hearts && card.rank >= Rank::Queen)
        .count();
    let passed_high_support = combo
        .iter()
        .filter(|(card, _)| is_high_support_heart(card))
        .count();
    let passed_mid_support = combo
        .iter()
        .filter(|(card, _)| is_mid_support_heart(card))
        .count();
    let passed_support_total = passed_high_support + passed_mid_support;
    let hearts_passed = combo
        .iter()
        .filter(|(card, _)| card.suit == Suit::Hearts)
        .count();
    let passes_single_ten = hearts_passed == 1
        && combo
            .iter()
            .any(|(card, _)| card.suit == Suit::Hearts && card.rank == Rank::Ten);
    let passes_single_jack = hearts_passed == 1
        && combo
            .iter()
            .any(|(card, _)| card.suit == Suit::Hearts && card.rank == Rank::Jack);
    let remaining_ten_plus = input
        .hand
        .iter()
        .filter(|card| card.suit == Suit::Hearts && card.rank >= Rank::Ten)
        .filter(|card| combo.iter().all(|(passed, _)| passed != *card))
        .count();
    let remaining_high_support = input
        .hand
        .iter()
        .filter(|card| is_high_support_heart(card))
        .filter(|card| combo.iter().all(|(passed, _)| passed != *card))
        .count();
    let remaining_mid_support = input
        .hand
        .iter()
        .filter(|card| is_mid_support_heart(card))
        .filter(|card| combo.iter().all(|(passed, _)| passed != *card))
        .count();
    let remaining_support_total = remaining_high_support + remaining_mid_support;
    if premium_passed >= 1
        && passed_support_total == 0
        && (remaining_support_total > 0 || remaining_ten_plus <= 1)
    {
        return true;
    }
    if (passes_single_ten || passes_single_jack)
        && remaining_ten_plus >= 2
        && remaining_support_total > 0
    {
        return true;
    }
    if remaining_ten_plus >= 2 {
        return false;
    }
    let pass_has_ace = combo
        .iter()
        .any(|(card, _)| card.suit == Suit::Hearts && card.rank == Rank::Ace);
    if pass_has_ace {
        return true;
    }
    if premium_passed >= 2 && remaining_ten_plus < 2 && passed_ten_plus >= 2 {
        return true;
    }
    passed_ten_plus >= 3 && remaining_ten_plus == 0
}

fn missing_ten_plus_support(
    input: &PassScoreInput<'_>,
    combo: &[(Card, PassScoreBreakdown); 3],
) -> bool {
    let cards = [combo[0].0, combo[1].0, combo[2].0];
    missing_ten_plus_support_cards(input, &cards)
}

fn missing_ten_plus_support_cards(input: &PassScoreInput<'_>, cards: &[Card; 3]) -> bool {
    let combo_ten_plus = cards
        .iter()
        .filter(|card| card.suit == Suit::Hearts && card.rank >= Rank::Ten)
        .count();
    if combo_ten_plus > 0 {
        return false;
    }
    let hearts_in_combo = cards
        .iter()
        .filter(|card| card.suit == Suit::Hearts)
        .count();
    let liability_offsuit = cards
        .iter()
        .filter(|card| is_offsuit_liability(card))
        .count();
    let remaining_ten_plus = input
        .hand
        .iter()
        .filter(|card| card.suit == Suit::Hearts && card.rank >= Rank::Ten)
        .filter(|card| cards.iter().all(|passed| passed != *card))
        .count();
    if hearts_in_combo <= 1 && liability_offsuit >= 1 && remaining_ten_plus >= 2 {
        return false;
    }
    input
        .hand
        .iter()
        .filter(|card| card.suit == Suit::Hearts && card.rank >= Rank::Ten)
        .any(|card| !cards.iter().any(|passed| passed == card))
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
        if candidates.is_empty() {
            assert!(
                force_guarded_pass(&input).is_some(),
                "expected guarded fallback when candidate enumeration is empty"
            );
        } else {
            assert!(
                candidates.iter().all(|cand| cand.score.is_finite()),
                "unexpected invalid candidate scores: {candidates:?}"
            );
        }
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
        assert!(
            candidates.iter().all(|cand| {
                !(cand.cards.contains(&Card::new(Rank::Ace, Suit::Hearts))
                    && cand.cards.contains(&Card::new(Rank::King, Suit::Hearts))
                    && cand.cards.contains(&Card::new(Rank::Queen, Suit::Hearts)))
            }),
            "triple premium candidate should no longer be generated: {candidates:?}"
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
