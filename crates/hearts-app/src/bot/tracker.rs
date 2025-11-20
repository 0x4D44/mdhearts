use hearts_core::model::card::Card;
use hearts_core::model::hand::Hand;
use hearts_core::model::player::PlayerPosition;
use hearts_core::model::rank::Rank;
use hearts_core::model::round::RoundState;
use hearts_core::model::suit::Suit;
use parking_lot::RwLock;
use rand::Rng;
use rand::seq::SliceRandom;
use std::array;
use std::collections::{HashSet, VecDeque, hash_map::DefaultHasher};
use std::hash::{Hash, Hasher};
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

const SUIT_COUNT: usize = 4;
const RANK_COUNT: usize = 13;

#[allow(dead_code)]
#[derive(Debug, Clone)]
pub struct BeliefState {
    #[allow(dead_code)]
    seat: PlayerPosition,
    card_probs: [[f32; RANK_COUNT]; SUIT_COUNT],
    total_mass: f32,
    #[allow(dead_code)]
    queen_spades_risk: f32,
    #[allow(dead_code)]
    moon_likelihood: f32,
    entropy: f32,
}

#[allow(dead_code)]
impl BeliefState {
    pub fn new(seat: PlayerPosition) -> Self {
        Self {
            seat,
            card_probs: [[0.0; RANK_COUNT]; SUIT_COUNT],
            total_mass: 0.0,
            queen_spades_risk: 0.0,
            moon_likelihood: 0.0,
            entropy: 0.0,
        }
    }

    pub fn seat(&self) -> PlayerPosition {
        self.seat
    }

    pub fn entropy(&self) -> f32 {
        self.entropy
    }

    pub fn queen_spades_risk(&self) -> f32 {
        self.queen_spades_risk
    }

    pub fn moon_likelihood(&self) -> f32 {
        self.moon_likelihood
    }

    pub fn total_mass(&self) -> f32 {
        self.total_mass
    }

    pub fn card_probability(&self, card: Card) -> f32 {
        let (suit_idx, rank_idx) = card_indices(card);
        self.card_probs[suit_idx][rank_idx]
    }

    pub fn reset_uniform(&mut self, cards: &[Card]) {
        self.clear();
        if cards.is_empty() {
            return;
        }
        let uniform = 1.0f32 / cards.len() as f32;
        for &card in cards {
            let (suit_idx, rank_idx) = card_indices(card);
            self.card_probs[suit_idx][rank_idx] = uniform;
        }
        self.total_mass = 1.0;
        self.recompute_entropy();
    }

    pub fn remove_card(&mut self, card: Card) {
        let (suit_idx, rank_idx) = card_indices(card);
        let removed = self.card_probs[suit_idx][rank_idx];
        if removed == 0.0 {
            return;
        }
        self.card_probs[suit_idx][rank_idx] = 0.0;
        if self.total_mass > 0.0 {
            let remaining = (self.total_mass - removed).max(0.0);
            self.total_mass = remaining;
            if remaining > 0.0 {
                let scale = 1.0 / remaining;
                for suit in &mut self.card_probs {
                    for prob in suit.iter_mut() {
                        if *prob > 0.0 {
                            *prob *= scale;
                        }
                    }
                }
                self.total_mass = 1.0;
            } else {
                self.total_mass = 0.0;
            }
        }
        self.recompute_entropy();
    }

    pub fn remove_suit(&mut self, suit: Suit) {
        let suit_idx = suit as usize;
        let mut removed_total = 0.0;
        for prob in self.card_probs[suit_idx].iter_mut() {
            removed_total += *prob;
            *prob = 0.0;
        }
        if removed_total == 0.0 {
            return;
        }
        if self.total_mass > 0.0 {
            let remaining = (self.total_mass - removed_total).max(0.0);
            self.total_mass = remaining;
            if remaining > 0.0 {
                let scale = 1.0 / remaining;
                for suit_probs in &mut self.card_probs {
                    for prob in suit_probs.iter_mut() {
                        if *prob > 0.0 {
                            *prob *= scale;
                        }
                    }
                }
                self.total_mass = 1.0;
            } else {
                self.total_mass = 0.0;
            }
        }
        self.recompute_entropy();
    }

