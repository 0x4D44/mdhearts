//! World sampling utilities operating on the belief state.

use super::{Belief, BeliefCacheKey, SamplerCache};
use crate::model::card::Card;
use crate::model::hand::Hand;
use crate::model::player::PlayerPosition;
use crate::model::suit::Suit;
use rand::Rng;
use std::array;
use std::fmt;

/// Maximum tolerance when checking whether a probability column is effectively one.
const PROBABILITY_ONE_EPSILON: f32 = 1e-6;

/// Samples weighted worlds from a [`Belief`] distribution.
#[derive(Debug, Default)]
pub struct BeliefSampler;

impl BeliefSampler {
    /// Samples a world using weighted sampling without replacement.
    pub fn sample_world<R: Rng + ?Sized>(
        belief: &Belief,
        quotas: &SuitQuotas,
        rng: &mut R,
    ) -> Result<SampledWorld, SamplingError> {
        Self::sample_world_with_cache(belief, quotas, rng, None, None, 1, None)
    }

    /// Samples a world, consulting and updating the supplied cache when available.
    ///
    /// The sampler will attempt up to `max_attempts` draws, invoking a repair pass when quota
    /// violations are detected. On success, the sampled world is stored in the cache.
    pub fn sample_world_with_cache<R: Rng + ?Sized>(
        belief: &Belief,
        quotas: &SuitQuotas,
        rng: &mut R,
        mut cache: Option<&mut SamplerCache>,
        cache_key: Option<BeliefCacheKey>,
        max_attempts: usize,
        stats: Option<&mut SamplingStats>,
    ) -> Result<SampledWorld, SamplingError> {
        if let (Some(cache_ref), Some(key_ref)) = (cache.as_ref(), cache_key.as_ref()) {
            if let Some(existing) = cache_ref
                .get(key_ref)
                .and_then(|worlds| worlds.first())
                .cloned()
            {
                return Ok(existing);
            }
        }

        let attempts = max_attempts.max(1);
        let mut last_error: Option<SamplingError> = None;

        let mut stats = stats;

        for _ in 0..attempts {
            if let Some(inner) = stats.as_deref_mut() {
                inner.attempts += 1;
            }
            match sample_world_once(belief, rng) {
                Ok(mut world) => {
                    if quotas.check(world.hands()) || attempt_repair(&mut world, quotas) {
                        if let Some(inner) = stats.as_deref_mut() {
                            inner.succeeded += 1;
                            if !quotas.check(world.hands()) {
                                inner.repairs += 1;
                            }
                        }
                        match recompute_log_weight(belief, &world) {
                            Ok(weight) => {
                                world.set_log_weight(weight);
                                if let Some(key) = cache_key.clone() {
                                    if let Some(cache_mut) = cache.as_mut() {
                                        (**cache_mut).insert(key, world.clone());
                                    }
                                }
                                return Ok(world);
                            }
                            Err(err) => last_error = Some(err),
                        }
                    } else {
                        if let Some(inner) = stats.as_deref_mut() {
                            inner.rejections += 1;
                        }
                        last_error = Some(SamplingError::QuotaMismatch);
                    }
                }
                Err(err) => last_error = Some(err),
            }
        }

        if let Some(inner) = stats.as_deref_mut() {
            if inner.succeeded == 0 && inner.rejections == 0 {
                inner.rejections += 1;
            }
        }

        Err(last_error.unwrap_or(SamplingError::QuotaMismatch))
    }
}

fn sample_world_once<R: Rng + ?Sized>(
    belief: &Belief,
    rng: &mut R,
) -> Result<SampledWorld, SamplingError> {
    let perspective = belief.perspective();
    let mut hands = array::from_fn(|_| Hand::new());
    let mut available_cards = Vec::new();

    // Split cards between known perspective ownership and candidates for opponents.
    for card_id in 0..52 {
        let card = match Card::from_id(card_id as u8) {
            Some(card) => card,
            None => continue,
        };
        let column_mass: f32 = PlayerPosition::LOOP
            .iter()
            .map(|seat| belief.prob_card(*seat, card))
            .sum();

        if column_mass == 0.0 {
            continue; // already played or impossible.
        }

        let perspective_prob = belief.prob_card(perspective, card);
        if (perspective_prob - 1.0).abs() < PROBABILITY_ONE_EPSILON {
            hands[perspective.index()].add(card);
        } else {
            available_cards.push(card);
        }
    }

    let expected_perspective = belief.expected_hand_size(perspective) as usize;
    if hands[perspective.index()].len() != expected_perspective {
        return Err(SamplingError::InconsistentHandSize { seat: perspective });
    }

    let mut log_weight = 0.0_f32;

    for seat in PlayerPosition::LOOP {
        if seat == perspective {
            continue;
        }

        let target_size = belief.expected_hand_size(seat) as usize;
        let current_size = hands[seat.index()].len();

        if target_size < current_size {
            return Err(SamplingError::InconsistentHandSize { seat });
        }

        for _ in current_size..target_size {
            let selection = select_weighted_card(seat, &available_cards, belief, rng)
                .ok_or(SamplingError::NoFeasibleCard { seat })?;

            let (card_index, card, normalized_prob) = selection;
            hands[seat.index()].add(card);
            log_weight += normalized_prob.ln();
            available_cards.swap_remove(card_index);
        }
    }

    if !available_cards.is_empty() {
        // Remaining cards indicate the belief did not distribute probabilities across all seats.
        return Err(SamplingError::UnassignedCards {
            remaining: available_cards.len(),
        });
    }

    Ok(SampledWorld::new(hands, log_weight))
}

