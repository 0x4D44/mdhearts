use crate::model::rank::Rank;
use crate::model::suit::Suit;
use core::fmt;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Card {
    pub rank: Rank,
    pub suit: Suit,
}

impl Card {
    pub const fn new(rank: Rank, suit: Suit) -> Self {
        Self { rank, suit }
    }

    pub const fn is_penalty(self) -> bool {
        matches!(self.suit, Suit::Hearts) || self.is_queen_of_spades()
    }

    pub const fn is_queen_of_spades(self) -> bool {
        matches!(self.rank, Rank::Queen) && matches!(self.suit, Suit::Spades)
    }

    pub fn penalty_value(self) -> u8 {
        if self.is_queen_of_spades() {
            13
        } else if self.suit == Suit::Hearts {
            1
        } else {
            0
        }
    }

    /// Convert card to unique ID in range 0..52
    /// Encoding: suit * 13 + rank_index
    /// where rank_index is 0 for Two, 1 for Three, ..., 12 for Ace
    pub const fn to_id(self) -> u8 {
        let suit_id = self.suit as u8;
        let rank_index = self.rank.value() - 2; // Two=2 becomes 0, Ace=14 becomes 12
        suit_id * 13 + rank_index
    }

    /// Convert ID (0..52) back to Card
    /// Returns None if id >= 52
    pub const fn from_id(id: u8) -> Option<Self> {
        if id >= 52 {
            return None;
        }
        let suit_id = id / 13;
        let rank_index = id % 13;
        let rank_value = rank_index + 2; // 0 becomes Two=2, 12 becomes Ace=14

        let suit = match Suit::from_index(suit_id as usize) {
            Some(s) => s,
            None => return None,
        };

        let rank = match Rank::from_value(rank_value) {
            Some(r) => r,
            None => return None,
        };

        Some(Card::new(rank, suit))
    }
}

impl fmt::Display for Card {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}{}", self.rank, self.suit)
    }
}

#[cfg(test)]
mod tests {
    use super::{Card, Rank, Suit};

    #[test]
    fn queen_of_spades_identified() {
        let card = Card::new(Rank::Queen, Suit::Spades);
        assert!(card.is_queen_of_spades());
        assert!(card.is_penalty());
        assert_eq!(card.penalty_value(), 13);
    }

    #[test]
    fn regular_card_not_penalty() {
        let card = Card::new(Rank::Ten, Suit::Clubs);
        assert!(!card.is_penalty());
        assert_eq!(card.penalty_value(), 0);
    }

    #[test]
    fn hearts_are_one_point() {
        let card = Card::new(Rank::Ace, Suit::Hearts);
        assert!(card.is_penalty());
        assert_eq!(card.penalty_value(), 1);
    }

    #[test]
    fn card_id_roundtrip() {
        for id in 0..52 {
            let card = Card::from_id(id).expect("Valid ID");
            assert_eq!(card.to_id(), id);
        }
    }

    #[test]
    fn card_id_known_values() {
        // 2C should be 0
        let two_clubs = Card::new(Rank::Two, Suit::Clubs);
        assert_eq!(two_clubs.to_id(), 0);

        // AC should be 12
        let ace_clubs = Card::new(Rank::Ace, Suit::Clubs);
        assert_eq!(ace_clubs.to_id(), 12);

        // 2D should be 13
        let two_diamonds = Card::new(Rank::Two, Suit::Diamonds);
        assert_eq!(two_diamonds.to_id(), 13);

        // QS should be 2*13 + 10 = 36
        let queen_spades = Card::new(Rank::Queen, Suit::Spades);
        assert_eq!(queen_spades.to_id(), 36);

        // AH should be 51
        let ace_hearts = Card::new(Rank::Ace, Suit::Hearts);
        assert_eq!(ace_hearts.to_id(), 51);
    }

    #[test]
    fn card_from_id_invalid() {
        assert_eq!(Card::from_id(52), None);
        assert_eq!(Card::from_id(100), None);
    }

    #[test]
    fn all_52_cards_unique() {
        let mut seen = std::collections::HashSet::new();
        for id in 0..52 {
            let card = Card::from_id(id).expect("Valid ID");
            assert!(seen.insert(card), "Card ID {} produced duplicate", id);
        }
        assert_eq!(seen.len(), 52);
    }
}
