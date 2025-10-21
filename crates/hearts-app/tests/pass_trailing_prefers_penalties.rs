use hearts_app::bot::{BotContext, BotDifficulty, PassPlanner, UnseenTracker};
use hearts_core::model::card::Card;
use hearts_core::model::hand::Hand;
use hearts_core::model::passing::{PassingDirection, PassingState};
use hearts_core::model::player::PlayerPosition;
use hearts_core::model::rank::Rank;
use hearts_core::model::round::{RoundPhase, RoundState};
use hearts_core::model::score::ScoreBoard;
use hearts_core::model::suit::Suit;

fn build_round(
    seat: PlayerPosition,
    hand_cards: &[Card],
    passing_direction: PassingDirection,
) -> RoundState {
    let mut hands = [Hand::new(), Hand::new(), Hand::new(), Hand::new()];
    hands[seat.index()] = Hand::with_cards(hand_cards.to_vec());
    let phase = RoundPhase::Passing(PassingState::new(passing_direction));
    RoundState::from_hands(hands, seat, passing_direction, phase)
}

fn build_scores(values: [u32; 4]) -> ScoreBoard {
    let mut scores = ScoreBoard::new();
    for (idx, value) in values.iter().enumerate() {
        if let Some(pos) = PlayerPosition::from_index(idx) {
            scores.set_score(pos, *value);
        }
    }
    scores
}

#[test]
fn passing_to_trailing_prefers_penalties() {
    // We are North. Passing is Right => pass to East. East is trailing (max score) and should receive penalties.
    let seat = PlayerPosition::North;
    let passing = PassingDirection::Right;
    let hand = vec![
        Card::new(Rank::Queen, Suit::Spades), // big penalty to pass
        Card::new(Rank::Ace, Suit::Hearts),
        Card::new(Rank::King, Suit::Hearts),
        Card::new(Rank::Ten, Suit::Hearts),
        Card::new(Rank::Two, Suit::Clubs),
        Card::new(Rank::Three, Suit::Clubs),
        Card::new(Rank::Four, Suit::Clubs),
        Card::new(Rank::Five, Suit::Clubs),
        Card::new(Rank::Six, Suit::Clubs),
        Card::new(Rank::Seven, Suit::Clubs),
        Card::new(Rank::Eight, Suit::Diamonds),
        Card::new(Rank::Nine, Suit::Diamonds),
        Card::new(Rank::Ten, Suit::Diamonds),
    ];
    let round = build_round(seat, &hand, passing);
    // East is trailing (highest score)
    let scores = build_scores([20, 50, 30, 25]);
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

    let picks = PassPlanner::choose(round.hand(seat), &ctx).unwrap();
    // Should include QS
    assert!(
        picks.contains(&Card::new(Rank::Queen, Suit::Spades)),
        "picks: {:?}",
        picks
    );
    // And at least one heart
    assert!(
        picks.iter().any(|c| c.suit == Suit::Hearts),
        "picks: {:?}",
        picks
    );
}
