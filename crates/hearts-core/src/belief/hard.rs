//! Deterministic (hard) belief updates and data structures.

use super::soft::SoftLikelihoodModel;
use crate::model::card::Card;
use crate::model::hand::Hand;
use crate::model::player::PlayerPosition;
use crate::model::rank::Rank;
use crate::model::round::RoundState;
use crate::model::suit::Suit;
use std::array;
use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};

/// Probability model over card ownership for each seat.
#[derive(Debug, Clone)]
pub struct Belief {
    perspective: PlayerPosition,
    probs: [[f32; 52]; 4],
    voids: [SuitMask; 4],
    remain: SuitCounts,
    hand_counts: [u8; 4],
    trick_index: u8,
}

impl Belief {
    /// Creates a belief seeded with uniform probabilities over unseen cards.
    pub fn new_uninitialized(perspective: PlayerPosition) -> Self {
        Self {
            perspective,
            probs: array_init_uniform(),
            voids: [SuitMask::EMPTY; 4],
            remain: SuitCounts::new_full_deck(),
            hand_counts: [13; 4],
            trick_index: 0,
        }
    }

    /// Constructs a belief from the current round state using hard constraints only.
    pub fn from_state(round: &RoundState, perspective: PlayerPosition) -> Self {
        let mut belief = Self::new_uninitialized(perspective);
        belief.rebuild_from_round(round);
        belief
    }

    /// Returns the probability that `card` belongs to `seat`.
    pub fn prob_card(&self, seat: PlayerPosition, card: Card) -> f32 {
        self.probs[seat.index()][card.to_id() as usize]
    }

    pub fn summary_hash(&self) -> u64 {
        let mut hasher = DefaultHasher::new();
        self.trick_index.hash(&mut hasher);
        self.remain.hash(&mut hasher);
        for seat in PlayerPosition::LOOP {
            self.voids[seat.index()].hash(&mut hasher);
            self.hand_counts[seat.index()].hash(&mut hasher);
            for suit in Suit::ALL {
                let slice = &self.probs[seat.index()][suit as usize * 13..suit as usize * 13 + 13];
                let quantised: u16 = slice.iter().enumerate().fold(0u16, |acc, (idx, prob)| {
                    let q = ((*prob * 1000.0).round() as i32).clamp(0, 1000) as u16;
                    acc.wrapping_add(q.wrapping_mul((idx as u16) ^ 0x9d))
                });
                quantised.hash(&mut hasher);
            }
        }
        hasher.finish()
    }

    pub fn iter_suit_probs(&self, seat: PlayerPosition, suit: Suit) -> impl Iterator<Item = &f32> {
        let start = suit as usize * 13;
        let end = start + 13;
        self.probs[seat.index()][start..end].iter()
    }

    /// Returns the void mask maintained for `seat`.
    pub fn void_mask(&self, seat: PlayerPosition) -> SuitMask {
        self.voids[seat.index()]
    }

    /// Number of cards remaining per suit across all players.
    pub fn remaining_suits(&self) -> SuitCounts {
        self.remain
    }

    /// Returns the trick index the belief is currently aligned with.
    pub fn trick_index(&self) -> u8 {
        self.trick_index
    }

    /// Returns the number of cards the model expects `seat` to hold.
    pub fn expected_hand_size(&self, seat: PlayerPosition) -> u8 {
        self.hand_counts[seat.index()]
    }

    /// Seat this belief is computed from.
    pub fn perspective(&self) -> PlayerPosition {
        self.perspective
    }

    /// Applies a hard update after observing a card play.
    pub fn on_card_played(&mut self, seat: PlayerPosition, card: Card, ctx: &BeliefUpdateCtx) {
        let card_index = card.to_id() as usize;
        self.zero_column(card_index);
        if let Some(lead) = ctx.lead_suit {
            if lead != card.suit {
                self.apply_void(seat, lead);
            }
        }
        if self.hand_counts[seat.index()] > 0 {
            self.hand_counts[seat.index()] -= 1;
        }
        self.remain.decrement(card.suit);
        self.trick_index = ctx.trick_index;
    }

    /// Applies both hard constraints and optional soft likelihood adjustments.
    pub fn on_card_played_with_soft(
        &mut self,
        seat: PlayerPosition,
        card: Card,
        ctx: &BeliefUpdateCtx,
        soft: Option<&SoftLikelihoodModel>,
    ) {
        self.on_card_played(seat, card, ctx);
        if let Some(model) = soft {
            model.update_after_play(self, seat, card, ctx);
        }
    }

