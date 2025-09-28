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
}
