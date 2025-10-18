use hearts_core::belief::Belief;
use hearts_core::model::card::Card;
use hearts_core::model::player::PlayerPosition;
use hearts_core::model::rank::Rank;
use hearts_core::model::suit::Suit;

/// Lightweight view over a [`Belief`] used by heuristic planners.
#[derive(Debug, Clone, Copy)]
pub struct BeliefView<'a> {
    belief: &'a Belief,
    void_threshold: f32,
}

impl<'a> BeliefView<'a> {
    pub fn new(belief: &'a Belief, void_threshold: f32) -> Self {
        Self {
            belief,
            void_threshold,
        }
    }

    pub fn belief(&self) -> &'a Belief {
        self.belief
    }

    pub fn void_matrix(&self) -> [[bool; 4]; 4] {
        let mut matrix = [[false; 4]; 4];
        for seat in PlayerPosition::LOOP {
            for suit in Suit::ALL {
                let mass = self.suit_mass(seat, suit);
                if mass < self.void_threshold {
                    matrix[seat.index()][suit as usize] = true;
                }
            }
        }
        matrix
    }

    pub fn suit_mass(&self, seat: PlayerPosition, suit: Suit) -> f32 {
        Rank::ORDERED
            .iter()
            .copied()
            .map(|rank| {
                let card = Card::new(rank, suit);
                self.belief.prob_card(seat, card)
            })
            .sum()
    }

    pub fn void_threshold(&self) -> f32 {
        self.void_threshold
    }

    pub fn summary_hash(&self) -> u64 {
        self.belief.summary_hash()
    }
}