    fn rebuild_from_round(&mut self, round: &RoundState) {
        self.trick_index = round.tricks_completed() as u8;
        self.hand_counts = compute_hand_counts(round);
        self.remain = compute_remaining_suits(round);
        self.voids = compute_voids(round, self.perspective);
        let played = collect_played(round);
        let my_hand = round.hand(self.perspective).clone();

        for card_id in 0..52 {
            let card = Card::from_id(card_id as u8).expect("valid card id");
            self.zero_column(card_id);
            if played[card_id] {
                continue;
            }

            if my_hand.contains(card) {
                self.probs[self.perspective.index()][card_id] = 1.0;
                continue;
            }

            let mut candidates = Vec::new();
            for seat in PlayerPosition::LOOP {
                if seat == self.perspective {
                    continue;
                }

                if self.hand_counts[seat.index()] == 0 {
                    continue;
                }

                if self.voids[seat.index()].contains(card.suit) {
                    continue;
                }

                candidates.push(seat);
            }

            if candidates.is_empty() {
                for seat in PlayerPosition::LOOP {
                    if seat != self.perspective && self.hand_counts[seat.index()] > 0 {
                        candidates.push(seat);
                    }
                }
            }

            if candidates.is_empty() {
                continue;
            }

            let prob = 1.0 / candidates.len() as f32;
            for seat in candidates {
                self.probs[seat.index()][card_id] = prob;
            }
            self.renormalize_column(card_id);
        }
    }

    fn zero_column(&mut self, card_index: usize) {
        for seat in PlayerPosition::LOOP {
            self.probs[seat.index()][card_index] = 0.0;
        }
    }

    fn renormalize_column(&mut self, card_index: usize) {
        let mut total = 0.0;
        for seat in PlayerPosition::LOOP {
            total += self.probs[seat.index()][card_index];
        }
        if total == 0.0 {
            return;
        }
        for seat in PlayerPosition::LOOP {
            self.probs[seat.index()][card_index] /= total;
        }
    }

    fn apply_void(&mut self, seat: PlayerPosition, suit: Suit) {
        let mask = &mut self.voids[seat.index()];
        if mask.contains(suit) {
            return;
        }
        *mask = mask.with(suit);
        for rank_index in 0..13 {
            let card_index = suit as usize * 13 + rank_index;
            self.probs[seat.index()][card_index] = 0.0;
            self.renormalize_column(card_index);
        }
    }

    pub(crate) fn scale_card_probability(&mut self, seat: PlayerPosition, card: Card, weight: f32) {
        if seat == self.perspective || weight <= 0.0 {
            return;
        }
        let card_index = card.to_id() as usize;
        let current = self.probs[seat.index()][card_index];
        if current == 0.0 {
            return;
        }
        self.probs[seat.index()][card_index] = (current * weight).max(0.0);
        self.renormalize_column(card_index);
    }

    pub(crate) fn scale_suit_for_seat(&mut self, seat: PlayerPosition, suit: Suit, weight: f32) {
        if weight <= 0.0 {
            return;
        }
        for rank in Rank::ORDERED {
            let card = Card::new(rank, suit);
            self.scale_card_probability(seat, card, weight);
        }
    }
}

fn collect_played(round: &RoundState) -> [bool; 52] {
    let mut seen = [false; 52];
    for trick in round.trick_history() {
        for play in trick.plays() {
            seen[play.card.to_id() as usize] = true;
        }
    }
    for play in round.current_trick().plays() {
        seen[play.card.to_id() as usize] = true;
    }
    seen
}

fn compute_hand_counts(round: &RoundState) -> [u8; 4] {
    array::from_fn(|index| {
        let seat = PlayerPosition::from_index(index).expect("valid seat index");
        round.hand(seat).len() as u8
    })
}

fn compute_remaining_suits(round: &RoundState) -> SuitCounts {
    let mut counts = SuitCounts::new_full_deck();
    for trick in round.trick_history() {
        for play in trick.plays() {
            counts.decrement(play.card.suit);
        }
    }
    for play in round.current_trick().plays() {
        counts.decrement(play.card.suit);
    }
    counts
}

fn compute_voids(round: &RoundState, perspective: PlayerPosition) -> [SuitMask; 4] {
    let mut voids = [SuitMask::EMPTY; 4];
    let my_hand = round.hand(perspective);
    for suit in Suit::ALL {
        if !hand_contains_suit(my_hand, suit) {
            voids[perspective.index()] = voids[perspective.index()].with(suit);
        }
    }

    for trick in round.trick_history() {
        mark_voids_from_trick(&mut voids, trick);
    }
    mark_voids_from_trick(&mut voids, round.current_trick());

    voids
}

fn hand_contains_suit(hand: &Hand, suit: Suit) -> bool {
    hand.iter().any(|card| card.suit == suit)
}

