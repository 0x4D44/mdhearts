use hearts_core::game::match_state::MatchState;
use hearts_core::model::card::Card;
use hearts_core::model::hand::Hand;
use hearts_core::model::passing::{PassingDirection, PassingState};
use hearts_core::model::player::PlayerPosition;
use hearts_core::model::rank::Rank;
use hearts_core::model::round::{RoundPhase, RoundState};
use hearts_core::model::score::ScoreBoard;
use hearts_core::model::suit::Suit;
use hearts_core::moon::{MoonEstimate, MoonObjective};
use hearts_core::pass::direction::DirectionProfile;
use hearts_core::pass::optimizer::{enumerate_pass_triples, force_guarded_pass};
use hearts_core::pass::scoring::{PassScoreInput, PassWeights};
use rand::RngCore;
use rand::SeedableRng;
use rand::rngs::StdRng;

fn base_seed_for(hand_index: usize) -> u64 {
    let mut rng = StdRng::seed_from_u64(20251017);
    let mut base_seed = 0u64;
    for _ in 0..=hand_index {
        base_seed = rng.next_u64();
    }
    base_seed
}

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

fn stage2_input(hand_index: usize, seat: PlayerPosition) -> PassScoreInput<'static> {
    let base_seed = base_seed_for(hand_index);
    let state = MatchState::with_seed(PlayerPosition::North, base_seed);
    let round = state.round();
    let cards: Vec<Card> = round.hand(seat).iter().copied().collect();
    assert_eq!(
        cards.len(),
        13,
        "expected full hand for hand {hand_index} seat {seat:?}"
    );
    make_input(cards, seat)
}

#[test]
fn stage2_force_guarded_pass_preserves_stoppers() {
    struct Case {
        hand_index: usize,
        seat: PlayerPosition,
        forbid_ace: bool,
        forbid_king: bool,
    }
    let cases = [
        Case {
            hand_index: 75,
            seat: PlayerPosition::West,
            forbid_ace: true,
            forbid_king: false,
        },
        Case {
            hand_index: 511,
            seat: PlayerPosition::South,
            forbid_ace: true,
            forbid_king: true,
        },
        Case {
            hand_index: 757,
            seat: PlayerPosition::North,
            forbid_ace: true,
            forbid_king: true,
        },
        Case {
            hand_index: 767,
            seat: PlayerPosition::North,
            forbid_ace: true,
            forbid_king: true,
        },
    ];

    for case in cases {
        let input = stage2_input(case.hand_index, case.seat);
        let forced = force_guarded_pass(&input)
            .unwrap_or_else(|| panic!("expected forced candidate for hand {}", case.hand_index));
        if case.forbid_ace {
            assert!(
                !forced
                    .cards
                    .iter()
                    .any(|card| { card.suit == Suit::Hearts && card.rank == Rank::Ace }),
                "forced pass should retain A♥ for hand {} seat {:?}, got {:?}",
                case.hand_index,
                case.seat,
                forced.cards
            );
        }
        if case.forbid_king {
            assert!(
                !forced
                    .cards
                    .iter()
                    .any(|card| { card.suit == Suit::Hearts && card.rank == Rank::King }),
                "forced pass should retain K♥ for hand {} seat {:?}, got {:?}",
                case.hand_index,
                case.seat,
                forced.cards
            );
        }
    }
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
    let invalid: Vec<[Card; 3]> = combos
        .iter()
        .filter(|candidate| {
            candidate
                .cards
                .iter()
                .filter(|card| card.suit == Suit::Hearts && card.rank >= Rank::Ten)
                .count()
                < 3
        })
        .map(|candidate| candidate.cards)
        .collect();
    assert!(
        invalid.is_empty(),
        "expected all combos to send three Ten+ hearts, found {invalid:?}"
    );
    assert!(
        !combos.iter().any(|candidate| candidate_contains(
            &candidate.cards,
            &[
                Card::new(Rank::Queen, Suit::Spades),
                Card::new(Rank::Seven, Suit::Clubs),
                Card::new(Rank::Six, Suit::Clubs),
            ]
        )),
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
        !candidate
            .cards
            .iter()
            .any(|card| card.rank == Rank::King && card.suit == Suit::Hearts)
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
        !combos.iter().any(|candidate| candidate_contains(
            &candidate.cards,
            &[
                Card::new(Rank::Queen, Suit::Spades),
                Card::new(Rank::Four, Suit::Clubs),
                Card::new(Rank::Five, Suit::Clubs),
            ]
        )),
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
        !combos.iter().any(|candidate| candidate_contains(
            &candidate.cards,
            &[
                Card::new(Rank::Queen, Suit::Spades),
                Card::new(Rank::Six, Suit::Clubs),
                Card::new(Rank::Five, Suit::Clubs),
            ]
        )),
        "unexpected Q♠ dump candidate present"
    );

    for candidate in combos
        .iter()
        .filter(|candidate| candidate.cards.iter().any(|card| card.suit == Suit::Hearts))
    {
        let ten_plus = candidate
            .cards
            .iter()
            .filter(|card| card.suit == Suit::Hearts && card.rank >= Rank::Ten)
            .count();
        assert!(
            ten_plus > 0,
            "expected low-heart triple to include at least one Ten+ heart: {:?}",
            candidate.cards
        );
    }
}

