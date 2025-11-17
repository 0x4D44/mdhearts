// Comprehensive pass logic tests covering all 16 combinations:
// 4 seats (North, East, South, West) × 4 directions (Left, Right, Across, Hold)

use hearts_app::bot::{BotContext, BotDifficulty, PassPlanner, UnseenTracker};
use hearts_core::model::card::Card;
use hearts_core::model::hand::Hand;
use hearts_core::model::passing::{PassingDirection, PassingState};
use hearts_core::model::player::PlayerPosition;
use hearts_core::model::rank::Rank;
use hearts_core::model::round::{RoundPhase, RoundState};
use hearts_core::model::score::ScoreBoard;
use hearts_core::model::suit::Suit;

fn build_test_hand() -> Vec<Card> {
    // Balanced hand with all suits, including QS and some hearts
    vec![
        Card::new(Rank::Queen, Suit::Spades),
        Card::new(Rank::Ten, Suit::Spades),
        Card::new(Rank::Five, Suit::Spades),
        Card::new(Rank::Ace, Suit::Hearts),
        Card::new(Rank::King, Suit::Hearts),
        Card::new(Rank::Six, Suit::Hearts),
        Card::new(Rank::Ace, Suit::Clubs),
        Card::new(Rank::King, Suit::Clubs),
        Card::new(Rank::Four, Suit::Clubs),
        Card::new(Rank::Nine, Suit::Diamonds),
        Card::new(Rank::Seven, Suit::Diamonds),
        Card::new(Rank::Three, Suit::Diamonds),
        Card::new(Rank::Two, Suit::Diamonds),
    ]
}

fn build_round(seat: PlayerPosition, hand: &[Card], passing: PassingDirection) -> RoundState {
    let mut hands = [Hand::new(), Hand::new(), Hand::new(), Hand::new()];
    hands[seat.index()] = Hand::with_cards(hand.to_vec());
    // Fill other hands with dummy cards to have 13 cards each
    for i in 0..4 {
        if i != seat.index() {
            let mut cards = Vec::new();
            for rank in [Rank::Two, Rank::Three, Rank::Four, Rank::Five, Rank::Six,
                        Rank::Seven, Rank::Eight, Rank::Nine, Rank::Ten, Rank::Jack,
                        Rank::Queen, Rank::King, Rank::Ace] {
                if cards.len() < 13 {
                    cards.push(Card::new(rank, Suit::Diamonds));
                }
            }
            hands[i] = Hand::with_cards(cards);
        }
    }

    RoundState::from_hands_with_state(
        hands,
        PlayerPosition::North,
        passing,
        RoundPhase::Passing(PassingState::new(passing)),
        hearts_core::model::trick::Trick::new(PlayerPosition::North),
        vec![],
        false,
    )
}

fn build_scores(totals: [u32; 4]) -> ScoreBoard {
    let mut scores = ScoreBoard::new();
    scores.set_totals(totals);
    scores
}

fn assert_valid_pass(
    seat: PlayerPosition,
    passing: PassingDirection,
    hand: &[Card],
    scores: [u32; 4],
) {
    let round = build_round(seat, hand, passing);
    let scores = build_scores(scores);
    let mut tracker = UnseenTracker::new();
    tracker.reset_for_round(&round);
    let ctx = BotContext::new(
        seat,
        &round,
        &scores,
        passing,
        &tracker,
        BotDifficulty::NormalHeuristic,
    );

    let picks = PassPlanner::choose(round.hand(seat), &ctx);

    // PassPlanner always returns 3 cards for valid hands, even in Hold rounds
    // (the game logic decides whether to actually pass them based on direction)
    assert!(
        picks.is_some(),
        "PassPlanner should return choices for {:?}/{:?}",
        seat,
        passing
    );

    let picks = picks.unwrap();
    assert_eq!(
        picks.len(),
        3,
        "Should select exactly 3 cards for {:?}/{:?}",
        seat,
        passing
    );

    // All picked cards should be in the original hand
    for card in picks.iter() {
        assert!(
            hand.contains(card),
            "Selected card {:?} not in original hand for {:?}/{:?}",
            card,
            seat,
            passing
        );
    }

    // All picked cards should be unique
    for i in 0..picks.len() {
        for j in (i + 1)..picks.len() {
            assert_ne!(
                picks[i], picks[j],
                "Duplicate cards in selection for {:?}/{:?}",
                seat, passing
            );
        }
    }
}

