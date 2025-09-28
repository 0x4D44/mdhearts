use crate::model::card::Card;
use crate::model::deck::Deck;
use crate::model::hand::Hand;
use crate::model::passing::{PassingDirection, PassingError, PassingState};
use crate::model::player::PlayerPosition;
use crate::model::rank::Rank;
use crate::model::suit::Suit;
use crate::model::trick::Trick;
use std::{array, vec::Vec};

#[derive(Debug, Clone)]
pub struct RoundState {
    hands: [Hand; 4],
    current_trick: Trick,
    trick_history: Vec<Trick>,
    starting_player: PlayerPosition,
    passing_direction: PassingDirection,
    phase: RoundPhase,
    hearts_broken: bool,
}

#[derive(Debug, Clone)]
pub enum RoundPhase {
    Passing(PassingState),
    Playing,
}

impl RoundState {
    pub fn deal(
        deck: &Deck,
        starting_player: PlayerPosition,
        passing_direction: PassingDirection,
    ) -> Self {
        let mut hands = array::from_fn(|_| Hand::new());

        for (index, card) in deck.cards().iter().enumerate() {
            let seat = PlayerPosition::from_index(index % 4).expect("player index in range");
            hands[seat.index()].add(*card);
        }

        let two_of_clubs = Card::new(Rank::Two, Suit::Clubs);
        let starting_player = hands
            .iter()
            .enumerate()
            .find(|(_, hand)| hand.contains(two_of_clubs))
            .and_then(|(idx, _)| PlayerPosition::from_index(idx))
            .unwrap_or(starting_player);

        let phase = if passing_direction.requires_selection() {
            RoundPhase::Passing(PassingState::new(passing_direction))
        } else {
            RoundPhase::Playing
        };

        Self {
            hands,
            current_trick: Trick::new(starting_player),
            trick_history: Vec::new(),
            starting_player,
            passing_direction,
            phase,
            hearts_broken: false,
        }
    }

    pub fn passing_direction(&self) -> PassingDirection {
        self.passing_direction
    }

    pub fn phase(&self) -> &RoundPhase {
        &self.phase
    }

    pub fn hand(&self, seat: PlayerPosition) -> &Hand {
        &self.hands[seat.index()]
    }

    pub fn current_trick(&self) -> &Trick {
        &self.current_trick
    }

    pub fn current_trick_mut(&mut self) -> &mut Trick {
        &mut self.current_trick
    }

    pub fn trick_history(&self) -> &[Trick] {
        &self.trick_history
    }

    pub fn starting_player(&self) -> PlayerPosition {
        self.starting_player
    }

    pub fn complete_trick(&mut self, next_leader: PlayerPosition) {
        let finished = std::mem::replace(&mut self.current_trick, Trick::new(next_leader));
        self.trick_history.push(finished);
    }

    pub fn tricks_completed(&self) -> usize {
        self.trick_history.len()
    }

    pub fn submit_pass(
        &mut self,
        seat: PlayerPosition,
        cards: [Card; 3],
    ) -> Result<(), PassingError> {
        match &mut self.phase {
            RoundPhase::Passing(state) => {
                let hand = &mut self.hands[seat.index()];
                state.submit(seat, cards, hand)
            }
            RoundPhase::Playing => Err(PassingError::NotInPassingPhase),
        }
    }

    pub fn resolve_passes(&mut self) -> Result<(), PassingError> {
        let state = match &self.phase {
            RoundPhase::Passing(state) => state.clone(),
            RoundPhase::Playing => return Err(PassingError::NotInPassingPhase),
        };

        if !state.direction().requires_selection() {
            return Err(PassingError::DirectionDoesNotPass);
        }

        if !state.is_complete() {
            return Err(PassingError::Incomplete);
        }

        state.apply(&mut self.hands)?;
        // After passes are applied, the holder of the Two of Clubs may change.
        // Ensure the first trick leader follows the current 2C holder, as per Hearts rules.
        let two_of_clubs = Card::new(Rank::Two, Suit::Clubs);
        if let Some(new_leader) = PlayerPosition::LOOP
            .iter()
            .copied()
            .find(|seat| self.hands[seat.index()].contains(two_of_clubs))
        {
            self.starting_player = new_leader;
            self.current_trick = Trick::new(new_leader);
        }
        self.phase = RoundPhase::Playing;
        Ok(())
    }

