use hearts_core::model::card::Card;
use hearts_core::model::player::PlayerPosition;
use hearts_core::model::rank::Rank;
use hearts_core::model::round::RoundState;
use hearts_core::model::suit::Suit;
use std::collections::HashSet;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MoonState {
    Inactive,
    Considering,
    Committed,
}

#[derive(Debug, Clone)]
pub struct UnseenTracker {
    unseen: HashSet<Card>,
    // Known suit voids per seat (seat_idx x suit_idx)
    voids: [[bool; 4]; 4],
    // Moon attempt state per seat for Stage 2 heuristics
    moon: [MoonState; 4],
}

impl UnseenTracker {
    pub fn new() -> Self {
        Self {
            unseen: full_deck_cards().collect(),
            voids: [[false; 4]; 4],
            moon: [MoonState::Inactive; 4],
        }
    }

    pub fn reset_for_round(&mut self, round: &RoundState) {
        self.unseen = full_deck_cards().collect();
        self.voids = [[false; 4]; 4];
        self.moon = [MoonState::Inactive; 4];
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
    }

    pub fn is_void(&self, seat: PlayerPosition, suit: Suit) -> bool {
        self.voids[seat.index()][Self::suit_index(suit)]
    }

    pub fn moon_state(&self, seat: PlayerPosition) -> MoonState {
        self.moon[seat.index()]
    }

    pub fn set_moon_state(&mut self, seat: PlayerPosition, state: MoonState) {
        self.moon[seat.index()] = state;
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
    use hearts_core::model::player::PlayerPosition;
    use hearts_core::model::rank::Rank;
    use hearts_core::model::suit::Suit;

    #[test]
    fn tracker_initialises_with_full_deck() {
        let tracker = UnseenTracker::new();
        assert_eq!(tracker.unseen_count(), 52);
        assert!(tracker.is_unseen(Card::new(Rank::Ace, Suit::Spades)));
    }

    #[test]
    fn tracker_marks_cards_seen() {
        let mut tracker = UnseenTracker::new();
        let queen = Card::new(Rank::Queen, Suit::Spades);
        tracker.note_card_played(PlayerPosition::East, queen);
        assert!(!tracker.is_unseen(queen));
        assert_eq!(tracker.unseen_count(), 51);
    }

    #[test]
    fn tracker_voids_and_moon_state() {
        let mut tracker = UnseenTracker::new();
        assert!(!tracker.is_void(PlayerPosition::South, Suit::Hearts));
        tracker.note_void(PlayerPosition::South, Suit::Hearts);
        assert!(tracker.is_void(PlayerPosition::South, Suit::Hearts));

        use super::MoonState;
        assert_eq!(tracker.moon_state(PlayerPosition::East), MoonState::Inactive);
        tracker.set_moon_state(PlayerPosition::East, MoonState::Committed);
        assert_eq!(tracker.moon_state(PlayerPosition::East), MoonState::Committed);
    }
}
