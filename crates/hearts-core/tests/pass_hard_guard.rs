use hearts_core::model::card::Card;
use hearts_core::model::hand::Hand;
use hearts_core::model::passing::{PassingDirection, PassingState};
use hearts_core::model::player::PlayerPosition;
use hearts_core::model::rank::Rank;
use hearts_core::model::round::{RoundPhase, RoundState};
use hearts_core::model::score::ScoreBoard;
use hearts_core::moon::{MoonEstimate, MoonObjective};
use hearts_core::pass::direction::DirectionProfile;
use hearts_core::pass::optimizer::enumerate_pass_triples;
use hearts_core::pass::scoring::{PassScoreInput, PassWeights};
use hearts_core::model::suit::Suit;

fn make_input(cards: Vec<Card>, seat: PlayerPosition) -> PassScoreInput<'static> {
    let passing = PassingDirection::Left;
    let mut hands = [Hand::new(), Hand::new(), Hand::new(), Hand::new()];
    hands[seat.index()] = Hand::with_cards(cards);

    let round = RoundState::from_hands(
        hands,
        seat,
        passing,
        RoundPhase::Passing(PassingState::new(passing)),
    );
    let round = Box::leak(Box::new(round));
    let scores = Box::leak(Box::new(ScoreBoard::new()));
    let moon_estimate = MoonEstimate {
        probability: 0.7,
        raw_score: 1.3,
        objective: MoonObjective::BlockShooter,
    };

    PassScoreInput {
        seat,
        hand: round.hand(seat),
        round,
        scores,
        belief: None,
        weights: PassWeights::default(),
        direction: passing,
        direction_profile: DirectionProfile::from_direction(passing),
        moon_estimate,
    }
}

fn candidate_contains(cards: &[Card; 3], target: &[Card]) -> bool {
    target.iter().all(|card| cards.contains(card))
}

#[test]
fn fixture_hand_75_rejects_qspade_dump() {
    let input = make_input(
        vec![
            Card::new(Rank::Queen, Suit::Spades),
            Card::new(Rank::Seven, Suit::Clubs),
            Card::new(Rank::Six, Suit::Clubs),
            Card::new(Rank::Ace, Suit::Hearts),
            Card::new(Rank::King, Suit::Hearts),
            Card::new(Rank::Queen, Suit::Hearts),
            Card::new(Rank::Jack, Suit::Hearts),
            Card::new(Rank::Ten, Suit::Hearts),
            Card::new(Rank::Nine, Suit::Hearts),
            Card::new(Rank::Eight, Suit::Hearts),
            Card::new(Rank::Five, Suit::Hearts),
            Card::new(Rank::Four, Suit::Hearts),
            Card::new(Rank::Three, Suit::Hearts),
        ],
        PlayerPosition::West,
    );
    let combos = enumerate_pass_triples(&input);
    assert!(
        combos.iter().all(|candidate| {
            candidate
                .cards
                .iter()
                .filter(|card| card.suit == Suit::Hearts && card.rank >= Rank::Ten)
                .count()
                >= 3
        }),
        "expected all combos to send three Ten+ hearts"
    );
    assert!(
        !combos.iter().any(|candidate| candidate_contains(&candidate.cards, &[
            Card::new(Rank::Queen, Suit::Spades),
            Card::new(Rank::Seven, Suit::Clubs),
            Card::new(Rank::Six, Suit::Clubs),
        ])),
        "unexpected Q♠ dump candidate present"
    );
}

#[test]
fn fixture_hand_912_keeps_ace_with_insufficient_support() {
    let input = make_input(
        vec![
            Card::new(Rank::Ace, Suit::Hearts),
            Card::new(Rank::Ten, Suit::Hearts),
            Card::new(Rank::Three, Suit::Hearts),
            Card::new(Rank::Queen, Suit::Spades),
            Card::new(Rank::Jack, Suit::Spades),
            Card::new(Rank::Ten, Suit::Spades),
            Card::new(Rank::Nine, Suit::Diamonds),
            Card::new(Rank::Eight, Suit::Diamonds),
            Card::new(Rank::Seven, Suit::Diamonds),
            Card::new(Rank::Six, Suit::Diamonds),
            Card::new(Rank::Five, Suit::Clubs),
            Card::new(Rank::Four, Suit::Clubs),
            Card::new(Rank::Three, Suit::Clubs),
        ],
        PlayerPosition::North,
    );
    let combos = enumerate_pass_triples(&input);
    assert!(
        combos.iter().all(|candidate| {
            !candidate
                .cards
                .iter()
                .any(|card| card.suit == Suit::Hearts && card.rank == Rank::Ace)
        }),
        "Ace should be retained when support is insufficient"
    );
}

