use hearts_app::bot::{BotContext, BotDifficulty, PlayPlannerHard, UnseenTracker};
use hearts_core::model::card::Card;
use hearts_core::model::hand::Hand;
use hearts_core::model::passing::PassingDirection;
use hearts_core::model::player::PlayerPosition;
use hearts_core::model::rank::Rank;
use hearts_core::model::round::{RoundPhase, RoundState};
use hearts_core::model::score::ScoreBoard;
use hearts_core::model::suit::Suit;

fn build_round_not_first_trick(starting: PlayerPosition, hands_vec: [Vec<Card>; 4]) -> RoundState {
    let mut hands = [Hand::new(), Hand::new(), Hand::new(), Hand::new()];
    for (idx, cards) in hands_vec.into_iter().enumerate() { hands[idx] = Hand::with_cards(cards); }
    let mut prev = hearts_core::model::trick::Trick::new(starting);
    let mut seat = starting;
    for card in [
        Card::new(Rank::Two, Suit::Clubs),
        Card::new(Rank::Three, Suit::Clubs),
        Card::new(Rank::Four, Suit::Clubs),
        Card::new(Rank::Five, Suit::Clubs),
    ] { prev.play(seat, card).unwrap(); seat = seat.next(); }
    RoundState::from_hands_with_state(
        hands, starting, PassingDirection::Hold, RoundPhase::Playing,
        hearts_core::model::trick::Trick::new(starting), vec![prev], false,
    )
}

fn empty_scores() -> ScoreBoard { ScoreBoard::new() }

#[test]
fn hard_budget_deterministic_explain_and_choose_are_stable() {
    unsafe {
        std::env::set_var("MDH_HARD_DETERMINISTIC", "1");
        std::env::set_var("MDH_HARD_TEST_STEPS", "50");
    }

    let starting = PlayerPosition::East;
    let east = vec![
        Card::new(Rank::Two, Suit::Clubs),
        Card::new(Rank::Three, Suit::Diamonds),
        Card::new(Rank::Four, Suit::Spades),
        Card::new(Rank::Five, Suit::Hearts),
        Card::new(Rank::Six, Suit::Hearts),
    ];
    let north = vec![Card::new(Rank::Ten, Suit::Clubs)];
    let west = vec![Card::new(Rank::Queen, Suit::Diamonds)];
    let south = vec![Card::new(Rank::King, Suit::Spades)];

    let round = build_round_not_first_trick(starting, [north, east.clone(), south, west]);
    let mut tracker = UnseenTracker::new();
    tracker.reset_for_round(&round);
    let scores = empty_scores();
    let ctx = BotContext::new(starting, &round, &scores, PassingDirection::Hold, &tracker, BotDifficulty::FutureHard);
    let legal = {
        round.hand(starting).iter().copied().filter(|card| {
            let mut probe = round.clone(); probe.play_card(starting, *card).is_ok()
        }).collect::<Vec<_>>()
    };
    assert!(!legal.is_empty());

    let exp1 = PlayPlannerHard::explain_candidates(&legal, &ctx);
    let exp2 = PlayPlannerHard::explain_candidates(&legal, &ctx);
    assert_eq!(exp1, exp2, "deterministic explains should match");

    let ch1 = PlayPlannerHard::choose(&legal, &ctx);
    let ch2 = PlayPlannerHard::choose(&legal, &ctx);
    assert_eq!(ch1, ch2, "deterministic choose should match");

    // Cleanup env
    unsafe {
        std::env::remove_var("MDH_HARD_DETERMINISTIC");
        std::env::remove_var("MDH_HARD_TEST_STEPS");
    }
}