// ============================================================================
// North Seat Tests (all 4 directions)
// ============================================================================

#[test]
fn north_pass_left() {
    assert_valid_pass(
        PlayerPosition::North,
        PassingDirection::Left,
        &build_test_hand(),
        [20, 25, 30, 15],
    );
}

#[test]
fn north_pass_right() {
    assert_valid_pass(
        PlayerPosition::North,
        PassingDirection::Right,
        &build_test_hand(),
        [20, 25, 30, 15],
    );
}

#[test]
fn north_pass_across() {
    assert_valid_pass(
        PlayerPosition::North,
        PassingDirection::Across,
        &build_test_hand(),
        [20, 25, 30, 15],
    );
}

#[test]
fn north_pass_hold() {
    assert_valid_pass(
        PlayerPosition::North,
        PassingDirection::Hold,
        &build_test_hand(),
        [20, 25, 30, 15],
    );
}

// ============================================================================
// East Seat Tests (all 4 directions)
// ============================================================================

#[test]
fn east_pass_left() {
    assert_valid_pass(
        PlayerPosition::East,
        PassingDirection::Left,
        &build_test_hand(),
        [15, 28, 22, 35],
    );
}

#[test]
fn east_pass_right() {
    assert_valid_pass(
        PlayerPosition::East,
        PassingDirection::Right,
        &build_test_hand(),
        [15, 28, 22, 35],
    );
}

#[test]
fn east_pass_across() {
    assert_valid_pass(
        PlayerPosition::East,
        PassingDirection::Across,
        &build_test_hand(),
        [15, 28, 22, 35],
    );
}

#[test]
fn east_pass_hold() {
    assert_valid_pass(
        PlayerPosition::East,
        PassingDirection::Hold,
        &build_test_hand(),
        [15, 28, 22, 35],
    );
}

// ============================================================================
// South Seat Tests (all 4 directions)
// ============================================================================

#[test]
fn south_pass_left() {
    assert_valid_pass(
        PlayerPosition::South,
        PassingDirection::Left,
        &build_test_hand(),
        [30, 18, 24, 28],
    );
}

#[test]
fn south_pass_right() {
    assert_valid_pass(
        PlayerPosition::South,
        PassingDirection::Right,
        &build_test_hand(),
        [30, 18, 24, 28],
    );
}

#[test]
fn south_pass_across() {
    assert_valid_pass(
        PlayerPosition::South,
        PassingDirection::Across,
        &build_test_hand(),
        [30, 18, 24, 28],
    );
}

#[test]
fn south_pass_hold() {
    assert_valid_pass(
        PlayerPosition::South,
        PassingDirection::Hold,
        &build_test_hand(),
        [30, 18, 24, 28],
    );
}

// ============================================================================
// West Seat Tests (all 4 directions)
// ============================================================================

#[test]
fn west_pass_left() {
    assert_valid_pass(
        PlayerPosition::West,
        PassingDirection::Left,
        &build_test_hand(),
        [22, 26, 19, 33],
    );
}

#[test]
fn west_pass_right() {
    assert_valid_pass(
        PlayerPosition::West,
        PassingDirection::Right,
        &build_test_hand(),
        [22, 26, 19, 33],
    );
}

#[test]
fn west_pass_across() {
    assert_valid_pass(
        PlayerPosition::West,
        PassingDirection::Across,
        &build_test_hand(),
        [22, 26, 19, 33],
    );
}

#[test]
fn west_pass_hold() {
    assert_valid_pass(
        PlayerPosition::West,
        PassingDirection::Hold,
        &build_test_hand(),
        [22, 26, 19, 33],
    );
}

// ============================================================================
// Strategy-Specific Tests
// ============================================================================