#[test]
fn fallback_injects_best_available_heart_when_ten_plus_short() {
    let input = make_input(
        vec![
            Card::new(Rank::Ace, Suit::Hearts),
            Card::new(Rank::Queen, Suit::Hearts),
            Card::new(Rank::Nine, Suit::Hearts),
            Card::new(Rank::Seven, Suit::Hearts),
            Card::new(Rank::Four, Suit::Hearts),
            Card::new(Rank::King, Suit::Spades),
            Card::new(Rank::Queen, Suit::Spades),
            Card::new(Rank::Jack, Suit::Spades),
            Card::new(Rank::Ten, Suit::Spades),
            Card::new(Rank::Six, Suit::Clubs),
            Card::new(Rank::Five, Suit::Clubs),
            Card::new(Rank::Three, Suit::Diamonds),
            Card::new(Rank::Two, Suit::Diamonds),
        ],
        PlayerPosition::North,
    );
    let combos = enumerate_pass_triples(&input);
    for candidate in combos.iter().filter(|candidate| {
        candidate
            .cards
            .iter()
            .any(|card| *card == Card::new(Rank::Ace, Suit::Hearts))
            && candidate
                .cards
                .iter()
                .any(|card| *card == Card::new(Rank::Queen, Suit::Hearts))
    }) {
        assert_eq!(
            candidate
                .cards
                .iter()
                .filter(|card| card.suit == Suit::Hearts)
                .count(),
            3,
            "expected fallback to promote an additional heart when Ten+ supply is short"
        );
        assert!(
            !candidate
                .cards
                .iter()
                .any(|card| card == &Card::new(Rank::King, Suit::Spades)),
            "fallback should replace K♠ with best available heart: {:?}",
            candidate.cards
        );
    }
}

#[test]
fn fixture_hand_32_requires_three_ten_plus_when_passing_qheart() {
    let input = make_input(
        vec![
            Card::new(Rank::Ace, Suit::Hearts),
            Card::new(Rank::King, Suit::Hearts),
            Card::new(Rank::Queen, Suit::Hearts),
            Card::new(Rank::Jack, Suit::Hearts),
            Card::new(Rank::Ten, Suit::Hearts),
            Card::new(Rank::Nine, Suit::Hearts),
            Card::new(Rank::Four, Suit::Hearts),
            Card::new(Rank::Ten, Suit::Clubs),
            Card::new(Rank::Eight, Suit::Clubs),
            Card::new(Rank::Seven, Suit::Clubs),
            Card::new(Rank::Six, Suit::Spades),
            Card::new(Rank::Five, Suit::Diamonds),
            Card::new(Rank::Three, Suit::Diamonds),
        ],
        PlayerPosition::West,
    );
    let combos = enumerate_pass_triples(&input);
    for candidate in combos.iter().filter(|candidate| {
        candidate
            .cards
            .iter()
            .any(|card| card == &Card::new(Rank::Queen, Suit::Hearts))
    }) {
        let ten_plus = candidate
            .cards
            .iter()
            .filter(|card| card.suit == Suit::Hearts && card.rank >= Rank::Ten)
            .count();
        assert!(
            ten_plus >= 3,
            "passing Q♥ must include three Ten+ hearts: {:?}",
            candidate.cards
        );
        assert!(
            !candidate.cards.contains(&Card::new(Rank::Ten, Suit::Clubs)),
            "expected Ten♣ to be replaced by a premium heart: {:?}",
            candidate.cards
        );
    }
}

