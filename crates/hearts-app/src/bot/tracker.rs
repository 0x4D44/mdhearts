use hearts_core::model::card::Card;
use hearts_core::model::player::PlayerPosition;
use hearts_core::model::rank::Rank;
use hearts_core::model::round::RoundState;
use hearts_core::model::suit::Suit;
use parking_lot::RwLock;
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

    pub fn note_pass_selection(&mut self, _seat: PlayerPosition, cards: &[Card]) {
        for &card in cards {
            self.note_card_revealed(card);
        }
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
}