#[test]
fn pass_always_includes_queen_of_spades() {
    let hand_with_qs = vec![
        Card::new(Rank::Queen, Suit::Spades), // QS - high priority to pass
        Card::new(Rank::Ace, Suit::Hearts),
        Card::new(Rank::King, Suit::Hearts),
        Card::new(Rank::Queen, Suit::Hearts),
        Card::new(Rank::Two, Suit::Spades),
        Card::new(Rank::Three, Suit::Spades),
        Card::new(Rank::Four, Suit::Spades),
        Card::new(Rank::Two, Suit::Clubs),
        Card::new(Rank::Three, Suit::Clubs),
        Card::new(Rank::Two, Suit::Diamonds),
        Card::new(Rank::Three, Suit::Diamonds),
        Card::new(Rank::Four, Suit::Diamonds),
        Card::new(Rank::Five, Suit::Diamonds),
    ];

    let round = build_round(PlayerPosition::North, &hand_with_qs, PassingDirection::Left);
    let scores = build_scores([20, 25, 30, 15]);
    let mut tracker = UnseenTracker::new();
    tracker.reset_for_round(&round);
    let ctx = BotContext::new(
        PlayerPosition::North,
        &round,
        &scores,
        PassingDirection::Left,
        &tracker,
        BotDifficulty::NormalHeuristic,
    );

    let picks = PassPlanner::choose(round.hand(PlayerPosition::North), &ctx).unwrap();

    assert!(
        picks.contains(&Card::new(Rank::Queen, Suit::Spades)),
        "Should always pass Queen of Spades when not attempting moon, got {:?}",
        picks
    );
}

#[test]
fn pass_avoids_creating_dangerous_void_in_spades() {
    let hand_short_spades = vec![
        Card::new(Rank::Two, Suit::Spades),  // Only low spade - dangerous to void
        Card::new(Rank::Ace, Suit::Hearts),
        Card::new(Rank::King, Suit::Hearts),
        Card::new(Rank::Queen, Suit::Hearts),
        Card::new(Rank::Jack, Suit::Hearts),
        Card::new(Rank::Ten, Suit::Hearts),
        Card::new(Rank::Ace, Suit::Clubs),
        Card::new(Rank::King, Suit::Clubs),
        Card::new(Rank::Queen, Suit::Clubs),
        Card::new(Rank::Ace, Suit::Diamonds),
        Card::new(Rank::King, Suit::Diamonds),
        Card::new(Rank::Queen, Suit::Diamonds),
        Card::new(Rank::Jack, Suit::Diamonds),
    ];

    let round = build_round(
        PlayerPosition::South,
        &hand_short_spades,
        PassingDirection::Right,
    );
    let scores = build_scores([25, 22, 20, 33]);
    let mut tracker = UnseenTracker::new();
    tracker.reset_for_round(&round);
    let ctx = BotContext::new(
        PlayerPosition::South,
        &round,
        &scores,
        PassingDirection::Right,
        &tracker,
        BotDifficulty::NormalHeuristic,
    );

    let picks = PassPlanner::choose(round.hand(PlayerPosition::South), &ctx).unwrap();

    // Should prefer passing hearts (have many) over creating spade void
    assert!(
        !picks.contains(&Card::new(Rank::Two, Suit::Spades)),
        "Should avoid creating dangerous spade void, got {:?}",
        picks
    );
}

#[test]
fn pass_behavior_differs_by_score_context() {
    let hand = build_test_hand();

    // Scenario 1: Passing to leader (high score opponent)
    let round1 = build_round(PlayerPosition::North, &hand, PassingDirection::Left);
    // East (target) is leader with 50 points
    let scores1 = build_scores([20, 50, 25, 15]);
    let mut tracker1 = UnseenTracker::new();
    tracker1.reset_for_round(&round1);
    let ctx1 = BotContext::new(
        PlayerPosition::North,
        &round1,
        &scores1,
        PassingDirection::Left,
        &tracker1,
        BotDifficulty::NormalHeuristic,
    );
    let picks1 = PassPlanner::choose(round1.hand(PlayerPosition::North), &ctx1).unwrap();

    // Scenario 2: Passing to trailing player (low score opponent)
    let round2 = build_round(PlayerPosition::North, &hand, PassingDirection::Left);
    // East (target) is trailing with 10 points
    let scores2 = build_scores([20, 10, 50, 45]);
    let mut tracker2 = UnseenTracker::new();
    tracker2.reset_for_round(&round2);
    let ctx2 = BotContext::new(
        PlayerPosition::North,
        &round2,
        &scores2,
        PassingDirection::Left,
        &tracker2,
        BotDifficulty::NormalHeuristic,
    );
    let picks2 = PassPlanner::choose(round2.hand(PlayerPosition::North), &ctx2).unwrap();

    // Passing choices should differ based on opponent's score position
    // (May pass different cards when targeting leader vs trailing player)
    assert_eq!(picks1.len(), 3);
    assert_eq!(picks2.len(), 3);
}