#[test]
fn fixture_hand_567_requires_three_premium_hearts() {
    let input = make_input(
        vec![
            Card::new(Rank::Ace, Suit::Hearts),
            Card::new(Rank::King, Suit::Hearts),
            Card::new(Rank::Queen, Suit::Hearts),
            Card::new(Rank::Jack, Suit::Hearts),
            Card::new(Rank::Ten, Suit::Hearts),
            Card::new(Rank::Nine, Suit::Hearts),
            Card::new(Rank::Eight, Suit::Hearts),
            Card::new(Rank::Seven, Suit::Spades),
            Card::new(Rank::Six, Suit::Clubs),
            Card::new(Rank::Five, Suit::Diamonds),
            Card::new(Rank::Four, Suit::Diamonds),
            Card::new(Rank::Three, Suit::Diamonds),
            Card::new(Rank::Two, Suit::Clubs),
        ],
        PlayerPosition::West,
    );
    let combos = enumerate_pass_triples(&input);
    for candidate in combos.iter().filter(|candidate| {
        candidate
            .cards
            .iter()
            .any(|card| card.suit == Suit::Hearts && card.rank >= Rank::Queen)
    }) {
        let ten_plus = candidate
            .cards
            .iter()
            .filter(|card| card.suit == Suit::Hearts && card.rank >= Rank::Ten)
            .count();
        assert!(
            ten_plus >= 3,
            "passing Q♥/K♥/A♥ must include three Ten+ hearts: {:?}",
            candidate.cards
        );
    }
}

#[test]
fn fixture_hand_757_prevents_ace_club_anchor() {
    let input = make_input(
        vec![
            Card::new(Rank::Ace, Suit::Hearts),
            Card::new(Rank::King, Suit::Hearts),
            Card::new(Rank::Queen, Suit::Hearts),
            Card::new(Rank::Jack, Suit::Hearts),
            Card::new(Rank::Ten, Suit::Hearts),
            Card::new(Rank::Nine, Suit::Hearts),
            Card::new(Rank::Eight, Suit::Hearts),
            Card::new(Rank::Ace, Suit::Clubs),
            Card::new(Rank::Seven, Suit::Clubs),
            Card::new(Rank::Six, Suit::Clubs),
            Card::new(Rank::Five, Suit::Clubs),
            Card::new(Rank::Four, Suit::Clubs),
            Card::new(Rank::Three, Suit::Clubs),
        ],
        PlayerPosition::North,
    );
    let combos = enumerate_pass_triples(&input);
    assert!(
        !combos.iter().any(|candidate| candidate_contains(
            &candidate.cards,
            &[
                Card::new(Rank::Ace, Suit::Hearts),
                Card::new(Rank::King, Suit::Hearts),
                Card::new(Rank::Ace, Suit::Clubs),
            ]
        )),
        "expected guard to reject heart+club anchor pass"
    );
}

#[test]
fn fixture_hand_890_rejects_offsuit_dump() {
    let input = make_input(
        vec![
            Card::new(Rank::Jack, Suit::Spades),
            Card::new(Rank::Seven, Suit::Diamonds),
            Card::new(Rank::Six, Suit::Clubs),
            Card::new(Rank::Ace, Suit::Hearts),
            Card::new(Rank::King, Suit::Hearts),
            Card::new(Rank::Queen, Suit::Hearts),
            Card::new(Rank::Jack, Suit::Hearts),
            Card::new(Rank::Ten, Suit::Hearts),
            Card::new(Rank::Nine, Suit::Hearts),
            Card::new(Rank::Eight, Suit::Hearts),
            Card::new(Rank::Seven, Suit::Hearts),
            Card::new(Rank::Six, Suit::Hearts),
            Card::new(Rank::Five, Suit::Hearts),
        ],
        PlayerPosition::West,
    );
    let combos = enumerate_pass_triples(&input);
    assert!(
        !combos.iter().any(|candidate| candidate_contains(
            &candidate.cards,
            &[
                Card::new(Rank::Jack, Suit::Spades),
                Card::new(Rank::Seven, Suit::Diamonds),
                Card::new(Rank::Six, Suit::Clubs),
            ]
        )),
        "expected off-suit dump to be rejected"
    );
}