    pub fn penalty_totals(&self) -> [u8; 4] {
        let mut totals = [0u8; 4];
        let mut accumulate = |trick: &Trick| {
            for play in trick.plays() {
                let idx = play.position.index();
                totals[idx] = totals[idx].saturating_add(play.card.penalty_value());
            }
        };

        for trick in &self.trick_history {
            accumulate(trick);
        }

        if self.current_trick.is_complete() {
            accumulate(&self.current_trick);
        }

        totals
    }

    pub fn hearts_broken(&self) -> bool {
        self.hearts_broken
    }

    pub fn is_first_trick(&self) -> bool {
        self.trick_history.is_empty()
    }

    pub fn legal_to_lead_hearts(&self, seat: PlayerPosition) -> bool {
        if self.hearts_broken {
            return true;
        }
        let hand = &self.hands[seat.index()];
        !hand.iter().any(|c| !c.suit.is_heart())
    }

    pub fn play_card(
        &mut self,
        seat: PlayerPosition,
        card: Card,
    ) -> Result<PlayOutcome, PlayError> {
        if !matches!(self.phase, RoundPhase::Playing) {
            return Err(PlayError::NotInPlayPhase);
        }

        if !self.hands[seat.index()].contains(card) {
            return Err(PlayError::CardNotInHand(card));
        }

        // Determine expected seat
        let expected = self
            .current_trick
            .plays()
            .last()
            .map(|p| p.position.next())
            .unwrap_or(self.current_trick.leader());
        if expected != seat {
            return Err(PlayError::OutOfTurn { expected, actual: seat });
        }

        let lead_suit = self.current_trick.lead_suit();
        let is_lead = lead_suit.is_none();

        // First trick constraints
        if self.is_first_trick() {
            if is_lead {
                let two_of_clubs = Card::new(Rank::Two, Suit::Clubs);
                if card != two_of_clubs {
                    return Err(PlayError::MustLeadTwoOfClubs);
                }
            } else if lead_suit == Some(Suit::Clubs) {
                let hand = &self.hands[seat.index()];
                let can_follow = hand.iter().any(|c| c.suit == Suit::Clubs);
                if !can_follow {
                    let is_qs = card.is_queen_of_spades();
                    if card.suit.is_heart() || is_qs {
                        let only_hearts = hand.iter().all(|c| c.suit.is_heart());
                        if !(only_hearts && card.suit.is_heart()) {
                            return Err(PlayError::NoPointsOnFirstTrick);
                        }
                    }
                }
            }
        }

        // Follow suit enforcement
        if let Some(suit) = lead_suit {
            if card.suit != suit {
                if self.hands[seat.index()].iter().any(|c| c.suit == suit) {
                    return Err(PlayError::MustFollowSuit(suit));
                }
            }
        } else {
            // Leader constraints: cannot lead hearts before broken unless only hearts
            if card.suit == Suit::Hearts && !self.legal_to_lead_hearts(seat) {
                return Err(PlayError::HeartsNotBroken);
            }
        }

        // Remove and record play
        let _ = self.hands[seat.index()].remove(card);
        if card.suit == Suit::Hearts {
            self.hearts_broken = true;
        }
        self.current_trick
            .play(seat, card)
            .map_err(PlayError::Trick)?;

        if self.current_trick.is_complete() {
            let winner = self.current_trick.winner().expect("winner when complete");
            let penalties = self.current_trick.penalty_total();
            self.complete_trick(winner);
            Ok(PlayOutcome::TrickCompleted { winner, penalties })
        } else {
            Ok(PlayOutcome::Played)
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PlayOutcome {
    Played,
    TrickCompleted { winner: PlayerPosition, penalties: u8 },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PlayError {
    NotInPlayPhase,
    CardNotInHand(Card),
    OutOfTurn { expected: PlayerPosition, actual: PlayerPosition },
    MustLeadTwoOfClubs,
    MustFollowSuit(Suit),
    HeartsNotBroken,
    NoPointsOnFirstTrick,
    Trick(super::trick::TrickError),
}

#[cfg(test)]
mod tests {
    use super::{PassingDirection, PlayError, PlayOutcome, RoundPhase, RoundState};
    use crate::model::card::Card;
    use crate::model::deck::Deck;
    use crate::model::player::PlayerPosition;
    use crate::model::rank::Rank;
    use crate::model::suit::Suit;

    #[test]
    fn dealing_distributes_thirteen_cards_per_player() {
        let deck = Deck::standard();
        let round = RoundState::deal(&deck, PlayerPosition::North, PassingDirection::Left);

        for seat in PlayerPosition::LOOP.iter().copied() {
            assert_eq!(round.hand(seat).len(), 13, "{seat} should have 13 cards");
        }
        assert!(matches!(round.phase(), RoundPhase::Passing(_)));
        assert_eq!(round.current_trick().leader(), PlayerPosition::North);
        assert_eq!(round.trick_history().len(), 0);
    }

    #[test]
    fn completed_tricks_move_to_history() {
        let deck = Deck::standard();
        let mut round = RoundState::deal(&deck, PlayerPosition::North, PassingDirection::Hold);
        round.complete_trick(PlayerPosition::East);
        assert_eq!(round.tricks_completed(), 1);
        assert_eq!(round.current_trick().leader(), PlayerPosition::East);
    }

    #[test]
    fn passing_flow_moves_cards_and_enters_play_phase() {
        let deck = Deck::standard();
        let mut round = RoundState::deal(&deck, PlayerPosition::North, PassingDirection::Left);

        for seat in PlayerPosition::LOOP.iter().copied() {
            let hand = round.hand(seat);
            let cards = [hand.cards()[0], hand.cards()[1], hand.cards()[2]];
            round.submit_pass(seat, cards).unwrap();
        }

        round.resolve_passes().unwrap();
        assert!(matches!(round.phase(), RoundPhase::Playing));
        for seat in PlayerPosition::LOOP.iter().copied() {
            assert_eq!(round.hand(seat).len(), 13);
        }
    }

    #[test]
    fn hold_direction_starts_in_play_phase() {
        let deck = Deck::standard();
        let round = RoundState::deal(&deck, PlayerPosition::North, PassingDirection::Hold);
        assert!(matches!(round.phase(), RoundPhase::Playing));
    }

    #[test]
    fn submitting_pass_missing_card_errors() {
        let deck = Deck::standard();
        let mut round = RoundState::deal(&deck, PlayerPosition::North, PassingDirection::Left);

        let invalid_card = round.hand(PlayerPosition::East).cards()[0];
        assert!(
            round
                .submit_pass(
                    PlayerPosition::North,
                    [invalid_card, invalid_card, invalid_card],
                )
                .is_err()
        );
    }

    #[test]
    fn penalty_totals_accumulate_completed_tricks() {
        let deck = Deck::standard();
        let mut round = RoundState::deal(&deck, PlayerPosition::North, PassingDirection::Hold);

        let plays = [
            (PlayerPosition::North, Card::new(Rank::Queen, Suit::Spades)),
            (PlayerPosition::East, Card::new(Rank::Two, Suit::Hearts)),
            (PlayerPosition::South, Card::new(Rank::Three, Suit::Hearts)),
            (PlayerPosition::West, Card::new(Rank::Four, Suit::Clubs)),
        ];

        for (seat, card) in plays {
            round.current_trick_mut().play(seat, card).unwrap();
        }
        round.complete_trick(PlayerPosition::North);

        let totals = round.penalty_totals();
        assert_eq!(totals[PlayerPosition::North.index()], 13);
        assert_eq!(totals[PlayerPosition::East.index()], 1);
        assert_eq!(totals[PlayerPosition::South.index()], 1);
        assert_eq!(totals[PlayerPosition::West.index()], 0);
    }

    #[test]
    fn leader_follows_two_of_clubs_holder() {
        let deck = Deck::shuffled_with_seed(99);
        let round = RoundState::deal(&deck, PlayerPosition::North, PassingDirection::Hold);
        let two_of_clubs = Card::new(Rank::Two, Suit::Clubs);

        let expected = PlayerPosition::LOOP
            .iter()
            .copied()
            .find(|seat| round.hand(*seat).contains(two_of_clubs))
            .expect("two of clubs is dealt");

        assert_eq!(round.current_trick().leader(), expected);
        assert_eq!(round.starting_player(), expected);
    }

    #[test]
    fn first_lead_must_be_two_of_clubs() {
        let deck = Deck::standard();
        let mut round = RoundState::deal(&deck, PlayerPosition::North, PassingDirection::Hold);
        let wrong = {
            let hand = round.hand(PlayerPosition::North);
            hand.iter().copied().find(|&c| c != Card::new(Rank::Two, Suit::Clubs)).unwrap()
        };
        assert!(matches!(
            round.play_card(PlayerPosition::North, wrong),
            Err(PlayError::MustLeadTwoOfClubs)
        ));
        let two = Card::new(Rank::Two, Suit::Clubs);
        assert!(matches!(
            round.play_card(PlayerPosition::North, two),
            Ok(PlayOutcome::Played)
        ));
    }

    #[test]
    fn follow_suit_is_required() {
        let deck = Deck::standard();
        let mut round = RoundState::deal(&deck, PlayerPosition::North, PassingDirection::Hold);
        round
            .play_card(PlayerPosition::North, Card::new(Rank::Two, Suit::Clubs))
            .unwrap();
        let illegal = Card::new(Rank::Two, Suit::Diamonds);
        match round.play_card(PlayerPosition::East, illegal) {
            Err(PlayError::MustFollowSuit(Suit::Clubs)) => {}
            other => panic!("expected MustFollowSuit, got {other:?}"),
        }
        round
            .play_card(PlayerPosition::East, Card::new(Rank::Three, Suit::Clubs))
            .unwrap();
    }

    #[test]
    fn cannot_lead_hearts_before_broken_on_second_trick() {
        let deck = Deck::standard();
        let mut round = RoundState::deal(&deck, PlayerPosition::North, PassingDirection::Hold);
        round
            .play_card(PlayerPosition::North, Card::new(Rank::Two, Suit::Clubs))
            .unwrap();
        round
            .play_card(PlayerPosition::East, Card::new(Rank::Three, Suit::Clubs))
            .unwrap();
        round
            .play_card(PlayerPosition::South, Card::new(Rank::Four, Suit::Clubs))
            .unwrap();
        let outcome = round
            .play_card(PlayerPosition::West, Card::new(Rank::Five, Suit::Clubs))
            .unwrap();
        match outcome {
            PlayOutcome::TrickCompleted { winner, .. } => assert_eq!(winner, PlayerPosition::West),
            other => panic!("expected TrickCompleted, got {other:?}"),
        }
        assert!(!round.hearts_broken());

        let west_hand = round.hand(PlayerPosition::West).clone();
        if let Some(h) = west_hand.iter().find(|c| c.suit == Suit::Hearts).copied() {
            if west_hand.iter().any(|c| !c.suit.is_heart()) {
                assert!(matches!(
                    round.play_card(PlayerPosition::West, h),
                    Err(PlayError::HeartsNotBroken)
                ));
            }
        }
    }
}

