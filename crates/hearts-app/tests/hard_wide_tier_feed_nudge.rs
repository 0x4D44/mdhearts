use hearts_app::bot::search::last_stats;
use hearts_app::bot::{BotContext, BotDifficulty, PlayPlannerHard, UnseenTracker};
use hearts_core::model::card::Card;
use hearts_core::model::hand::Hand;
use hearts_core::model::passing::PassingDirection;
use hearts_core::model::player::PlayerPosition;
use hearts_core::model::rank::Rank;
use hearts_core::model::round::{RoundPhase, RoundState};
use hearts_core::model::score::ScoreBoard;
use hearts_core::model::suit::Suit;

fn build_round_with_scores(scores: [u32; 4]) -> (RoundState, ScoreBoard) {
    let leader = PlayerPosition::North;

    let mut hands = [Hand::new(), Hand::new(), Hand::new(), Hand::new()];
    hands[PlayerPosition::North.index()] = Hand::with_cards(vec![
        Card::new(Rank::Four, Suit::Clubs),
        Card::new(Rank::Nine, Suit::Spades),
    ]);
    hands[PlayerPosition::East.index()] = Hand::with_cards(vec![
        Card::new(Rank::Six, Suit::Hearts),
        Card::new(Rank::Seven, Suit::Diamonds),
    ]);
    hands[PlayerPosition::South.index()] = Hand::with_cards(vec![
        Card::new(Rank::Ace, Suit::Hearts),
        Card::new(Rank::Two, Suit::Hearts),
        Card::new(Rank::Three, Suit::Clubs),
    ]);
    hands[PlayerPosition::West.index()] = Hand::with_cards(vec![
        Card::new(Rank::Four, Suit::Hearts),
        Card::new(Rank::Five, Suit::Clubs),
    ]);

    let mut current = hearts_core::model::trick::Trick::new(leader);
    current
        .play(leader, Card::new(Rank::Nine, Suit::Hearts))
        .unwrap();
    current
        .play(leader.next(), Card::new(Rank::Queen, Suit::Hearts))
        .unwrap();

    let mut prev = hearts_core::model::trick::Trick::new(leader);
    prev.play(leader, Card::new(Rank::Two, Suit::Clubs))
        .unwrap();
    prev.play(leader.next(), Card::new(Rank::Three, Suit::Clubs))
        .unwrap();
    prev.play(leader.next().next(), Card::new(Rank::Four, Suit::Clubs))
        .unwrap();
    prev.play(
        leader.next().next().next(),
        Card::new(Rank::Five, Suit::Clubs),
    )
    .unwrap();

    let round = RoundState::from_hands_with_state(
        hands,
        leader,
        PassingDirection::Hold,
        RoundPhase::Playing,
        current,
        vec![prev],
        true, // hearts are already broken
    );

    let mut scoreboard = ScoreBoard::new();
    scoreboard.set_totals(scores);
    (round, scoreboard)
}

fn set_stage1_env() {
    unsafe {
        std::env::set_var("MDH_HARD_DETERMINISTIC", "1");
        std::env::set_var("MDH_HARD_TEST_STEPS", "160");
        std::env::set_var("MDH_HARD_PLANNER_LEADER_FEED_NUDGE", "40");
        std::env::set_var("MDH_HARD_PLANNER_MAX_BASE_FOR_NUDGE", "320");
        std::env::set_var("MDH_HARD_PLANNER_NUDGE_NEAR100", "95");
        std::env::set_var("MDH_HARD_PLANNER_NUDGE_GAP_MIN", "4");
    }
}

fn clear_stage1_env() {
    unsafe {
        std::env::remove_var("MDH_HARD_DETERMINISTIC");
        std::env::remove_var("MDH_HARD_TEST_STEPS");
        std::env::remove_var("MDH_HARD_PLANNER_LEADER_FEED_NUDGE");
        std::env::remove_var("MDH_HARD_PLANNER_MAX_BASE_FOR_NUDGE");
        std::env::remove_var("MDH_HARD_PLANNER_NUDGE_NEAR100");
        std::env::remove_var("MDH_HARD_PLANNER_NUDGE_GAP_MIN");
    }
}

#[test]
fn hard_nudge_prefers_feeding_unique_leader() {
    set_stage1_env();
    let (round, scores) = build_round_with_scores([48, 92, 63, 55]);
    let mut tracker = UnseenTracker::new();
    tracker.reset_for_round(&round);
    let seat = PlayerPosition::South;
    let ctx = BotContext::new(
        seat,
        &round,
        &scores,
        PassingDirection::Hold,
        &tracker,
        BotDifficulty::FutureHard,
    );
    let legal = round
        .hand(seat)
        .iter()
        .copied()
        .filter(|card| {
            let mut probe = round.clone();
            probe.play_card(seat, *card).is_ok()
        })
        .collect::<Vec<_>>();
    let choice = PlayPlannerHard::choose(&legal, &ctx).expect("hard choice");
    clear_stage1_env();
    assert_eq!(choice, Card::new(Rank::Two, Suit::Hearts));
    let stats = last_stats().expect("stats present after hard choose");
    assert!(stats.planner_nudge_hits >= 1, "expected nudge to fire");
}

#[test]
fn hard_nudge_skips_when_leader_ambiguous() {
    set_stage1_env();
    let (round, scores) = build_round_with_scores([48, 92, 63, 92]);
    let mut tracker = UnseenTracker::new();
    tracker.reset_for_round(&round);
    let seat = PlayerPosition::South;
    let ctx = BotContext::new(
        seat,
        &round,
        &scores,
        PassingDirection::Hold,
        &tracker,
        BotDifficulty::FutureHard,
    );
    let legal = round
        .hand(seat)
        .iter()
        .copied()
        .filter(|card| {
            let mut probe = round.clone();
            probe.play_card(seat, *card).is_ok()
        })
        .collect::<Vec<_>>();
    let _ = PlayPlannerHard::choose(&legal, &ctx);
    clear_stage1_env();
    let stats = last_stats().expect("stats present after hard choose");
    assert_eq!(
        stats.planner_nudge_hits, 0,
        "ambiguous leader should suppress planner nudge"
    );
}