#[test]
fn fixture_hand_153_requires_three_ten_plus_hearts() {
    let input = make_input(
        vec![
            Card::new(Rank::Ace, Suit::Hearts),
            Card::new(Rank::King, Suit::Hearts),
            Card::new(Rank::Queen, Suit::Hearts),
            Card::new(Rank::Jack, Suit::Hearts),
            Card::new(Rank::Ten, Suit::Hearts),
            Card::new(Rank::Nine, Suit::Hearts),
            Card::new(Rank::Eight, Suit::Hearts),
            Card::new(Rank::Two, Suit::Hearts),
            Card::new(Rank::Queen, Suit::Spades),
            Card::new(Rank::Seven, Suit::Clubs),
            Card::new(Rank::Six, Suit::Clubs),
            Card::new(Rank::Five, Suit::Diamonds),
            Card::new(Rank::Four, Suit::Diamonds),
        ],
        PlayerPosition::South,
    );
    let combos = enumerate_pass_triples(&input);
    assert!(
        !combos.iter().any(|candidate| candidate_contains(
            &candidate.cards,
            &[
                Card::new(Rank::Two, Suit::Hearts),
                Card::new(Rank::King, Suit::Hearts),
                Card::new(Rank::Ace, Suit::Hearts),
            ]
        )),
        "expected to avoid passing only two premium hearts with a low heart kicker"
    );
    for candidate in combos.iter().filter(|candidate| {
        candidate.cards.iter().any(|card| {
            card.suit == Suit::Hearts && (card.rank == Rank::Ace || card.rank == Rank::King)
        })
    }) {
        let ten_plus = candidate
            .cards
            .iter()
            .filter(|card| card.suit == Suit::Hearts && card.rank >= Rank::Ten)
            .count();
        assert!(
            ten_plus >= 3,
            "expected three Ten+ hearts when shipping A/K, got {:?}",
            candidate.cards
        );
    }
}

#[test]
fn fixture_hand_242_blocks_qspade_dump() {
    let input = make_input(
        vec![
            Card::new(Rank::Ace, Suit::Hearts),
            Card::new(Rank::King, Suit::Hearts),
            Card::new(Rank::Queen, Suit::Hearts),
            Card::new(Rank::Jack, Suit::Hearts),
            Card::new(Rank::Ten, Suit::Hearts),
            Card::new(Rank::Nine, Suit::Hearts),
            Card::new(Rank::Seven, Suit::Hearts),
            Card::new(Rank::Queen, Suit::Spades),
            Card::new(Rank::Eight, Suit::Clubs),
            Card::new(Rank::Seven, Suit::Clubs),
            Card::new(Rank::Five, Suit::Diamonds),
            Card::new(Rank::Three, Suit::Spades),
            Card::new(Rank::Two, Suit::Spades),
        ],
        PlayerPosition::North,
    );
    let combos = enumerate_pass_triples(&input);
    let offenders: Vec<[Card; 3]> = combos
        .iter()
        .filter(|candidate| {
            candidate_contains(
                &candidate.cards,
                &[
                    Card::new(Rank::Queen, Suit::Spades),
                    Card::new(Rank::Seven, Suit::Clubs),
                    Card::new(Rank::Eight, Suit::Clubs),
                ],
            )
        })
        .map(|candidate| candidate.cards)
        .collect();
    assert!(
        offenders.is_empty(),
        "unexpected Q♠ + clubs dump persisted: {offenders:?}"
    );
}

#[test]
fn fixture_hand_432_requires_ten_plus_substitution() {
    let input = make_input(
        vec![
            Card::new(Rank::Ace, Suit::Hearts),
            Card::new(Rank::King, Suit::Hearts),
            Card::new(Rank::Queen, Suit::Hearts),
            Card::new(Rank::Jack, Suit::Hearts),
            Card::new(Rank::Ten, Suit::Hearts),
            Card::new(Rank::Nine, Suit::Hearts),
            Card::new(Rank::Four, Suit::Hearts),
            Card::new(Rank::Queen, Suit::Spades),
            Card::new(Rank::Jack, Suit::Spades),
            Card::new(Rank::Seven, Suit::Clubs),
            Card::new(Rank::Six, Suit::Clubs),
            Card::new(Rank::Five, Suit::Diamonds),
            Card::new(Rank::Three, Suit::Diamonds),
        ],
        PlayerPosition::East,
    );
    let combos = enumerate_pass_triples(&input);
    assert!(
        !combos.iter().any(|candidate| candidate_contains(
            &candidate.cards,
            &[
                Card::new(Rank::Four, Suit::Hearts),
                Card::new(Rank::Jack, Suit::Hearts),
                Card::new(Rank::King, Suit::Hearts),
            ]
        )),
        "expected low-heart kicker to be replaced by Ten+ support"
    );
    for candidate in combos.iter().filter(|candidate| {
        candidate
            .cards
            .iter()
            .any(|card| card.suit == Suit::Hearts && card.rank == Rank::King)
    }) {
        let ten_plus = candidate
            .cards
            .iter()
            .filter(|card| card.suit == Suit::Hearts && card.rank >= Rank::Ten)
            .count();
        assert!(
            ten_plus >= 3,
            "expected replacement Ten+ heart present, got {:?}",
            candidate.cards
        );
    }
}