fn recompute_log_weight(belief: &Belief, world: &SampledWorld) -> Result<f32, SamplingError> {
    let mut weight = 0.0_f32;
    for seat in PlayerPosition::LOOP {
        if seat == belief.perspective() {
            continue;
        }
        for &card in world.hand(seat).cards() {
            let prob = belief.prob_card(seat, card);
            if prob <= 0.0 {
                return Err(SamplingError::NoFeasibleCard { seat });
            }
            weight += prob.ln();
        }
    }
    Ok(weight)
}

fn attempt_repair(world: &mut SampledWorld, quotas: &SuitQuotas) -> bool {
    if !quotas.enforced() {
        return true;
    }

    let mut diff = [[0i8; 4]; 4];
    for seat in PlayerPosition::LOOP {
        for suit in Suit::ALL {
            let actual = world
                .hand(seat)
                .cards()
                .iter()
                .filter(|card| card.suit == suit)
                .count() as i8;
            let required = quotas.required(seat, suit) as i8;
            diff[seat.index()][suit as usize] = actual - required;
        }
    }

    let mut iterations = 0;
    while let Some((need_seat, suit_needed)) = find_deficit(&diff) {
        iterations += 1;
        if iterations > 64 {
            return false;
        }

        let donor = match find_donor(&diff, suit_needed) {
            Some(seat) => seat,
            None => return false,
        };

        let swap_suit = match find_surplus_suit(&diff, need_seat) {
            Some(suit) => suit,
            None => return false,
        };

        let (donor_hand, need_hand) = {
            let hands = world.hands_mut();
            split_two_mut(hands, donor.index(), need_seat.index())
        };

        let card_needed = match take_card(donor_hand, suit_needed) {
            Some(card) => card,
            None => return false,
        };
        let card_to_return = match take_card(need_hand, swap_suit) {
            Some(card) => card,
            None => return false,
        };

        need_hand.add(card_needed);
        donor_hand.add(card_to_return);

        diff[donor.index()][suit_needed as usize] -= 1;
        diff[need_seat.index()][suit_needed as usize] += 1;
        diff[need_seat.index()][swap_suit as usize] -= 1;
        diff[donor.index()][swap_suit as usize] += 1;
    }

    diff.iter()
        .flat_map(|row| row.iter())
        .all(|value| *value == 0)
}

fn find_deficit(diff: &[[i8; 4]; 4]) -> Option<(PlayerPosition, Suit)> {
    for seat in PlayerPosition::LOOP {
        for suit in Suit::ALL {
            if diff[seat.index()][suit as usize] < 0 {
                return Some((seat, suit));
            }
        }
    }
    None
}

fn find_donor(diff: &[[i8; 4]; 4], suit: Suit) -> Option<PlayerPosition> {
    for seat in PlayerPosition::LOOP {
        if diff[seat.index()][suit as usize] > 0 {
            return Some(seat);
        }
    }
    None
}

fn find_surplus_suit(diff: &[[i8; 4]; 4], seat: PlayerPosition) -> Option<Suit> {
    for suit in Suit::ALL {
        if diff[seat.index()][suit as usize] > 0 {
            return Some(suit);
        }
    }
    None
}

fn take_card(hand: &mut Hand, suit: Suit) -> Option<Card> {
    let card = hand
        .cards()
        .iter()
        .copied()
        .find(|card| card.suit == suit)?;
    let removed = hand.remove(card);
    debug_assert!(removed, "failed to remove expected card {}", card);
    Some(card)
}

fn split_two_mut<T>(slice: &mut [T], a: usize, b: usize) -> (&mut T, &mut T) {
    assert_ne!(a, b);
    if a < b {
        let (left, right) = slice.split_at_mut(b);
        (&mut left[a], &mut right[0])
    } else {
        let (left, right) = slice.split_at_mut(a);
        (&mut right[0], &mut left[b])
    }
}

#[derive(Debug, Default, Clone, Copy)]
pub struct SamplingStats {
    pub attempts: usize,
    pub succeeded: usize,
    pub repairs: usize,
    pub rejections: usize,
}

