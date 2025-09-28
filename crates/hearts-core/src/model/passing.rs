use crate::model::card::Card;
use crate::model::hand::Hand;
use crate::model::player::PlayerPosition;
use std::array;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PassingDirection {
    Left,
    Right,
    Across,
    Hold,
}

impl PassingDirection {
    pub const CYCLE: [PassingDirection; 4] = [
        PassingDirection::Left,
        PassingDirection::Right,
        PassingDirection::Across,
        PassingDirection::Hold,
    ];

    pub const fn next(self) -> PassingDirection {
        match self {
            PassingDirection::Left => PassingDirection::Right,
            PassingDirection::Right => PassingDirection::Across,
            PassingDirection::Across => PassingDirection::Hold,
            PassingDirection::Hold => PassingDirection::Left,
        }
    }

    pub const fn requires_selection(self) -> bool {
        !matches!(self, PassingDirection::Hold)
    }

    pub fn from_str(value: &str) -> Option<Self> {
        match value.to_ascii_lowercase().as_str() {
            "left" => Some(PassingDirection::Left),
            "right" => Some(PassingDirection::Right),
            "across" => Some(PassingDirection::Across),
            "hold" => Some(PassingDirection::Hold),
            _ => None,
        }
    }

    pub const fn target(self, seat: PlayerPosition) -> PlayerPosition {
        match self {
            PassingDirection::Left => seat.next(),
            PassingDirection::Right => seat.previous(),
            PassingDirection::Across => seat.opposite(),
            PassingDirection::Hold => seat,
        }
    }

    pub const fn as_str(self) -> &'static str {
        match self {
            PassingDirection::Left => "Left",
            PassingDirection::Right => "Right",
            PassingDirection::Across => "Across",
            PassingDirection::Hold => "Hold",
        }
    }
}

#[derive(Debug, Clone)]
pub struct PassingState {
    direction: PassingDirection,
    submissions: [Option<[Card; 3]>; 4],
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PassingError {
    NotInPassingPhase,
    DirectionDoesNotPass,
    AlreadySubmitted(PlayerPosition),
    CardNotInHand(Card),
    Incomplete,
}

impl PassingState {
    pub fn new(direction: PassingDirection) -> Self {
        Self {
            direction,
            submissions: array::from_fn(|_| None),
        }
    }

    pub fn direction(&self) -> PassingDirection {
        self.direction
    }

    pub fn submit(
        &mut self,
        seat: PlayerPosition,
        cards: [Card; 3],
        hand: &mut Hand,
    ) -> Result<(), PassingError> {
        if !self.direction.requires_selection() {
            return Err(PassingError::DirectionDoesNotPass);
        }

        if self.submissions[seat.index()].is_some() {
            return Err(PassingError::AlreadySubmitted(seat));
        }

        for card in cards.iter() {
            if !hand.contains(*card) {
                return Err(PassingError::CardNotInHand(*card));
            }
        }

        for card in cards.iter() {
            if !hand.remove(*card) {
                return Err(PassingError::CardNotInHand(*card));
            }
        }

        self.submissions[seat.index()] = Some(cards);
        Ok(())
    }

    pub fn is_complete(&self) -> bool {
        self.submissions
            .iter()
            .all(|submission| submission.is_some())
    }

