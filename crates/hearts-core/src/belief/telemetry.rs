use super::{Belief, SampledWorld};
use crate::model::player::PlayerPosition;
use crate::model::suit::Suit;

#[derive(Debug, Clone)]
pub struct BeliefMetrics {
    pub trick_index: u8,
    pub entropy_per_seat: [f32; 4],
    pub entropy_per_suit: [[f32; 4]; 4],
}

impl BeliefMetrics {
    pub fn from_belief(belief: &Belief) -> Self {
        let mut entropy_per_seat = [0.0; 4];
        let mut entropy_per_suit = [[0.0; 4]; 4];

        for seat in PlayerPosition::LOOP {
            let mut seat_entropy = 0.0;
            for suit in Suit::ALL {
                let mut suit_entropy = 0.0;
                for card_prob in belief.iter_suit_probs(seat, suit) {
                    if *card_prob > 0.0 {
                        suit_entropy -= card_prob * card_prob.ln();
                    }
                }
                entropy_per_suit[seat.index()][suit as usize] = suit_entropy;
                seat_entropy += suit_entropy;
            }
            entropy_per_seat[seat.index()] = seat_entropy;
        }

        Self {
            trick_index: belief.trick_index(),
            entropy_per_seat,
            entropy_per_suit,
        }
    }
}

#[derive(Debug, Clone)]
pub struct SampledWorldMetrics {
    pub log_weight: f32,
    pub rejection: bool,
}

impl SampledWorldMetrics {
    pub fn successful(world: &SampledWorld) -> Self {
        Self {
            log_weight: world.log_weight(),
            rejection: false,
        }
    }

    pub fn rejected() -> Self {
        Self {
            log_weight: f32::NEG_INFINITY,
            rejection: true,
        }
    }
}