/// Represents the suit quotas we attempt to satisfy during sampling.
#[derive(Debug, Clone)]
pub struct SuitQuotas {
    per_seat: [[u8; 4]; 4],
    enforce: bool,
}

impl SuitQuotas {
    /// Creates a quota configuration that does not enforce any per-suit counts.
    pub fn disabled() -> Self {
        Self {
            per_seat: [[0; 4]; 4],
            enforce: false,
        }
    }

    /// Creates a quota configuration that enforces exact per-suit counts.
    pub fn new(per_seat: [[u8; 4]; 4]) -> Self {
        Self {
            per_seat,
            enforce: true,
        }
    }

    pub fn enforced(&self) -> bool {
        self.enforce
    }

    pub fn required(&self, seat: PlayerPosition, suit: Suit) -> u8 {
        self.per_seat[seat.index()][suit as usize]
    }

    /// Returns true if quotas are not enforced or the provided hands satisfy them.
    pub fn check(&self, hands: &[Hand; 4]) -> bool {
        if !self.enforce {
            return true;
        }

        for seat in PlayerPosition::LOOP {
            for suit in Suit::ALL {
                let required = self.per_seat[seat.index()][suit as usize];
                let actual = hands[seat.index()]
                    .cards()
                    .iter()
                    .filter(|card| card.suit == suit)
                    .count() as u8;
                if actual != required {
                    return false;
                }
            }
        }

        true
    }

    /// Builds quotas from an existing sampled world (primarily used in tests).
    #[cfg(test)]
    pub(crate) fn from_world(world: &SampledWorld) -> Self {
        let mut counts = [[0u8; 4]; 4];
        for seat in PlayerPosition::LOOP {
            for suit in Suit::ALL {
                let total = world
                    .hand(seat)
                    .cards()
                    .iter()
                    .filter(|card| card.suit == suit)
                    .count() as u8;
                counts[seat.index()][suit as usize] = total;
            }
        }
        Self::new(counts)
    }
}

/// Outcome of a belief sample, including the world representation and its log-probability weight.
#[derive(Debug, Clone)]
pub struct SampledWorld {
    hands: [Hand; 4],
    log_weight: f32,
}

impl SampledWorld {
    pub(crate) fn new(hands: [Hand; 4], log_weight: f32) -> Self {
        Self { hands, log_weight }
    }

    /// Returns the log-probability weight of the sampled world.
    pub fn log_weight(&self) -> f32 {
        self.log_weight
    }

    pub(crate) fn set_log_weight(&mut self, weight: f32) {
        self.log_weight = weight;
    }

    /// Returns a reference to the sampled hand for `seat`.
    pub fn hand(&self, seat: PlayerPosition) -> &Hand {
        &self.hands[seat.index()]
    }

    /// Exposes all hands for downstream consumers (e.g., cache, sampler tests).
    pub fn hands(&self) -> &[Hand; 4] {
        &self.hands
    }

    pub(crate) fn hands_mut(&mut self) -> &mut [Hand; 4] {
        &mut self.hands
    }
}

/// Errors that can arise while sampling from a belief distribution.
#[derive(Debug)]
pub enum SamplingError {
    InconsistentHandSize { seat: PlayerPosition },
    NoFeasibleCard { seat: PlayerPosition },
    QuotaMismatch,
    UnassignedCards { remaining: usize },
}

impl fmt::Display for SamplingError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            SamplingError::InconsistentHandSize { seat } => {
                write!(f, "seat {seat} has inconsistent expected hand size")
            }
            SamplingError::NoFeasibleCard { seat } => {
                write!(f, "no feasible card to assign to {seat}")
            }
            SamplingError::QuotaMismatch => write!(f, "sampled world violates suit quotas"),
            SamplingError::UnassignedCards { remaining } => {
                write!(f, "{remaining} cards left unassigned after sampling")
            }
        }
    }
}

impl std::error::Error for SamplingError {}

