use hearts_core::model::card::Card;
use hearts_core::model::player::PlayerPosition;
use hearts_core::model::rank::Rank;
use hearts_core::model::round::RoundState;
use hearts_core::model::suit::Suit;
use std::collections::HashSet;

#[derive(Debug, Clone)]
pub struct UnseenTracker {
    unseen: HashSet<Card>,
}

impl UnseenTracker {
    pub fn new() -> Self {
        Self {
            unseen: full_deck_cards().collect(),
        }
    }

    pub fn reset_for_round(&mut self, round: &RoundState) {
        self.unseen = full_deck_cards().collect();
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

    /// Infer which opponents are void in which suits based on trick history
    /// Returns a 4x4 array where voids[seat][suit] = true if seat is void in suit
    pub fn infer_voids(&self, _my_seat: PlayerPosition, round: &RoundState) -> [[bool; 4]; 4] {
        let mut voids = [[false; 4]; 4];

        // Look at all completed tricks
        for trick in round.trick_history() {
            if let Some(lead_suit) = trick.lead_suit() {
                // Check each player's response
                for play in trick.plays() {
                    // If they played a different suit, they're void in the led suit
                    if play.card.suit != lead_suit {
                        voids[play.position.index()][lead_suit as usize] = true;
                    }
                }
            }
        }

        // Also check current trick
        let current = round.current_trick();
        if let Some(lead_suit) = current.lead_suit() {
            for play in current.plays() {
                if play.card.suit != lead_suit {
                    voids[play.position.index()][lead_suit as usize] = true;
                }
            }
        }

        voids
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
    fn void_inference_detects_off_suit_discard() {
        use hearts_core::model::hand::Hand;
        use hearts_core::model::passing::PassingDirection;
        use hearts_core::model::round::{RoundPhase, RoundState};

        // Create a round with hands
        let mut hands = [Hand::new(), Hand::new(), Hand::new(), Hand::new()];
        hands[PlayerPosition::South.index()] = Hand::with_cards(vec![
            Card::new(Rank::Two, Suit::Clubs),
            Card::new(Rank::Three, Suit::Clubs),
        ]);
        hands[PlayerPosition::West.index()] = Hand::with_cards(vec![
            Card::new(Rank::Three, Suit::Diamonds),
            Card::new(Rank::Four, Suit::Diamonds),
        ]);
        hands[PlayerPosition::North.index()] =
            Hand::with_cards(vec![Card::new(Rank::Five, Suit::Clubs)]);
        hands[PlayerPosition::East.index()] =
            Hand::with_cards(vec![Card::new(Rank::Six, Suit::Clubs)]);

        let mut round = RoundState::from_hands(
            hands,
            PlayerPosition::South,
            PassingDirection::Hold,
            RoundPhase::Playing,
        );

        // South leads 2C
        let _ = round.play_card(PlayerPosition::South, Card::new(Rank::Two, Suit::Clubs));

        // West discards a Diamond (void in Clubs)
        let _ = round.play_card(PlayerPosition::West, Card::new(Rank::Three, Suit::Diamonds));

        let tracker = UnseenTracker::new();
        let voids = tracker.infer_voids(PlayerPosition::South, &round);

        // West should be marked as void in Clubs
        assert!(voids[PlayerPosition::West.index()][Suit::Clubs as usize]);
    }
}