#[test]
fn fixture_hand_461_rejects_qspade_ace_combo() {
    let input = make_input(
        vec![
            Card::new(Rank::Ace, Suit::Hearts),
            Card::new(Rank::King, Suit::Hearts),
            Card::new(Rank::Queen, Suit::Hearts),
            Card::new(Rank::Jack, Suit::Hearts),
            Card::new(Rank::Ten, Suit::Hearts),
            Card::new(Rank::Nine, Suit::Hearts),
            Card::new(Rank::Seven, Suit::Hearts),
            Card::new(Rank::Queen, Suit::Spades),
            Card::new(Rank::Six, Suit::Clubs),
            Card::new(Rank::Seven, Suit::Clubs),
            Card::new(Rank::Five, Suit::Diamonds),
            Card::new(Rank::Four, Suit::Diamonds),
            Card::new(Rank::Three, Suit::Spades),
        ],
        PlayerPosition::North,
    );
    let combos = enumerate_pass_triples(&input);
    assert!(
        !combos.iter().any(|candidate| candidate_contains(
            &candidate.cards,
            &[
                Card::new(Rank::Six, Suit::Clubs),
                Card::new(Rank::Queen, Suit::Spades),
                Card::new(Rank::Ace, Suit::Hearts),
            ]
        )),
        "expected Ace guard to reject Q♠ + club pass"
    );
}

#[test]
fn fixture_hand_681_rejects_qspade_ace_combo() {
    let input = make_input(
        vec![
            Card::new(Rank::Ace, Suit::Hearts),
            Card::new(Rank::King, Suit::Hearts),
            Card::new(Rank::Queen, Suit::Hearts),
            Card::new(Rank::Jack, Suit::Hearts),
            Card::new(Rank::Ten, Suit::Hearts),
            Card::new(Rank::Nine, Suit::Hearts),
            Card::new(Rank::Eight, Suit::Hearts),
            Card::new(Rank::Queen, Suit::Spades),
            Card::new(Rank::Five, Suit::Clubs),
            Card::new(Rank::Six, Suit::Clubs),
            Card::new(Rank::Seven, Suit::Clubs),
            Card::new(Rank::Five, Suit::Diamonds),
            Card::new(Rank::Four, Suit::Diamonds),
        ],
        PlayerPosition::North,
    );
    let combos = enumerate_pass_triples(&input);
    assert!(
        !combos.iter().any(|candidate| candidate_contains(
            &candidate.cards,
            &[
                Card::new(Rank::Five, Suit::Clubs),
                Card::new(Rank::Queen, Suit::Spades),
                Card::new(Rank::Ace, Suit::Hearts),
            ]
        )),
        "expected Ace guard to reject Q♠ + club pass"
    );
}

#[test]
fn fixture_hand_757_rejects_qspade_dump() {
    let input = make_input(
        vec![
            Card::new(Rank::Ace, Suit::Hearts),
            Card::new(Rank::King, Suit::Hearts),
            Card::new(Rank::Queen, Suit::Hearts),
            Card::new(Rank::Jack, Suit::Hearts),
            Card::new(Rank::Ten, Suit::Hearts),
            Card::new(Rank::Nine, Suit::Hearts),
            Card::new(Rank::Eight, Suit::Hearts),
            Card::new(Rank::Queen, Suit::Spades),
            Card::new(Rank::Seven, Suit::Clubs),
            Card::new(Rank::Six, Suit::Clubs),
            Card::new(Rank::Two, Suit::Clubs),
            Card::new(Rank::Five, Suit::Diamonds),
            Card::new(Rank::Three, Suit::Spades),
        ],
        PlayerPosition::North,
    );
    let combos = enumerate_pass_triples(&input);
    assert!(
        !combos.iter().any(|candidate| candidate_contains(
            &candidate.cards,
            &[
                Card::new(Rank::Seven, Suit::Clubs),
                Card::new(Rank::Six, Suit::Clubs),
                Card::new(Rank::Queen, Suit::Spades),
            ]
        )),
        "unexpected Q♠ + clubs dump persisted"
    );
}