fn select_weighted_card<R: Rng + ?Sized>(
    seat: PlayerPosition,
    available: &[Card],
    belief: &Belief,
    rng: &mut R,
) -> Option<(usize, Card, f32)> {
    let mut weighted_cards = Vec::new();
    let mut total_weight = 0.0_f32;

    for (idx, &card) in available.iter().enumerate() {
        let weight = belief.prob_card(seat, card);
        if weight > 0.0 {
            total_weight += weight;
            weighted_cards.push((idx, card, weight));
        }
    }

    if total_weight <= 0.0 {
        return None;
    }

    let mut choice = rng.gen_range(0.0..total_weight);

    for (idx, card, weight) in weighted_cards {
        if choice <= weight {
            let normalized = weight / total_weight;
            return Some((idx, card, normalized));
        }
        choice -= weight;
    }

    None
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::belief::{BeliefCacheKey, SuitCounts, SuitMask};
    use crate::model::card::Card;
    use crate::model::deck::Deck;
    use crate::model::hand::Hand;
    use crate::model::passing::PassingDirection;
    use crate::model::player::PlayerPosition;
    use crate::model::rank::Rank;
    use crate::model::round::RoundState;
    use crate::model::suit::Suit;
    use rand::SeedableRng;
    use rand::rngs::SmallRng;

    #[test]
    fn deterministic_with_fixed_seed() {
        let deck = Deck::shuffled_with_seed(7);
        let round = RoundState::deal(&deck, PlayerPosition::North, PassingDirection::Hold);
        let belief = Belief::from_state(&round, PlayerPosition::South);
        let quotas = SuitQuotas::disabled();

        let mut rng_a = SmallRng::seed_from_u64(123);
        let mut rng_b = SmallRng::seed_from_u64(123);

        let world_a =
            BeliefSampler::sample_world(&belief, &quotas, &mut rng_a).expect("sample succeeds");
        let world_b =
            BeliefSampler::sample_world(&belief, &quotas, &mut rng_b).expect("sample succeeds");

        for seat in PlayerPosition::LOOP {
            assert_eq!(
                world_a.hand(seat).cards(),
                world_b.hand(seat).cards(),
                "seat {seat} differs between deterministic samples"
            );
            assert_eq!(
                world_a.hand(seat).len(),
                belief.expected_hand_size(seat) as usize
            );
        }
        assert!(
            (world_a.log_weight() - world_b.log_weight()).abs() < f32::EPSILON,
            "log weights should match"
        );
    }

    #[test]
    fn quota_failure_is_reported() {
        let belief = Belief::new_uninitialized(PlayerPosition::South);
        let quotas = SuitQuotas::new([[13, 0, 0, 0]; 4]);
        let mut rng = SmallRng::seed_from_u64(1);
        let result = BeliefSampler::sample_world(&belief, &quotas, &mut rng);
        assert!(matches!(
            result,
            Err(SamplingError::InconsistentHandSize { .. })
        ));
    }

    #[test]
    fn quotas_can_be_verified_from_world() {
        let deck = Deck::shuffled_with_seed(99);
        let round = RoundState::deal(&deck, PlayerPosition::North, PassingDirection::Hold);
        let belief = Belief::from_state(&round, PlayerPosition::South);
        let quotas = SuitQuotas::disabled();
        let mut rng = SmallRng::seed_from_u64(42);
        let world = BeliefSampler::sample_world(&belief, &quotas, &mut rng).unwrap();
        let enforced = SuitQuotas::from_world(&world);
        assert!(enforced.check(world.hands()));
    }

    #[test]
    fn repair_swaps_cards_to_meet_quota() {
        let mut hands = array::from_fn(|_| Hand::new());
        hands[PlayerPosition::North.index()].add(Card::new(Rank::Two, Suit::Hearts));
        hands[PlayerPosition::North.index()].add(Card::new(Rank::Three, Suit::Hearts));
        hands[PlayerPosition::East.index()].add(Card::new(Rank::Four, Suit::Clubs));
        hands[PlayerPosition::East.index()].add(Card::new(Rank::Five, Suit::Clubs));

        let quotas = SuitQuotas::new([[1, 0, 0, 1], [1, 0, 0, 1], [0, 0, 0, 0], [0, 0, 0, 0]]);

        let mut world = SampledWorld::new(hands, 0.0);
        assert!(!quotas.check(world.hands()));
        assert!(attempt_repair(&mut world, &quotas));
        assert!(quotas.check(world.hands()));
    }

    #[test]
    fn cache_hit_short_circuits_sampling() {
        let deck = Deck::shuffled_with_seed(5);
        let round = RoundState::deal(&deck, PlayerPosition::North, PassingDirection::Hold);
        let belief = Belief::from_state(&round, PlayerPosition::South);
        let quotas = SuitQuotas::disabled();

        let mut rng = SmallRng::seed_from_u64(11);
        let cached_world =
            BeliefSampler::sample_world(&belief, &quotas, &mut rng).expect("sampling succeeds");

        let key = BeliefCacheKey::new(0, None, [SuitMask::EMPTY; 4], SuitCounts::new_full_deck());

        let mut cache = SamplerCache::new(4);
        cache.insert(key.clone(), cached_world.clone());

        let mut rng_alt = SmallRng::seed_from_u64(12345);
        let result = BeliefSampler::sample_world_with_cache(
            &belief,
            &quotas,
            &mut rng_alt,
            Some(&mut cache),
            Some(key),
            1,
            None,
        )
        .expect("cache hit succeeds");

        for seat in PlayerPosition::LOOP {
            assert_eq!(result.hand(seat).cards(), cached_world.hand(seat).cards());
        }
    }
}