    pub fn apply(self, hands: &mut [Hand; 4]) -> Result<(), PassingError> {
        if !self.direction.requires_selection() {
            return Err(PassingError::DirectionDoesNotPass);
        }

        if !self.is_complete() {
            return Err(PassingError::Incomplete);
        }

        for seat in PlayerPosition::LOOP.iter().copied() {
            if let Some(cards) = self.submissions[seat.index()] {
                let target = self.direction.target(seat);
                let hand = &mut hands[target.index()];
                for card in cards.iter() {
                    hand.add(*card);
                }
            }
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::{PassingDirection, PassingError, PassingState};
    use crate::model::card::Card;
    use crate::model::hand::Hand;
    use crate::model::player::PlayerPosition;
    use crate::model::rank::Rank;
    use crate::model::suit::Suit;
    use std::array;

    #[test]
    fn direction_cycle_wraps() {
        assert_eq!(PassingDirection::Hold.next(), PassingDirection::Left);
    }

    #[test]
    fn target_mapping_works() {
        assert_eq!(
            PassingDirection::Left.target(PlayerPosition::North),
            PlayerPosition::East
        );
        assert_eq!(
            PassingDirection::Right.target(PlayerPosition::North),
            PlayerPosition::West
        );
        assert_eq!(
            PassingDirection::Across.target(PlayerPosition::North),
            PlayerPosition::South
        );
    }

    #[test]
    fn submit_removes_cards_and_apply_distributes() {
        let mut state = PassingState::new(PassingDirection::Left);
        let mut hands = array::from_fn(|_| Hand::new());

        for (i, seat) in PlayerPosition::LOOP.iter().copied().enumerate() {
            let suit = match seat {
                PlayerPosition::North => Suit::Clubs,
                PlayerPosition::East => Suit::Diamonds,
                PlayerPosition::South => Suit::Spades,
                PlayerPosition::West => Suit::Hearts,
            };
            for rank in Rank::ORDERED.iter().copied().take(13) {
                hands[i].add(Card::new(rank, suit));
            }
        }

        let north_submission = [
            Card::new(Rank::Two, Suit::Clubs),
            Card::new(Rank::Three, Suit::Clubs),
            Card::new(Rank::Four, Suit::Clubs),
        ];

        state
            .submit(
                PlayerPosition::North,
                north_submission,
                &mut hands[PlayerPosition::North.index()],
            )
            .unwrap();
        assert_eq!(hands[PlayerPosition::North.index()].len(), 10);

        for seat in PlayerPosition::LOOP.iter().copied().skip(1) {
            let cards = [
                hands[seat.index()].cards()[0],
                hands[seat.index()].cards()[1],
                hands[seat.index()].cards()[2],
            ];
            state.submit(seat, cards, &mut hands[seat.index()]).unwrap();
        }

        state.apply(&mut hands).unwrap();
        for seat in PlayerPosition::LOOP.iter().copied() {
            assert_eq!(hands[seat.index()].len(), 13);
        }
    }

    #[test]
    fn cannot_submit_missing_card() {
        let mut state = PassingState::new(PassingDirection::Left);
        let mut hand = Hand::new();
        match state.submit(
            PlayerPosition::North,
            [
                Card::new(Rank::Two, Suit::Clubs),
                Card::new(Rank::Three, Suit::Clubs),
                Card::new(Rank::Four, Suit::Clubs),
            ],
            &mut hand,
        ) {
            Err(PassingError::CardNotInHand(_)) => {}
            other => panic!("expected missing card error, got {other:?}"),
        }
    }


    #[test]
    fn cannot_submit_duplicate_cards() {
        let mut state = PassingState::new(PassingDirection::Left);
        let mut hand = Hand::new();
        hand.add(Card::new(Rank::Two, Suit::Clubs));
        hand.add(Card::new(Rank::Three, Suit::Clubs));
        hand.add(Card::new(Rank::Four, Suit::Clubs));

        let duplicate = [
            Card::new(Rank::Two, Suit::Clubs),
            Card::new(Rank::Two, Suit::Clubs),
            Card::new(Rank::Two, Suit::Clubs),
        ];

        assert!(matches!(
            state.submit(
                PlayerPosition::North,
                duplicate,
                &mut hand,
            ),
            Err(PassingError::CardNotInHand(_))
        ));
    }

    #[test]
    fn as_str_returns_human_readable_values() {
        assert_eq!(PassingDirection::Left.as_str(), "Left");
        assert_eq!(PassingDirection::Hold.as_str(), "Hold");
    }

    #[test]
    fn from_str_parses_case_insensitive_values() {
        assert_eq!(
            PassingDirection::from_str("LEFT"),
            Some(PassingDirection::Left)
        );
        assert_eq!(
            PassingDirection::from_str("across"),
            Some(PassingDirection::Across)
        );
        assert_eq!(PassingDirection::from_str("unknown"), None);
    }
}