#[test]
fn fixture_hand_498_requires_three_ten_plus_hearts() {
    let input = make_input(
        vec![
            Card::new(Rank::King, Suit::Hearts),
            Card::new(Rank::Jack, Suit::Hearts),
            Card::new(Rank::Four, Suit::Hearts),
            Card::new(Rank::Ace, Suit::Hearts),
            Card::new(Rank::Queen, Suit::Hearts),
            Card::new(Rank::Ten, Suit::Hearts),
            Card::new(Rank::Nine, Suit::Hearts),
            Card::new(Rank::Eight, Suit::Spades),
            Card::new(Rank::Seven, Suit::Diamonds),
            Card::new(Rank::Six, Suit::Diamonds),
            Card::new(Rank::Five, Suit::Clubs),
            Card::new(Rank::Four, Suit::Clubs),
            Card::new(Rank::Three, Suit::Clubs),
        ],
        PlayerPosition::East,
    );
    let combos = enumerate_pass_triples(&input);
    assert!(combos.iter().all(|candidate| {
        !candidate.cards.iter().any(|card| card.rank == Rank::King && card.suit == Suit::Hearts)
            || candidate
                .cards
                .iter()
                .filter(|card| card.suit == Suit::Hearts && card.rank >= Rank::Ten)
                .count()
                >= 3
    }));
}

#[test]
fn fixture_hand_511_rejects_qspade_dump() {
    let input = make_input(
        vec![
            Card::new(Rank::Queen, Suit::Spades),
            Card::new(Rank::Five, Suit::Clubs),
            Card::new(Rank::Four, Suit::Clubs),
            Card::new(Rank::Ace, Suit::Hearts),
            Card::new(Rank::King, Suit::Hearts),
            Card::new(Rank::Queen, Suit::Hearts),
            Card::new(Rank::Jack, Suit::Hearts),
            Card::new(Rank::Ten, Suit::Hearts),
            Card::new(Rank::Nine, Suit::Hearts),
            Card::new(Rank::Eight, Suit::Hearts),
            Card::new(Rank::Seven, Suit::Hearts),
            Card::new(Rank::Six, Suit::Hearts),
            Card::new(Rank::Three, Suit::Hearts),
        ],
        PlayerPosition::South,
    );
    let combos = enumerate_pass_triples(&input);
    assert!(
        !combos.iter().any(|candidate| candidate_contains(&candidate.cards, &[
            Card::new(Rank::Queen, Suit::Spades),
            Card::new(Rank::Four, Suit::Clubs),
            Card::new(Rank::Five, Suit::Clubs),
        ])),
        "unexpected Q♠ dump candidate present"
    );
}

#[test]
fn fixture_hand_767_promotes_heart_splits() {
    let input = make_input(
        vec![
            Card::new(Rank::Queen, Suit::Spades),
            Card::new(Rank::Six, Suit::Clubs),
            Card::new(Rank::Five, Suit::Clubs),
            Card::new(Rank::Ace, Suit::Hearts),
            Card::new(Rank::King, Suit::Hearts),
            Card::new(Rank::Queen, Suit::Hearts),
            Card::new(Rank::Jack, Suit::Hearts),
            Card::new(Rank::Ten, Suit::Hearts),
            Card::new(Rank::Nine, Suit::Hearts),
            Card::new(Rank::Eight, Suit::Hearts),
            Card::new(Rank::Seven, Suit::Hearts),
            Card::new(Rank::Six, Suit::Hearts),
            Card::new(Rank::Three, Suit::Hearts),
        ],
        PlayerPosition::North,
    );
    let combos = enumerate_pass_triples(&input);
    assert!(
        !combos.iter().any(|candidate| candidate_contains(&candidate.cards, &[
            Card::new(Rank::Queen, Suit::Spades),
            Card::new(Rank::Six, Suit::Clubs),
            Card::new(Rank::Five, Suit::Clubs),
        ])),
        "unexpected Q♠ dump candidate present"
    );
}
