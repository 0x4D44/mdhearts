use hearts_app::bot::{BotContext, BotDifficulty, PlayPlanner, PlayPlannerHard, UnseenTracker};
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
    for (idx, cards) in hands_vec.into_iter().enumerate() {
        hands[idx] = Hand::with_cards(cards);
    }
    let mut prev = hearts_core::model::trick::Trick::new(starting);
    let mut seat = starting;
    for card in [
        Card::new(Rank::Two, Suit::Clubs),
        Card::new(Rank::Three, Suit::Clubs),
        Card::new(Rank::Four, Suit::Clubs),
        Card::new(Rank::Five, Suit::Clubs),
    ] {
        prev.play(seat, card).unwrap();
        seat = seat.next();
    }
    RoundState::from_hands_with_state(
        hands,
        starting,
        PassingDirection::Hold,
        RoundPhase::Playing,
        hearts_core::model::trick::Trick::new(starting),
        vec![prev],
        false,
    )
}

fn empty_scores() -> ScoreBoard {
    ScoreBoard::new()
}

#[test]
fn hard_phaseb_topk_preserves_base_for_non_probed() {
    unsafe {
        std::env::set_var("MDH_HARD_DETERMINISTIC", "1");
        std::env::set_var("MDH_HARD_TEST_STEPS", "200");
        std::env::set_var("MDH_HARD_PHASEB_TOPK", "1");
        std::env::set_var("MDH_HARD_BRANCH_LIMIT", "4");
    }

    let starting = PlayerPosition::West;
    // West has 4 legal different-suit cards to give a clear base ordering from heuristic
    let west = vec![
        Card::new(Rank::Two, Suit::Spades),
        Card::new(Rank::Three, Suit::Hearts),
        Card::new(Rank::Four, Suit::Diamonds),
        Card::new(Rank::Five, Suit::Clubs),
    ];
    let north = vec![Card::new(Rank::Six, Suit::Spades)];
    let east = vec![Card::new(Rank::Seven, Suit::Hearts)];
    let south = vec![Card::new(Rank::Eight, Suit::Diamonds)];

    let round = build_round_not_first_trick(starting, [north, east, south, west.clone()]);
    let scores = empty_scores();
    let mut tracker = UnseenTracker::new();
    tracker.reset_for_round(&round);
    let ctx = BotContext::new(
        starting,
        &round,
        &scores,
        PassingDirection::Hold,
        &tracker,
        BotDifficulty::FutureHard,
    );
    let legal = {
        round
            .hand(starting)
            .iter()
            .copied()
            .filter(|card| {
                let mut probe = round.clone();
                probe.play_card(starting, *card).is_ok()
            })
            .collect::<Vec<_>>()
    };
    assert!(legal.len() >= 2);

    // Get base scores from Normal planner (same heuristic function used to sort candidates)
    let base = PlayPlanner::explain_candidates(&legal, &ctx);
    let mut base_map = std::collections::HashMap::new();
    for (c, b) in base.into_iter() {
        base_map.insert(c, b);
    }

    let explained = PlayPlannerHard::explain_candidates(&legal, &ctx);
    // explained is in base-descending order; with topK=1, only first item gets continuation, others should have total==base
    for (idx, (c, total)) in explained.iter().enumerate() {
        let b = *base_map.get(c).expect("base present");
        if idx >= 1 {
            assert_eq!(*total, b, "candidate beyond topK should remain base-only");
        }
    }

    unsafe {
        std::env::remove_var("MDH_HARD_DETERMINISTIC");
        std::env::remove_var("MDH_HARD_TEST_STEPS");
        std::env::remove_var("MDH_HARD_PHASEB_TOPK");
        std::env::remove_var("MDH_HARD_BRANCH_LIMIT");
    }
}
