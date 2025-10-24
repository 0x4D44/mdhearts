use hearts_app::bot::{BotContext, BotDifficulty, PlayPlannerHard, UnseenTracker};
use hearts_app::bot::search;
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
    for c in [
        Card::new(Rank::Two, Suit::Clubs),
        Card::new(Rank::Three, Suit::Clubs),
        Card::new(Rank::Four, Suit::Clubs),
        Card::new(Rank::Five, Suit::Clubs),
    ] { prev.play(seat, c).unwrap(); seat = seat.next(); }
    RoundState::from_hands_with_state(
        hands, starting, PassingDirection::Hold, RoundPhase::Playing,
        hearts_core::model::trick::Trick::new(starting), vec![prev], true,
    )
}

fn empty_scores() -> ScoreBoard { ScoreBoard::new() }

#[test]
fn hard_budget_step_cap_reduces_scanned_candidates() {
    let starting = PlayerPosition::East;
    let east = vec![
        Card::new(Rank::Two, Suit::Clubs),
        Card::new(Rank::Three, Suit::Clubs),
        Card::new(Rank::Four, Suit::Clubs),
        Card::new(Rank::Five, Suit::Clubs),
        Card::new(Rank::Six, Suit::Clubs),
    ];
    let north = vec![Card::new(Rank::Ace, Suit::Clubs)];
    let west = vec![Card::new(Rank::King, Suit::Clubs)];
    let south = vec![Card::new(Rank::Queen, Suit::Clubs)];
    let round = build_round_not_first_trick(starting, [north, east.clone(), south, west]);
    let mut tracker = UnseenTracker::new();
    tracker.reset_for_round(&round);
    let scores = empty_scores();
    let ctx = BotContext::new(starting, &round, &scores, PassingDirection::Hold, &tracker, BotDifficulty::FutureHard);
    let legal = {
        round.hand(starting).iter().copied().filter(|c| {
            let mut p = round.clone(); p.play_card(starting, *c).is_ok()
        }).collect::<Vec<_>>()
    };
    // Tight step cap
    unsafe { std::env::set_var("MDH_HARD_DETERMINISTIC", "1"); std::env::set_var("MDH_HARD_TEST_STEPS", "5"); }
    let _ = PlayPlannerHard::explain_candidates(&legal, &ctx);
    let stats_small = search::last_stats().unwrap();
    // Larger step cap
    unsafe { std::env::set_var("MDH_HARD_TEST_STEPS", "100"); }
    let _ = PlayPlannerHard::explain_candidates(&legal, &ctx);
    let stats_large = search::last_stats().unwrap();
    unsafe { std::env::remove_var("MDH_HARD_DETERMINISTIC"); std::env::remove_var("MDH_HARD_TEST_STEPS"); }
    assert!(stats_small.scanned <= stats_large.scanned, "expected fewer/equal scanned with tight step cap: small={} large={}", stats_small.scanned, stats_large.scanned);
}







