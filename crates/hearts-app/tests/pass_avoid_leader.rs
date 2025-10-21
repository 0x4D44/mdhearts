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
fn avoid_passing_penalties_to_leader() {
    // We are North. Passing is Right => pass to East. East is current scoreboard leader.
    let seat = PlayerPosition::North;
    let passing = PassingDirection::Left;
    let hand = vec![
        Card::new(Rank::Queen, Suit::Spades), // big penalty candidate we don't want to give to leader
        Card::new(Rank::Ace, Suit::Hearts),   // penalty
        Card::new(Rank::King, Suit::Hearts),  // penalty
        Card::new(Rank::Two, Suit::Clubs),
        Card::new(Rank::Three, Suit::Clubs),
        Card::new(Rank::Four, Suit::Clubs),
        Card::new(Rank::Five, Suit::Clubs),
        Card::new(Rank::Six, Suit::Clubs),
        Card::new(Rank::Seven, Suit::Clubs),
        Card::new(Rank::Eight, Suit::Diamonds),
        Card::new(Rank::Nine, Suit::Diamonds),
        Card::new(Rank::Ten, Suit::Diamonds),
        Card::new(Rank::Jack, Suit::Diamonds),
    ];
    let round = build_round(seat, &hand, passing);
    // East is leader (lowest score in Hearts)
    let scores = build_scores([30, 20, 40, 35]);
    let target = passing.target(seat);
    println!("passing target: {:?}", target);
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

    let choice = PassPlanner::choose(round.hand(seat), &ctx).unwrap();
    println!("pass picks: {}, {}, {}", choice[0], choice[1], choice[2]);
    // Ensure QS is not passed when target is the leader (penalized by MDH_W_PASS_TO_LEADER_PENALTY)
    assert!(!choice.contains(&Card::new(Rank::Queen, Suit::Spades)));
}

#[test]
fn avoid_passing_qs_to_leader_right() {
    // Passing Right from North -> target West; make West the leader
    let seat = PlayerPosition::North;
    let passing = PassingDirection::Right;
    let hand = vec![
        Card::new(Rank::Queen, Suit::Spades),
        Card::new(Rank::Ace, Suit::Hearts),
        Card::new(Rank::King, Suit::Hearts),
        Card::new(Rank::Two, Suit::Clubs),
        Card::new(Rank::Three, Suit::Clubs),
        Card::new(Rank::Four, Suit::Clubs),
        Card::new(Rank::Five, Suit::Clubs),
        Card::new(Rank::Six, Suit::Clubs),
        Card::new(Rank::Seven, Suit::Clubs),
        Card::new(Rank::Eight, Suit::Diamonds),
        Card::new(Rank::Nine, Suit::Diamonds),
        Card::new(Rank::Ten, Suit::Diamonds),
        Card::new(Rank::Jack, Suit::Diamonds),
    ];
    let round = build_round(seat, &hand, passing);
    let scores = build_scores([30, 25, 35, 10]); // West leader
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
    assert!(
        !picks.contains(&Card::new(Rank::Queen, Suit::Spades)),
        "picks: {:?}",
        picks
    );
}

#[test]
fn avoid_passing_qs_to_leader_across() {
    // Passing Across from North -> target South; make South the leader
    let seat = PlayerPosition::North;
    let passing = PassingDirection::Across;
    let hand = vec![
        Card::new(Rank::Queen, Suit::Spades),
        Card::new(Rank::Ace, Suit::Hearts),
        Card::new(Rank::King, Suit::Hearts),
        Card::new(Rank::Two, Suit::Clubs),
        Card::new(Rank::Three, Suit::Clubs),
        Card::new(Rank::Four, Suit::Clubs),
        Card::new(Rank::Five, Suit::Clubs),
        Card::new(Rank::Six, Suit::Clubs),
        Card::new(Rank::Seven, Suit::Clubs),
        Card::new(Rank::Eight, Suit::Diamonds),
        Card::new(Rank::Nine, Suit::Diamonds),
        Card::new(Rank::Ten, Suit::Diamonds),
        Card::new(Rank::Jack, Suit::Diamonds),
    ];
    let round = build_round(seat, &hand, passing);
    let scores = build_scores([30, 25, 10, 35]); // South leader
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
    assert!(
        !picks.contains(&Card::new(Rank::Queen, Suit::Spades)),
        "picks: {:?}",
        picks
    );
}
