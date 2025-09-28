use crate::model::card::Card;
use crate::model::player::PlayerPosition;
use std::fmt;

#[derive(Debug, Clone)]
pub struct Trick {
    leader: PlayerPosition,
    plays: Vec<Play>,
}

#[derive(Debug, Clone, Copy)]
pub struct Play {
    pub position: PlayerPosition,
    pub card: Card,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TrickError {
    TrickComplete,
    OutOfTurn {
        expected: PlayerPosition,
        actual: PlayerPosition,
    },
    AlreadyPlayed(PlayerPosition),
}

impl fmt::Display for TrickError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            TrickError::TrickComplete => write!(f, "trick already complete"),
            TrickError::OutOfTurn { expected, actual } => {
                write!(f, "expected {expected} to play next but got {actual}")
            }
            TrickError::AlreadyPlayed(position) => {
                write!(f, "{position} has already played this trick")
            }
        }
    }
}

impl std::error::Error for TrickError {}

impl Trick {
    pub fn new(leader: PlayerPosition) -> Self {
        Self {
            leader,
            plays: Vec::with_capacity(4),
        }
    }

    pub fn leader(&self) -> PlayerPosition {
        self.leader
    }

    pub fn plays(&self) -> &[Play] {
        &self.plays
    }

    pub fn is_complete(&self) -> bool {
        self.plays.len() == 4
    }

    pub fn lead_suit(&self) -> Option<crate::model::suit::Suit> {
        self.plays.first().map(|play| play.card.suit)
    }

    pub fn play(&mut self, position: PlayerPosition, card: Card) -> Result<(), TrickError> {
        if self.is_complete() {
            return Err(TrickError::TrickComplete);
        }

        if self.plays.iter().any(|play| play.position == position) {
            return Err(TrickError::AlreadyPlayed(position));
        }

        let expected = self.expected_position();
        if expected != position {
            return Err(TrickError::OutOfTurn {
                expected,
                actual: position,
            });
        }

        self.plays.push(Play { position, card });
        Ok(())
    }

    pub fn winner(&self) -> Option<PlayerPosition> {
        if !self.is_complete() {
            return None;
        }
        let lead_suit = self.lead_suit()?;
        self.plays
            .iter()
            .filter(|play| play.card.suit == lead_suit)
            .max_by(|a, b| a.card.rank.cmp(&b.card.rank))
            .map(|play| play.position)
    }

    pub fn penalty_total(&self) -> u8 {
        self.plays
            .iter()
            .map(|play| play.card.penalty_value())
            .sum()
    }

    fn expected_position(&self) -> PlayerPosition {
        self.plays
            .last()
            .map(|play| play.position.next())
            .unwrap_or(self.leader)
    }
}

#[cfg(test)]
mod tests {
    use super::{Trick, TrickError};
    use crate::model::card::Card;
    use crate::model::player::PlayerPosition;
    use crate::model::rank::Rank;
    use crate::model::suit::Suit;

    #[test]
    fn plays_follow_turn_order() {
        let mut trick = Trick::new(PlayerPosition::North);
        assert!(
            trick
                .play(PlayerPosition::North, Card::new(Rank::Two, Suit::Clubs))
                .is_ok()
        );
        assert!(matches!(
            trick.play(PlayerPosition::South, Card::new(Rank::Three, Suit::Clubs)),
            Err(TrickError::OutOfTurn { .. })
        ));
    }

    #[test]
    fn winner_is_highest_card_of_lead_suit() {
        let mut trick = Trick::new(PlayerPosition::North);
        trick
            .play(PlayerPosition::North, Card::new(Rank::Ten, Suit::Clubs))
            .unwrap();
        trick
            .play(PlayerPosition::East, Card::new(Rank::Queen, Suit::Clubs))
            .unwrap();
        trick
            .play(PlayerPosition::South, Card::new(Rank::Four, Suit::Clubs))
            .unwrap();
        trick
            .play(PlayerPosition::West, Card::new(Rank::Ace, Suit::Spades))
            .unwrap();

        assert_eq!(trick.winner(), Some(PlayerPosition::East));
        assert_eq!(trick.penalty_total(), 0);
    }

    #[test]
    fn queen_of_spades_counts_as_penalty() {
        let mut trick = Trick::new(PlayerPosition::North);
        trick
            .play(PlayerPosition::North, Card::new(Rank::Two, Suit::Clubs))
            .unwrap();
        trick
            .play(PlayerPosition::East, Card::new(Rank::Queen, Suit::Spades))
            .unwrap();
        trick
            .play(PlayerPosition::South, Card::new(Rank::Four, Suit::Clubs))
            .unwrap();
        trick
            .play(PlayerPosition::West, Card::new(Rank::Five, Suit::Clubs))
            .unwrap();

        assert_eq!(trick.penalty_total(), 13);
    }
}
