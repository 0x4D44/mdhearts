use crate::model::card::Card;
use crate::model::rank::Rank;
use crate::model::suit::Suit;
use rand::SeedableRng;
use rand::rngs::StdRng;
use rand::seq::SliceRandom;

#[derive(Debug, Clone)]
pub struct Deck {
    cards: Vec<Card>,
}

impl Deck {
    pub fn standard() -> Self {
        let mut cards = Vec::with_capacity(52);
        for suit in Suit::ALL.iter().copied() {
            for rank in Rank::ORDERED.iter().copied() {
                cards.push(Card::new(rank, suit));
            }
        }
        Self { cards }
    }

    pub fn shuffled<R: rand::Rng + ?Sized>(rng: &mut R) -> Self {
        let mut deck = Self::standard();
        deck.shuffle_in_place(rng);
        deck
    }

    pub fn shuffled_with_seed(seed: u64) -> Self {
        let mut rng = StdRng::seed_from_u64(seed);
        Self::shuffled(&mut rng)
    }

    pub fn shuffle_in_place<R: rand::Rng + ?Sized>(&mut self, rng: &mut R) {
        self.cards.shuffle(rng);
    }

    pub fn cards(&self) -> &[Card] {
        &self.cards
    }
}

#[cfg(test)]
mod tests {
    use super::Deck;

    #[test]
    fn standard_deck_has_52_unique_cards() {
        let deck = Deck::standard();
        assert_eq!(deck.cards().len(), 52);
    }

    #[test]
    fn shuffle_with_seed_is_deterministic() {
        let deck_a = Deck::shuffled_with_seed(42);
        let deck_b = Deck::shuffled_with_seed(42);
        assert_eq!(deck_a.cards(), deck_b.cards());
    }

    #[test]
    fn shuffle_with_different_seeds_differs() {
        let deck_a = Deck::shuffled_with_seed(1);
        let deck_b = Deck::shuffled_with_seed(2);
        assert_ne!(deck_a.cards(), deck_b.cards());
    }
}