fn mark_voids_from_trick(voids: &mut [SuitMask; 4], trick: &crate::model::trick::Trick) {
    if let Some(lead) = trick.lead_suit() {
        for play in trick.plays() {
            if play.card.suit != lead {
                let seat = play.position.index();
                voids[seat] = voids[seat].with(lead);
            }
        }
    }
}

/// Metadata required when applying incremental updates.
#[derive(Debug, Clone, Copy)]
pub struct BeliefUpdateCtx {
    trick_index: u8,
    lead_suit: Option<Suit>,
}

impl BeliefUpdateCtx {
    /// Creates an update context for a new trick.
    pub fn new(trick_index: u8, lead_suit: Option<Suit>) -> Self {
        Self {
            trick_index,
            lead_suit,
        }
    }

    pub fn trick_index(&self) -> u8 {
        self.trick_index
    }

    pub fn lead_suit(&self) -> Option<Suit> {
        self.lead_suit
    }
}

/// Bit-mask describing which suits are void for a seat.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct SuitMask(u8);

impl SuitMask {
    pub const EMPTY: Self = Self(0);

    pub fn contains(self, suit: Suit) -> bool {
        let bit = 1 << suit as u8;
        self.0 & bit != 0
    }

    pub fn with(mut self, suit: Suit) -> Self {
        let bit = 1 << suit as u8;
        self.0 |= bit;
        self
    }
}

/// Counts of remaining cards per suit across all players.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct SuitCounts {
    pub clubs: u8,
    pub diamonds: u8,
    pub spades: u8,
    pub hearts: u8,
}

impl SuitCounts {
    pub const fn new(clubs: u8, diamonds: u8, spades: u8, hearts: u8) -> Self {
        Self {
            clubs,
            diamonds,
            spades,
            hearts,
        }
    }

    pub const fn new_full_deck() -> Self {
        Self::new(13, 13, 13, 13)
    }

    pub fn total(&self) -> u16 {
        self.clubs as u16 + self.diamonds as u16 + self.spades as u16 + self.hearts as u16
    }

    pub fn decrement(&mut self, suit: Suit) {
        match suit {
            Suit::Clubs => {
                if self.clubs > 0 {
                    self.clubs -= 1;
                }
            }
            Suit::Diamonds => {
                if self.diamonds > 0 {
                    self.diamonds -= 1;
                }
            }
            Suit::Spades => {
                if self.spades > 0 {
                    self.spades -= 1;
                }
            }
            Suit::Hearts => {
                if self.hearts > 0 {
                    self.hearts -= 1;
                }
            }
        }
    }
}