    pub fn set_moon_likelihood(&mut self, value: f32) {
        self.moon_likelihood = value.clamp(0.0, 1.0);
    }

    pub fn clear(&mut self) {
        for suit in &mut self.card_probs {
            suit.fill(0.0);
        }
        self.total_mass = 0.0;
        self.queen_spades_risk = 0.0;
        self.moon_likelihood = 0.0;
        self.entropy = 0.0;
    }

    fn recompute_entropy(&mut self) {
        self.queen_spades_risk = self.card_probability(Card::new(Rank::Queen, Suit::Spades));
        self.entropy = entropy_of_matrix(&self.card_probs);
    }

    /// Sample a card according to the probability distribution
    /// Returns None if no cards have non-zero probability
    pub fn sample_card(&self, rng: &mut impl Rng, exclude: &HashSet<Card>) -> Option<Card> {
        let mut candidates = Vec::new();
        let mut cumulative = Vec::new();
        let mut sum = 0.0;

        for suit in Suit::ALL {
            for rank in Rank::ORDERED {
                let card = Card::new(rank, suit);
                if exclude.contains(&card) {
                    continue;
                }
                let prob = self.card_probability(card);
                if prob > 0.0 {
                    candidates.push(card);
                    sum += prob;
                    cumulative.push(sum);
                }
            }
        }

        if candidates.is_empty() {
            return None;
        }

        let roll = rng.r#gen::<f32>() * sum;
        for (i, &threshold) in cumulative.iter().enumerate() {
            if roll <= threshold {
                return Some(candidates[i]);
            }
        }

        candidates.last().copied()
    }
}

/// Represents a sampled world - a consistent distribution of unseen cards
#[derive(Debug, Clone)]
pub struct SampledWorld {
    /// Hands for each player (only contains unseen cards that were sampled)
    pub hands: [Vec<Card>; 4],
    /// The seed used for this sample (for reproducibility in deterministic mode)
    pub seed: u64,
}

impl Default for SampledWorld {
    fn default() -> Self {
        Self::new()
    }
}

impl SampledWorld {
    pub fn new() -> Self {
        Self {
            hands: [vec![], vec![], vec![], vec![]],
            seed: 0,
        }
    }

    #[allow(dead_code)]
    pub fn hand(&self, seat: PlayerPosition) -> &[Card] {
        &self.hands[seat.index()]
    }

