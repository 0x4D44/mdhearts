use crate::model::card::Card;
use std::vec::Vec;

#[derive(Debug, Clone, Default)]
pub struct Hand {
    cards: Vec<Card>,
}

impl Hand {
    pub fn new() -> Self {
        Self { cards: Vec::new() }
    }

    pub fn with_cards(cards: Vec<Card>) -> Self {
        let mut hand = Self { cards };
        hand.sort();
        hand
    }

    pub fn add(&mut self, card: Card) {
        self.cards.push(card);
        self.sort();
    }

    pub fn remove(&mut self, card: Card) -> bool {
        if let Some(index) = self.cards.iter().position(|&c| c == card) {
            self.cards.remove(index);
            true
        } else {
            false
        }
    }

    pub fn contains(&self, card: Card) -> bool {
        self.cards.contains(&card)
    }

    pub fn len(&self) -> usize {
        self.cards.len()
    }

    pub fn is_empty(&self) -> bool {
        self.cards.is_empty()
    }

    pub fn iter(&self) -> impl Iterator<Item = &Card> {
        self.cards.iter()
    }

    pub fn cards(&self) -> &[Card] {
        &self.cards
    }

    fn sort(&mut self) {
        self.cards
            .sort_by(|a, b| a.suit.cmp(&b.suit).then(a.rank.cmp(&b.rank)));
    }
}

#[cfg(test)]
mod tests {
    use super::Hand;
    use crate::model::card::Card;
    use crate::model::rank::Rank;
    use crate::model::suit::Suit;

    #[test]
    fn add_and_remove_cards() {
        let mut hand = Hand::new();
        let card = Card::new(Rank::Three, Suit::Clubs);
        hand.add(card);
        assert!(hand.contains(card));
        assert!(hand.remove(card));
        assert!(!hand.contains(card));
    }

    #[test]
    fn cards_are_sorted_by_suit_then_rank() {
        let mut hand = Hand::new();
        hand.add(Card::new(Rank::King, Suit::Spades));
        hand.add(Card::new(Rank::Two, Suit::Clubs));
        hand.add(Card::new(Rank::Ace, Suit::Clubs));
        let ordered: Vec<_> = hand.iter().copied().collect();
        assert_eq!(ordered[0], Card::new(Rank::Two, Suit::Clubs));
        assert_eq!(ordered[1], Card::new(Rank::Ace, Suit::Clubs));
        assert_eq!(ordered[2], Card::new(Rank::King, Suit::Spades));
    }
}