fn array_init_uniform() -> [[f32; 52]; 4] {
    let mut rows = [[0.0; 52]; 4];
    for card_index in 0..52 {
        for seat in PlayerPosition::LOOP {
            rows[seat.index()][card_index] = 0.25;
        }
    }
    rows
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::card::Card;
    use crate::model::deck::Deck;
    use crate::model::hand::Hand;
    use crate::model::passing::PassingDirection;
    use crate::model::player::PlayerPosition;
    use crate::model::rank::Rank;
    use crate::model::round::{RoundPhase, RoundState};
    use crate::model::suit::Suit;
    use crate::model::trick::Trick;
    use rand::rngs::SmallRng;
    use rand::{RngCore, SeedableRng};

    #[test]
    fn uniform_prior_columns_sum_to_one() {
        let belief = Belief::new_uninitialized(PlayerPosition::South);
        for card_id in 0..52 {
            let card = Card::from_id(card_id as u8).expect("valid card");
            let sum: f32 = PlayerPosition::LOOP
                .iter()
                .map(|seat| belief.prob_card(*seat, card))
                .sum();
            assert!((sum - 1.0).abs() < 1e-6);
        }
    }

    #[test]
    fn from_state_sets_known_hand_cards() {
        let deck = Deck::standard();
        let mut cards_iter = deck.cards().iter().copied();
        let mut hands = array::from_fn(|_| Hand::new());
        for seat in PlayerPosition::LOOP {
            let hand = &mut hands[seat.index()];
            for _ in 0..13 {
                hand.add(cards_iter.next().expect("enough cards"));
            }
        }

        let round = RoundState::from_hands(
            hands,
            PlayerPosition::South,
            PassingDirection::Hold,
            RoundPhase::Playing,
        );
        let belief = Belief::from_state(&round, PlayerPosition::South);
        let my_hand = round.hand(PlayerPosition::South);
        for &card in my_hand.cards() {
            assert_eq!(belief.prob_card(PlayerPosition::South, card), 1.0);
            for seat in PlayerPosition::LOOP {
                if seat != PlayerPosition::South {
                    assert_eq!(belief.prob_card(seat, card), 0.0);
                }
            }
        }
    }

    #[test]
    fn history_marks_voids_for_offsuit_play() {
        let mut hands = array::from_fn(|_| Hand::new());
        hands[PlayerPosition::South.index()].add(Card::new(Rank::Five, Suit::Clubs));
        hands[PlayerPosition::West.index()].add(Card::new(Rank::Six, Suit::Clubs));
        hands[PlayerPosition::North.index()].add(Card::new(Rank::Seven, Suit::Clubs));
        hands[PlayerPosition::East.index()].add(Card::new(Rank::Eight, Suit::Diamonds));

        let mut trick = Trick::new(PlayerPosition::South);
        trick
            .play(PlayerPosition::South, Card::new(Rank::Two, Suit::Clubs))
            .unwrap();
        trick
            .play(PlayerPosition::West, Card::new(Rank::Three, Suit::Clubs))
            .unwrap();
        trick
            .play(PlayerPosition::North, Card::new(Rank::Four, Suit::Clubs))
            .unwrap();
        trick
            .play(PlayerPosition::East, Card::new(Rank::Five, Suit::Diamonds))
            .unwrap();

        let round = RoundState::from_hands_with_state(
            hands,
            PlayerPosition::South,
            PassingDirection::Hold,
            RoundPhase::Playing,
            Trick::new(PlayerPosition::South),
            vec![trick],
            false,
        );

        let belief = Belief::from_state(&round, PlayerPosition::South);
        let east_mask = belief.void_mask(PlayerPosition::East);
        assert!(east_mask.contains(Suit::Clubs));
        for rank in Rank::ORDERED {
            let card = Card::new(rank, Suit::Clubs);
            if belief.prob_card(PlayerPosition::South, card) == 1.0 {
                continue;
            }
            assert_eq!(belief.prob_card(PlayerPosition::East, card), 0.0);
        }
    }

    #[test]
    fn hard_update_marks_void_when_offsuit_played() {
        let round = make_simple_round();
        let mut belief = Belief::from_state(&round, PlayerPosition::South);

        let ctx = BeliefUpdateCtx::new(0, Some(Suit::Clubs));
        let played_card = Card::new(Rank::Nine, Suit::Diamonds);
        belief.on_card_played(PlayerPosition::East, played_card, &ctx);

        assert!(belief.void_mask(PlayerPosition::East).contains(Suit::Clubs));
        for rank in Rank::ORDERED {
            let column_card = Card::new(rank, Suit::Clubs);
            assert_eq!(belief.prob_card(PlayerPosition::East, column_card), 0.0);
        }
    }

    #[test]
    fn from_state_columns_sum_to_one_randomised() {
        let mut rng = SmallRng::seed_from_u64(42);
        for _ in 0..32 {
            let seed = rng.next_u64();
            let deck = Deck::shuffled_with_seed(seed);
            let round = RoundState::deal(&deck, PlayerPosition::North, PassingDirection::Hold);
            for seat in PlayerPosition::LOOP {
                let belief = Belief::from_state(&round, seat);
                for card_id in 0..52 {
                    let card = Card::from_id(card_id as u8).unwrap();
                    let sum: f32 = PlayerPosition::LOOP
                        .iter()
                        .map(|other| belief.prob_card(*other, card))
                        .sum();
                    if sum != 0.0 {
                        assert!((sum - 1.0).abs() < 1e-6);
                    }
                }
            }
        }
    }

    #[test]
    fn summary_hash_changes_after_play() {
        let deck = Deck::shuffled_with_seed(55);
        let mut round = RoundState::deal(&deck, PlayerPosition::North, PassingDirection::Hold);
        let seat = round.current_trick().leader();
        let hash_before = Belief::from_state(&round, seat).summary_hash();

        let lead_card = Card::new(Rank::Two, Suit::Clubs);
        assert!(
            round.hand(seat).contains(lead_card),
            "leader should hold the Two of Clubs"
        );
        round.play_card(seat, lead_card).expect("play succeeds");
        let hash_after = Belief::from_state(&round, seat).summary_hash();
        assert_ne!(hash_before, hash_after);
    }

    fn make_simple_round() -> RoundState {
        let mut hands = array::from_fn(|_| Hand::new());
        hands[PlayerPosition::South.index()].add(Card::new(Rank::Two, Suit::Clubs));
        hands[PlayerPosition::East.index()].add(Card::new(Rank::Nine, Suit::Diamonds));
        hands[PlayerPosition::North.index()].add(Card::new(Rank::Queen, Suit::Hearts));
        hands[PlayerPosition::West.index()].add(Card::new(Rank::King, Suit::Spades));

        RoundState::from_hands(
            hands,
            PlayerPosition::South,
            PassingDirection::Hold,
            RoundPhase::Playing,
        )
    }
}