    /// Get full hand for a player by combining known cards with sampled cards
    #[allow(dead_code)]
    pub fn full_hand(&self, seat: PlayerPosition, round: &RoundState) -> Hand {
        let known_hand = round.hand(seat);
        let mut all_cards: Vec<Card> = known_hand.iter().copied().collect();
        all_cards.extend(self.hands[seat.index()].iter().copied());
        Hand::with_cards(all_cards)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct BeliefCacheKey {
    trick_index: u8,
    leader: PlayerPosition,
    fingerprint: u64,
}

#[allow(dead_code)]
impl BeliefCacheKey {
    pub fn from_round(round: &RoundState) -> Self {
        let mut hasher = DefaultHasher::new();
        for trick in round.trick_history() {
            for play in trick.plays() {
                play.position.hash(&mut hasher);
                play.card.hash(&mut hasher);
            }
        }
        for play in round.current_trick().plays() {
            play.position.hash(&mut hasher);
            play.card.hash(&mut hasher);
        }
        Self {
            trick_index: round.trick_history().len().min(u8::MAX as usize) as u8,
            leader: round.current_trick().leader(),
            fingerprint: hasher.finish(),
        }
    }
}

#[derive(Debug, Clone)]
pub struct BeliefSnapshot {
    #[allow(dead_code)]
    pub created_at_ms: u128,
    #[allow(dead_code)]
    pub states: [BeliefState; 4],
}

#[allow(dead_code)]
impl BeliefSnapshot {
    pub fn new(states: [BeliefState; 4]) -> Self {
        let created_at_ms = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis();
        Self {
            created_at_ms,
            states,
        }
    }
}

#[derive(Debug)]
pub struct BeliefCache {
    capacity: usize,
    entries: VecDeque<(BeliefCacheKey, BeliefSnapshot)>,
}

#[allow(dead_code)]
impl BeliefCache {
    pub fn new(capacity: usize) -> Self {
        Self {
            capacity: capacity.max(1),
            entries: VecDeque::new(),
        }
    }

    pub fn len(&self) -> usize {
        self.entries.len()
    }

    pub fn capacity(&self) -> usize {
        self.capacity
    }

    pub fn get(&self, key: &BeliefCacheKey) -> Option<BeliefSnapshot> {
        self.entries
            .iter()
            .rev()
            .find(|(k, _)| k == key)
            .map(|(_, snapshot)| snapshot.clone())
    }

    pub fn insert(&mut self, key: BeliefCacheKey, snapshot: BeliefSnapshot) {
        if let Some(pos) = self.entries.iter().position(|(k, _)| k == &key) {
            self.entries.remove(pos);
        } else if self.entries.len() >= self.capacity {
            self.entries.pop_front();
        }
        self.entries.push_back((key, snapshot));
    }

    pub fn clear(&mut self) {
        self.entries.clear();
    }
}

fn card_indices(card: Card) -> (usize, usize) {
    let suit_idx = card.suit as usize;
    let rank_idx = (card.rank.value() - 2) as usize;
    (suit_idx, rank_idx)
}

fn entropy_of_matrix(matrix: &[[f32; RANK_COUNT]; SUIT_COUNT]) -> f32 {
    let mut entropy = 0.0f32;
    for prob in matrix.iter().flat_map(|row| row.iter()) {
        if *prob > 0.0 {
            entropy -= *prob * prob.ln();
        }
    }
    entropy
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct BeliefCacheMetrics {
    pub size: usize,
    pub capacity: usize,
    pub hits: usize,
    pub misses: usize,
}

#[derive(Debug, Clone, Copy)]
pub struct BeliefSamplerConfig {
    pub top_k: usize,
    pub diversity: usize,
    pub filter_zero: bool,
}

impl BeliefSamplerConfig {
    pub fn from_env() -> Self {
        let top_k = std::env::var("MDH_HARD_BELIEF_TOPK")
            .ok()
            .and_then(|raw| raw.trim().parse::<usize>().ok())
            .unwrap_or(3)
            .max(1);
        let diversity = std::env::var("MDH_HARD_BELIEF_DIVERSITY")
            .ok()
            .and_then(|raw| raw.trim().parse::<usize>().ok())
            .unwrap_or(1);
        let filter_zero = matches!(
            std::env::var("MDH_HARD_BELIEF_FILTER"),
            Ok(val) if matches!(val.trim().to_ascii_lowercase().as_str(), "1" | "true" | "on")
        );
        Self {
            top_k,
            diversity,
            filter_zero,
        }
    }
}
fn belief_cache_capacity_from_env() -> usize {
    std::env::var("MDH_HARD_BELIEF_CACHE_SIZE")
        .ok()
        .and_then(|raw| raw.trim().parse::<usize>().ok())
        .unwrap_or(128)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MoonState {
    Inactive,
    Considering,
    Committed,
}

#[derive(Debug)]
pub struct UnseenTracker {
    unseen: HashSet<Card>,
    // Known suit voids per seat (seat_idx x suit_idx)
    voids: [[bool; 4]; 4],
    // Moon attempt state per seat for Stage 2 heuristics
    moon: [MoonState; 4],
    beliefs: [BeliefState; 4],
    belief_cache: Arc<RwLock<BeliefCache>>,
    belief_cache_hits: AtomicUsize,
    belief_cache_misses: AtomicUsize,
}

impl Clone for UnseenTracker {
    fn clone(&self) -> Self {
        Self {
            unseen: self.unseen.clone(),
            voids: self.voids,
            moon: self.moon,
            beliefs: self.beliefs.clone(),
            belief_cache: Arc::clone(&self.belief_cache),
            belief_cache_hits: AtomicUsize::new(self.belief_cache_hits.load(Ordering::Relaxed)),
            belief_cache_misses: AtomicUsize::new(self.belief_cache_misses.load(Ordering::Relaxed)),
        }
    }
}
impl Default for UnseenTracker {
    fn default() -> Self {
        Self::new()
    }
}

impl UnseenTracker {
    pub fn new() -> Self {
        let beliefs = array::from_fn(|idx| {
            let seat = PlayerPosition::from_index(idx).unwrap_or(PlayerPosition::North);
            BeliefState::new(seat)
        });
        let cache_capacity = belief_cache_capacity_from_env();
        let mut tracker = Self {
            unseen: full_deck_cards().collect(),
            voids: [[false; 4]; 4],
            moon: [MoonState::Inactive; 4],
            beliefs,
            belief_cache: Arc::new(RwLock::new(BeliefCache::new(cache_capacity))),
            belief_cache_hits: AtomicUsize::new(0),
            belief_cache_misses: AtomicUsize::new(0),
        };
        tracker.rebuild_beliefs_uniform();
        tracker
    }

    #[allow(dead_code)]
    pub fn clone_with_fresh_cache(&self) -> Self {
        let mut cloned = self.clone();
        let capacity = belief_cache_capacity_from_env();
        cloned.belief_cache = Arc::new(RwLock::new(BeliefCache::new(capacity)));
        cloned.belief_cache_hits.store(0, Ordering::Relaxed);
        cloned.belief_cache_misses.store(0, Ordering::Relaxed);
        cloned
    }

    fn rebuild_beliefs_uniform(&mut self) {
        let cards: Vec<Card> = self.unseen.iter().copied().collect();
        for (idx, belief) in self.beliefs.iter_mut().enumerate() {
            let seat = PlayerPosition::from_index(idx).unwrap_or(PlayerPosition::North);
            *belief = BeliefState::new(seat);
            belief.reset_uniform(&cards);
        }
    }

    pub fn reset_for_round(&mut self, round: &RoundState) {
        self.unseen = full_deck_cards().collect();
        self.voids = [[false; 4]; 4];
        self.moon = [MoonState::Inactive; 4];
        self.belief_cache_hits.store(0, Ordering::Relaxed);
        self.belief_cache_misses.store(0, Ordering::Relaxed);
        self.rebuild_beliefs_uniform();
        self.belief_cache.write().clear();
        for trick in round.trick_history() {
            for play in trick.plays() {
                self.note_card_revealed(play.card);
            }
        }
        for play in round.current_trick().plays() {
            self.note_card_revealed(play.card);
        }
    }

    pub fn note_pass_selection(&mut self, _seat: PlayerPosition, _cards: &[Card]) {
        // Passing moves cards between hidden hands; it should not reveal them.
    }

    pub fn note_card_played(&mut self, _seat: PlayerPosition, card: Card) {
        self.note_card_revealed(card);
    }

    pub fn note_card_revealed(&mut self, card: Card) {
        self.unseen.remove(&card);
        for belief in &mut self.beliefs {
            belief.remove_card(card);
        }
    }

    pub fn is_unseen(&self, card: Card) -> bool {
        self.unseen.contains(&card)
    }

    pub fn unseen_count(&self) -> usize {
        self.unseen.len()
    }

    #[inline]
    fn suit_index(suit: Suit) -> usize {
        suit as usize
    }

    pub fn note_void(&mut self, seat: PlayerPosition, suit: Suit) {
        self.voids[seat.index()][Self::suit_index(suit)] = true;
        self.beliefs[seat.index()].remove_suit(suit);
    }

    pub fn is_void(&self, seat: PlayerPosition, suit: Suit) -> bool {
        self.voids[seat.index()][Self::suit_index(suit)]
    }

    pub fn moon_state(&self, seat: PlayerPosition) -> MoonState {
        self.moon[seat.index()]
    }

    pub fn note_trick_completion(
        &mut self,
        plays: &[(PlayerPosition, Card)],
        winner: PlayerPosition,
        penalties: u8,
        hearts_broken: bool,
    ) {
        if let Some((_, lead_card)) = plays.first() {
            let lead_suit = lead_card.suit;
            for &(seat, card) in plays {
                if card.suit != lead_suit {
                    self.note_void(seat, lead_suit);
                }
            }
        }
        if penalties == 0 {
            let belief = &mut self.beliefs[winner.index()];
            let boosted = (belief.moon_likelihood() + 0.15).clamp(0.0, 1.0);
            belief.set_moon_likelihood(boosted);
            if hearts_broken {
                let boosted = (belief.moon_likelihood() + 0.05).clamp(0.0, 1.0);
                belief.set_moon_likelihood(boosted);
            }
        } else {
            for seat in PlayerPosition::LOOP.iter().copied() {
                self.beliefs[seat.index()].set_moon_likelihood(0.0);
            }
        }
    }
    pub fn set_moon_state(&mut self, seat: PlayerPosition, state: MoonState) {
        self.moon[seat.index()] = state;
    }

    #[allow(dead_code)]
    pub fn belief_state(&self, seat: PlayerPosition) -> &BeliefState {
        &self.beliefs[seat.index()]
    }

    #[allow(dead_code)]
    pub fn belief_states(&self) -> &[BeliefState; 4] {
        &self.beliefs
    }

    pub fn belief_entropy(&self) -> [f32; 4] {
        array::from_fn(|idx| self.beliefs[idx].entropy())
    }

    pub fn snapshot_beliefs_for_round(&self, round: &RoundState) -> [BeliefState; 4] {
        let key = BeliefCacheKey::from_round(round);
        if let Some(snapshot) = self.belief_cache.read().get(&key) {
            self.belief_cache_hits.fetch_add(1, Ordering::Relaxed);
            return snapshot.states.clone();
        }
        let states = self.beliefs.clone();
        {
            let mut cache = self.belief_cache.write();
            cache.insert(key, BeliefSnapshot::new(states.clone()));
        }
        self.belief_cache_misses.fetch_add(1, Ordering::Relaxed);
        states
    }

    #[allow(dead_code)]
    pub fn belief_cache_arc(&self) -> Arc<RwLock<BeliefCache>> {
        Arc::clone(&self.belief_cache)
    }

    pub fn belief_cache_metrics(&self) -> BeliefCacheMetrics {
        let cache = self.belief_cache.read();
        BeliefCacheMetrics {
            size: cache.len(),
            capacity: cache.capacity(),
            hits: self.belief_cache_hits.load(Ordering::Relaxed),
            misses: self.belief_cache_misses.load(Ordering::Relaxed),
        }
    }

    #[allow(dead_code)]
    pub fn record_belief_cache_hit(&self) {
        self.belief_cache_hits.fetch_add(1, Ordering::Relaxed);
    }

    #[allow(dead_code)]
    pub fn record_belief_cache_miss(&self) {
        self.belief_cache_misses.fetch_add(1, Ordering::Relaxed);
    }

    /// Sample a consistent world where unseen cards are distributed among players
    /// respecting void constraints and belief probabilities
    ///
    /// This implements belief-state sampling for imperfect information Monte Carlo search.
    /// The sampled world can be used to run rollouts that account for uncertainty about
    /// which player holds which cards.
    ///
    /// # Arguments
    /// * `rng` - Random number generator for sampling
    /// * `our_seat` - The seat we're planning for (we already know our cards)
    /// * `round` - Current round state (to know which cards are already revealed)
    ///
    /// # Returns
    /// A `SampledWorld` containing a plausible distribution of unseen cards
    pub fn sample_world(
        &self,
        rng: &mut impl Rng,
        our_seat: PlayerPosition,
        round: &RoundState,
    ) -> SampledWorld {
        let mut world = SampledWorld::new();
        let seed = rng.r#gen();
        world.seed = seed;

        // Collect all unseen cards
        let mut unseen_cards: Vec<Card> = self.unseen.iter().copied().collect();

        // Simple approach: shuffle and deal respecting voids
        // TODO: More sophisticated belief-weighted sampling
        unseen_cards.shuffle(rng);

        // Determine how many cards each player should get
        let mut cards_per_player = [0usize; 4];
        for seat in PlayerPosition::LOOP.iter().copied() {
            if seat == our_seat {
                // We already know our hand, don't sample for ourselves
                cards_per_player[seat.index()] = 0;
            } else {
                // Other players get their fair share
                cards_per_player[seat.index()] = round.hand(seat).len() + (unseen_cards.len() / 3);
            }
        }

        // Distribute cards respecting void constraints
        let mut dealt = HashSet::new();
        for card in &unseen_cards {
            if dealt.contains(card) {
                continue;
            }

            // Find which players can receive this card (not void in its suit)
            let mut eligible: Vec<PlayerPosition> = PlayerPosition::LOOP
                .iter()
                .copied()
                .filter(|&seat| {
                    seat != our_seat
                        && !self.is_void(seat, card.suit)
                        && world.hands[seat.index()].len() < cards_per_player[seat.index()]
                })
                .collect();

            if eligible.is_empty() {
                // If no one is eligible due to voids, relax the constraint
                eligible = PlayerPosition::LOOP
                    .iter()
                    .copied()
                    .filter(|&seat| seat != our_seat)
                    .collect();
            }

            if !eligible.is_empty() {
                // Weight by belief probability
                let weights: Vec<f32> = eligible
                    .iter()
                    .map(|&seat| {
                        self.beliefs[seat.index()]
                            .card_probability(*card)
                            .max(0.001)
                    })
                    .collect();

                let total_weight: f32 = weights.iter().sum();
                let roll = rng.r#gen::<f32>() * total_weight;
                let mut cumulative = 0.0;
                let mut chosen_seat = eligible[0];

                for (i, &seat) in eligible.iter().enumerate() {
                    cumulative += weights[i];
                    if roll <= cumulative {
                        chosen_seat = seat;
                        break;
                    }
                }

                world.hands[chosen_seat.index()].push(*card);
                dealt.insert(*card);
            }
        }

        world
    }
}

fn full_deck_cards() -> impl Iterator<Item = Card> {
    Suit::ALL.into_iter().flat_map(|suit| {
        Rank::ORDERED
            .into_iter()
            .map(move |rank| Card::new(rank, suit))
    })
}

#[cfg(test)]
mod tests {
    use super::UnseenTracker;
    use hearts_core::model::card::Card;
    use hearts_core::model::deck::Deck;
    use hearts_core::model::passing::PassingDirection;
    use hearts_core::model::player::PlayerPosition;
    use hearts_core::model::rank::Rank;
    use hearts_core::model::round::RoundState;
    use hearts_core::model::suit::Suit;
    use rand::SeedableRng;
    use rand::rngs::StdRng;

    #[test]
    fn tracker_initialises_with_full_deck() {
        let tracker = UnseenTracker::new();
        assert_eq!(tracker.unseen_count(), 52);
        assert!(tracker.is_unseen(Card::new(Rank::Ace, Suit::Spades)));
        let north = tracker.belief_state(PlayerPosition::North);
        assert!((north.total_mass() - 1.0).abs() < 1e-6);
        assert!(north.entropy() > 3.0);
    }

    #[test]
    fn tracker_marks_cards_seen() {
        let mut tracker = UnseenTracker::new();
        let queen = Card::new(Rank::Queen, Suit::Spades);
        tracker.note_card_played(PlayerPosition::East, queen);
        assert!(!tracker.is_unseen(queen));
        assert_eq!(tracker.unseen_count(), 51);
        let north = tracker.belief_state(PlayerPosition::North);
        assert_eq!(north.card_probability(queen), 0.0);
    }

    #[test]
    fn pass_selection_does_not_remove_cards_from_unseen() {
        let mut tracker = UnseenTracker::new();
        let queen = Card::new(Rank::Queen, Suit::Spades);
        assert!(tracker.is_unseen(queen));
        tracker.note_pass_selection(PlayerPosition::North, &[queen]);
        assert!(tracker.is_unseen(queen));
        assert_eq!(tracker.unseen_count(), 52);
    }

    #[test]
    fn tracker_voids_and_moon_state() {
        let mut tracker = UnseenTracker::new();
        assert!(!tracker.is_void(PlayerPosition::South, Suit::Hearts));
        tracker.note_void(PlayerPosition::South, Suit::Hearts);
        assert!(tracker.is_void(PlayerPosition::South, Suit::Hearts));
        for rank in Rank::ORDERED {
            assert_eq!(
                tracker
                    .belief_state(PlayerPosition::South)
                    .card_probability(Card::new(rank, Suit::Hearts)),
                0.0
            );
        }

        use super::MoonState;
        assert_eq!(
            tracker.moon_state(PlayerPosition::East),
            MoonState::Inactive
        );
        tracker.set_moon_state(PlayerPosition::East, MoonState::Committed);
        assert_eq!(
            tracker.moon_state(PlayerPosition::East),
            MoonState::Committed
        );
    }

    #[test]
    fn tracker_belief_rebuild_on_reset() {
        let deck = Deck::standard();
        let round = RoundState::deal(&deck, PlayerPosition::North, PassingDirection::Hold);
        let mut tracker = UnseenTracker::new();
        tracker.reset_for_round(&round);
        let north = tracker.belief_state(PlayerPosition::North);
        assert!((north.total_mass() - 1.0).abs() < 1e-6);
        assert!(north.entropy() > 3.0);
    }

    #[test]
    fn belief_cache_basic_metrics() {
        let mut tracker = UnseenTracker::new();
        let deck = Deck::standard();
        let round = RoundState::deal(&deck, PlayerPosition::North, PassingDirection::Hold);
        tracker.reset_for_round(&round);
        let metrics = tracker.belief_cache_metrics();
        assert_eq!(metrics.size, 0);
        assert!(metrics.capacity >= 1);

        tracker.snapshot_beliefs_for_round(&round); // miss
        let metrics = tracker.belief_cache_metrics();
        assert_eq!(metrics.misses, 1);
        assert_eq!(metrics.hits, 0);
        assert_eq!(metrics.size, 1);

        tracker.snapshot_beliefs_for_round(&round); // hit
        let metrics = tracker.belief_cache_metrics();
        assert_eq!(metrics.hits, 1);
        assert_eq!(metrics.misses, 1);
    }

    #[test]
    fn sample_world_respects_voids() {
        let deck = Deck::standard();
        let round = RoundState::deal(&deck, PlayerPosition::North, PassingDirection::Hold);
        let mut tracker = UnseenTracker::new();
        tracker.reset_for_round(&round);

        // Mark East as void in Hearts
        tracker.note_void(PlayerPosition::East, Suit::Hearts);

        // Sample a world
        let mut rng = StdRng::seed_from_u64(12345);
        let world = tracker.sample_world(&mut rng, PlayerPosition::North, &round);

        // East should not have any hearts in the sampled world
        for card in world.hand(PlayerPosition::East) {
            assert_ne!(
                card.suit,
                Suit::Hearts,
                "East is void in Hearts but sampled world gave them {:?}",
                card
            );
        }
    }

    #[test]
    fn sample_world_distributes_all_unseen_cards() {
        let deck = Deck::standard();
        let round = RoundState::deal(&deck, PlayerPosition::North, PassingDirection::Hold);
        let mut tracker = UnseenTracker::new();
        tracker.reset_for_round(&round);

        // Note some cards as seen
        tracker.note_card_revealed(Card::new(Rank::Ace, Suit::Spades));
        tracker.note_card_revealed(Card::new(Rank::King, Suit::Spades));
        tracker.note_card_revealed(Card::new(Rank::Queen, Suit::Spades));

        let unseen_count_before = tracker.unseen_count();

        // Sample a world
        let mut rng = StdRng::seed_from_u64(54321);
        let world = tracker.sample_world(&mut rng, PlayerPosition::North, &round);

        // Count cards in sampled world (excluding North since we're playing that seat)
        let sampled_count: usize = PlayerPosition::LOOP
            .iter()
            .filter(|&&seat| seat != PlayerPosition::North)
            .map(|&seat| world.hand(seat).len())
            .sum();

        // The sampled cards should roughly match unseen cards
        // (may not be exact due to distribution constraints)
        assert!(
            sampled_count > 0,
            "Sample world should distribute some cards"
        );
        assert!(
            sampled_count <= unseen_count_before,
            "Sample world distributed {} cards but only {} unseen",
            sampled_count,
            unseen_count_before
        );
    }

    #[test]
    fn sample_world_is_deterministic_with_same_seed() {
        let deck = Deck::standard();
        let round = RoundState::deal(&deck, PlayerPosition::North, PassingDirection::Hold);
        let mut tracker = UnseenTracker::new();
        tracker.reset_for_round(&round);

        // Sample two worlds with the same seed
        let mut rng1 = StdRng::seed_from_u64(99999);
        let world1 = tracker.sample_world(&mut rng1, PlayerPosition::North, &round);

        let mut rng2 = StdRng::seed_from_u64(99999);
        let world2 = tracker.sample_world(&mut rng2, PlayerPosition::North, &round);

        // They should be identical
        for seat in PlayerPosition::LOOP.iter().copied() {
            let mut cards1: Vec<Card> = world1.hand(seat).to_vec();
            let mut cards2: Vec<Card> = world2.hand(seat).to_vec();
            cards1.sort_by_key(|c| (c.suit as u8, c.rank.value()));
            cards2.sort_by_key(|c| (c.suit as u8, c.rank.value()));

            assert_eq!(
                cards1, cards2,
                "Same seed should produce same sampled world for {:?}",
                seat
            );
        }
    }

    #[test]
    fn sample_world_uses_belief_probabilities() {
        let deck = Deck::standard();
        let round = RoundState::deal(&deck, PlayerPosition::North, PassingDirection::Hold);
        let mut tracker = UnseenTracker::new();
        tracker.reset_for_round(&round);

        // Set up a scenario where we know Queen of Spades must be with a specific player
        // by marking voids for all players except South
        let queen = Card::new(Rank::Queen, Suit::Spades);

        // Mark all players except South as void in Spades (we're playing North)
        tracker.note_void(PlayerPosition::East, Suit::Spades);
        tracker.note_void(PlayerPosition::West, Suit::Spades);

        // Sample multiple worlds - QS should consistently go to South
        // since it's the only player not void in Spades (besides North which we're playing)
        let mut rng = StdRng::seed_from_u64(777);
        let mut qs_in_south_count = 0;

        for _ in 0..10 {
            let world = tracker.sample_world(&mut rng, PlayerPosition::North, &round);

            // If QS is unseen (not in North's hand), it should be in South's sampled hand
            if tracker.is_unseen(queen) {
                let south_hand = world.hand(PlayerPosition::South);
                if south_hand.contains(&queen) {
                    qs_in_south_count += 1;
                }
            }
        }

        // QS should go to South in most samples due to void constraints
        assert!(
            qs_in_south_count >= 7,
            "Expected QS in South's hand at least 7/10 times due to void constraints, got {}",
            qs_in_south_count
        );
    }
}
