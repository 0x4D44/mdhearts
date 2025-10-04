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
}
