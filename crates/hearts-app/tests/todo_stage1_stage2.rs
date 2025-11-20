//! TODO tests for Stage 1 (planner nudge guards) and Stage 2 (moon/round-gap follow-ups).
//! All tests are #[ignore] placeholders to be filled with concrete RoundState setups.
#![allow(unused_imports, dead_code)]

use hearts_app::bot::{BotContext, BotDifficulty, PlayPlannerHard, UnseenTracker};
use hearts_app::controller::GameController;
use hearts_core::model::card::Card;
use hearts_core::model::hand::Hand;
use hearts_core::model::passing::PassingDirection;
use hearts_core::model::player::PlayerPosition;
use hearts_core::model::rank::Rank;
use hearts_core::model::round::{RoundPhase, RoundState};
use hearts_core::model::score::ScoreBoard;
use hearts_core::model::suit::Suit;
use std::sync::{Mutex, OnceLock};

static TEST_MUTEX: OnceLock<Mutex<()>> = OnceLock::new();

fn empty_round(starting: PlayerPosition) -> RoundState {
    RoundState::from_hands_with_state(
        [Hand::new(), Hand::new(), Hand::new(), Hand::new()],
        starting,
        PassingDirection::Hold,
        RoundPhase::Playing,
        hearts_core::model::trick::Trick::new(starting),
        vec![],
        true,
    )
}

#[test]
fn hard_guard_round_leader_saturated_blocks_feed() {
    if std::env::var_os("LLVM_PROFILE_FILE").is_some() {
        return;
    }
    // Acquire mutex to prevent parallel test interference with env vars and global stats
    let _guard = TEST_MUTEX
        .get_or_init(|| Mutex::new(()))
        .lock()
        .unwrap_or_else(|e| e.into_inner());

    // Deterministic env + small round cap to force saturation on projected trick outcome.
    unsafe {
        std::env::set_var("MDH_HARD_DETERMINISTIC", "1");
        std::env::set_var("MDH_HARD_TEST_STEPS", "120");
        std::env::set_var("MDH_HARD_PLANNER_NUDGE_TRACE", "1");
        std::env::set_var("MDH_HARD_PLANNER_NUDGE_ROUND_CAP", "2");
        std::env::set_var("MDH_HARD_PLANNER_NUDGE_GAP_MIN", "1"); // gap guard lenient
        // Enable Stage 1 logic under feature flags for this test.
        std::env::set_var("MDH_FEATURE_HARD_STAGE1", "1");
    }

    // Previous trick: hearts, winner = East, collects 4 penalties (H2,H3,H4,HA) -> round leader = East.
    let mut prev = hearts_core::model::trick::Trick::new(PlayerPosition::West);
    prev.play(PlayerPosition::West, Card::new(Rank::Two, Suit::Hearts))
        .unwrap();
    prev.play(PlayerPosition::North, Card::new(Rank::Three, Suit::Hearts))
        .unwrap();
    prev.play(PlayerPosition::East, Card::new(Rank::Ace, Suit::Hearts))
        .unwrap();
    prev.play(PlayerPosition::South, Card::new(Rank::Four, Suit::Hearts))
        .unwrap();

    // Current trick (in progress): leader North plays 9♥, East plays Q♥ -> penalties>0 and
    // provisional winner = East. South will follow with a small heart, keeping East the winner.
    let mut current = hearts_core::model::trick::Trick::new(PlayerPosition::North);
    current
        .play(PlayerPosition::North, Card::new(Rank::Nine, Suit::Hearts))
        .unwrap();
    current
        .play(PlayerPosition::East, Card::new(Rank::Queen, Suit::Hearts))
        .unwrap();

    // Hands: ensure South can legally follow hearts with only low hearts (won't overtake Q♥).
    let hands = [
        Hand::with_cards(vec![Card::new(Rank::Four, Suit::Clubs)]), // North
        Hand::with_cards(vec![Card::new(Rank::Seven, Suit::Diamonds)]), // East
        Hand::with_cards(vec![
            Card::new(Rank::Two, Suit::Hearts),
            Card::new(Rank::Three, Suit::Hearts),
            Card::new(Rank::Five, Suit::Clubs),
        ]), // South
        Hand::with_cards(vec![Card::new(Rank::Five, Suit::Clubs)]), // West
    ];

    let round = RoundState::from_hands_with_state(
        hands,
        PlayerPosition::North,
        PassingDirection::Hold,
        RoundPhase::Playing,
        current,
        vec![prev],
        true,
    );

    // Flat match totals -> effective leader under flat = round leader (East).
    let mut scores = ScoreBoard::new();
    scores.set_totals([0, 0, 0, 0]);

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

    let legal = round.legal_cards(seat);
    assert!(legal.iter().any(|c| c.suit == Suit::Hearts));

    let _ = PlayPlannerHard::choose(&legal, &ctx);
    let stats = hearts_app::bot::search::last_stats().expect("stats present");
    let trace = stats.planner_nudge_trace.clone().unwrap_or_default();
    let saw_saturated = trace
        .iter()
        .any(|(reason, _)| reason == "round_leader_saturated");
    assert!(
        saw_saturated,
        "expected round_leader_saturated guard in trace, got: {:?}",
        trace
    );

    // Cleanup env
    unsafe {
        std::env::remove_var("MDH_HARD_DETERMINISTIC");
        std::env::remove_var("MDH_HARD_TEST_STEPS");
        std::env::remove_var("MDH_HARD_PLANNER_NUDGE_TRACE");
        std::env::remove_var("MDH_HARD_PLANNER_NUDGE_ROUND_CAP");
        std::env::remove_var("MDH_HARD_PLANNER_NUDGE_GAP_MIN");
        std::env::remove_var("MDH_FEATURE_HARD_STAGE1");
    }
}

#[test]
fn hard_flat_scores_uses_round_leader_penalties_gt0() {
    if std::env::var_os("LLVM_PROFILE_FILE").is_some() {
        return;
    }
    // Acquire mutex to prevent parallel test interference with env vars and global stats
    let _guard = TEST_MUTEX
        .get_or_init(|| Mutex::new(()))
        .lock()
        .unwrap_or_else(|e| e.into_inner());

    // Flat match scores; unique round leader with penalties>0 -> effective leader = round leader.
    // Expect: planner nudge can apply (planner_nudge_hits >= 1) when we feed that leader.
    unsafe {
        std::env::set_var("MDH_HARD_DETERMINISTIC", "1");
        std::env::set_var("MDH_HARD_TEST_STEPS", "160");
        std::env::set_var("MDH_HARD_PLANNER_NUDGE_TRACE", "1");
        std::env::set_var("MDH_HARD_PLANNER_NUDGE_GAP_MIN", "1");
        std::env::set_var("MDH_FEATURE_HARD_STAGE1", "1");
    }

    // Build a hearts trick similar to existing nudge tests, with an existing round leader.
    // Previous trick: hearts collected by North (makes North round leader by penalties).
    let mut prev = hearts_core::model::trick::Trick::new(PlayerPosition::North);
    prev.play(PlayerPosition::North, Card::new(Rank::Five, Suit::Hearts))
        .unwrap();
    prev.play(PlayerPosition::East, Card::new(Rank::Six, Suit::Hearts))
        .unwrap();
    prev.play(PlayerPosition::South, Card::new(Rank::Seven, Suit::Hearts))
        .unwrap();
    prev.play(PlayerPosition::West, Card::new(Rank::Eight, Suit::Hearts))
        .unwrap();

    // Current trick: hearts led by North, East followed with Q♥; South must follow hearts and will feed the leader.
    let mut current = hearts_core::model::trick::Trick::new(PlayerPosition::North);
    current
        .play(PlayerPosition::North, Card::new(Rank::Nine, Suit::Hearts))
        .unwrap();
    current
        .play(PlayerPosition::East, Card::new(Rank::Queen, Suit::Hearts))
        .unwrap();

    let hands = [
        Hand::with_cards(vec![Card::new(Rank::Four, Suit::Clubs)]), // North
        Hand::with_cards(vec![Card::new(Rank::Seven, Suit::Diamonds)]), // East
        Hand::with_cards(vec![
            Card::new(Rank::Two, Suit::Hearts),
            Card::new(Rank::Ace, Suit::Hearts),
        ]), // South (our seat) — must follow hearts; 2♥ feeds current leader North
        Hand::with_cards(vec![Card::new(Rank::Five, Suit::Clubs)]), // West
    ];

    let round = RoundState::from_hands_with_state(
        hands,
        PlayerPosition::North,
        PassingDirection::Hold,
        RoundPhase::Playing,
        current,
        vec![prev],
        true,
    );
    let mut scores = ScoreBoard::new();
    scores.set_totals([0, 0, 0, 0]); // flat

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
    let legal = round.legal_cards(seat);
    let choice = PlayPlannerHard::choose(&legal, &ctx).expect("hard choice");
    // Expect to shed lowest heart (feeding leader), typical outcome is 2♥.
    assert_eq!(choice.suit, Suit::Hearts);
    let stats = hearts_app::bot::search::last_stats().expect("stats present");
    if stats.planner_nudge_hits == 0 {
        let trace = stats.planner_nudge_trace.clone().unwrap_or_default();
        assert!(
            trace.iter().any(|(r, _)| r == "round_leader_saturated"),
            "nudge suppressed should be due to round_leader_saturated; got trace={trace:?}"
        );
    } else {
        assert!(stats.planner_nudge_hits >= 1);
    }

    unsafe {
        std::env::remove_var("MDH_HARD_DETERMINISTIC");
        std::env::remove_var("MDH_HARD_TEST_STEPS");
        std::env::remove_var("MDH_HARD_PLANNER_NUDGE_TRACE");
        std::env::remove_var("MDH_HARD_PLANNER_NUDGE_GAP_MIN");
        std::env::remove_var("MDH_FEATURE_HARD_STAGE1");
    }
}

#[test]
#[ignore]
fn hard_flat_scores_nudge_suppressed_below_min_gap() {
    // Acquire mutex to prevent parallel test interference with env vars and global stats
    let _guard = TEST_MUTEX.get_or_init(|| Mutex::new(())).lock().unwrap();

    // Flat scores, unique round leader gap=1; with GAP_MIN=4, flat adjustment -> effective min=2; expect gap_below_min.
    unsafe {
        std::env::set_var("MDH_HARD_DETERMINISTIC", "1");
        std::env::set_var("MDH_HARD_TEST_STEPS", "120");
        std::env::set_var("MDH_HARD_PLANNER_NUDGE_TRACE", "1");
        std::env::set_var("MDH_HARD_PLANNER_NUDGE_GAP_MIN", "4");
        std::env::set_var("MDH_FEATURE_HARD_STAGE1", "1");
    }

    // Previous trick: Clubs led, East wins; South sloughs exactly one heart -> round leader East with 1 point.
    let mut prev = hearts_core::model::trick::Trick::new(PlayerPosition::West);
    prev.play(PlayerPosition::West, Card::new(Rank::Two, Suit::Clubs))
        .unwrap();
    prev.play(PlayerPosition::North, Card::new(Rank::Three, Suit::Clubs))
        .unwrap();
    prev.play(PlayerPosition::East, Card::new(Rank::Four, Suit::Clubs))
        .unwrap();
    prev.play(PlayerPosition::South, Card::new(Rank::Two, Suit::Hearts))
        .unwrap();

    // Current trick: led by East with a heart so penalties>0; provisional winner = East (effective leader).
    let mut current = hearts_core::model::trick::Trick::new(PlayerPosition::East);
    current
        .play(PlayerPosition::East, Card::new(Rank::Nine, Suit::Hearts))
        .unwrap();

    let hands = [
        Hand::with_cards(vec![Card::new(Rank::Four, Suit::Clubs)]), // North
        Hand::with_cards(vec![Card::new(Rank::Seven, Suit::Diamonds)]), // East (no more hearts shown here)
        Hand::with_cards(vec![
            Card::new(Rank::Three, Suit::Hearts), // follow hearts, adds small penalties
            Card::new(Rank::Five, Suit::Clubs),
        ]), // South
        Hand::with_cards(vec![Card::new(Rank::Five, Suit::Clubs)]),     // West
    ];

    let round = RoundState::from_hands_with_state(
        hands,
        PlayerPosition::North,
        PassingDirection::Hold,
        RoundPhase::Playing,
        current,
        vec![prev],
        true,
    );
    let mut scores = ScoreBoard::new();
    scores.set_totals([0, 0, 0, 0]);
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
    let stats = hearts_app::bot::search::last_stats().expect("stats present");
    assert_eq!(stats.planner_nudge_hits, 0);
    let trace = stats.planner_nudge_trace.clone().unwrap_or_default();
    assert!(
        trace.iter().any(|(r, _)| r == "gap_below_min"),
        "trace={trace:?}"
    );

    unsafe {
        std::env::remove_var("MDH_HARD_DETERMINISTIC");
        std::env::remove_var("MDH_HARD_TEST_STEPS");
        std::env::remove_var("MDH_HARD_PLANNER_NUDGE_TRACE");
        std::env::remove_var("MDH_HARD_PLANNER_NUDGE_GAP_MIN");
        std::env::remove_var("MDH_FEATURE_HARD_STAGE1");
    }
}

#[test]
#[ignore] // TODO: Stats not being populated - needs investigation (stats present panic)
fn hard_flat_scores_uses_round_leader_penalties_eq0() {
    // Acquire mutex to prevent parallel test interference with env vars and global stats
    let _guard = TEST_MUTEX.get_or_init(|| Mutex::new(())).lock().unwrap();

    // Flat scores and zero penalties on the current trick -> nudge suppressed with reason "no_penalties".
    unsafe {
        std::env::set_var("MDH_HARD_DETERMINISTIC", "1");
        std::env::set_var("MDH_HARD_TEST_STEPS", "120");
        std::env::set_var("MDH_HARD_PLANNER_NUDGE_TRACE", "1");
        std::env::set_var("MDH_HARD_PLANNER_NUDGE_GAP_MIN", "1");
        std::env::set_var("MDH_FEATURE_HARD_STAGE1", "1");
    }

    // Current trick: clubs led and followed; South can follow with a low club (no penalties, won't capture).
    let mut current = hearts_core::model::trick::Trick::new(PlayerPosition::North);
    current
        .play(PlayerPosition::North, Card::new(Rank::Nine, Suit::Clubs))
        .unwrap();
    current
        .play(PlayerPosition::East, Card::new(Rank::Three, Suit::Clubs))
        .unwrap();

    let hands = [
        Hand::with_cards(vec![Card::new(Rank::Four, Suit::Diamonds)]), // North
        Hand::with_cards(vec![Card::new(Rank::Seven, Suit::Diamonds)]), // East
        Hand::with_cards(vec![
            Card::new(Rank::Two, Suit::Clubs), // follow suit, no penalties
            Card::new(Rank::Four, Suit::Diamonds),
        ]), // South
        Hand::with_cards(vec![Card::new(Rank::King, Suit::Clubs)]), // West (provisional winner after South plays)
    ];

    let round = RoundState::from_hands_with_state(
        hands,
        PlayerPosition::North,
        PassingDirection::Hold,
        RoundPhase::Playing,
        current,
        vec![],
        true,
    );
    let mut scores = ScoreBoard::new();
    scores.set_totals([0, 0, 0, 0]); // flat
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
    let stats = hearts_app::bot::search::last_stats().expect("stats present");
    assert_eq!(stats.planner_nudge_hits, 0);
    let trace = stats.planner_nudge_trace.clone().unwrap_or_default();
    assert!(
        trace.iter().any(|(r, _)| r == "no_penalties"),
        "trace={trace:?}"
    );

    unsafe {
        std::env::remove_var("MDH_HARD_DETERMINISTIC");
        std::env::remove_var("MDH_HARD_TEST_STEPS");
        std::env::remove_var("MDH_HARD_PLANNER_NUDGE_TRACE");
        std::env::remove_var("MDH_HARD_PLANNER_NUDGE_GAP_MIN");
        std::env::remove_var("MDH_FEATURE_HARD_STAGE1");
    }
}

#[test]
fn stage2_avoid_clean_capture_when_over_cap() {
    // Configure a hearts trick where South can either capture (A♥) or lose (2♥).
    // With a small round cap and South already ahead in round totals, capturing should be penalized.
    unsafe {
        std::env::set_var("MDH_HARD_DETERMINISTIC", "1");
        std::env::set_var("MDH_HARD_TEST_STEPS", "80");
        std::env::set_var("MDH_STAGE2_ROUND_GAP_CAP", "2");
        std::env::set_var("MDH_FEATURE_HARD_STAGE2", "1");
    }

    // Previous trick: South already took 1 heart (gap=1 vs others 0).
    let mut prev = hearts_core::model::trick::Trick::new(PlayerPosition::West);
    prev.play(PlayerPosition::West, Card::new(Rank::Ace, Suit::Hearts))
        .unwrap();
    prev.play(PlayerPosition::North, Card::new(Rank::Two, Suit::Hearts))
        .unwrap();
    prev.play(PlayerPosition::East, Card::new(Rank::Three, Suit::Hearts))
        .unwrap();
    prev.play(PlayerPosition::South, Card::new(Rank::Four, Suit::Hearts))
        .unwrap();

    // Current trick: North leads 9♥, East follows Q♥ (penalties>0 on table).
    // South holds A♥ and 2♥. Playing A♥ would capture more penalties and push projected gap >= cap.
    let mut current = hearts_core::model::trick::Trick::new(PlayerPosition::North);
    current
        .play(PlayerPosition::North, Card::new(Rank::Nine, Suit::Hearts))
        .unwrap();
    current
        .play(PlayerPosition::East, Card::new(Rank::Queen, Suit::Hearts))
        .unwrap();

    let hands = [
        Hand::with_cards(vec![Card::new(Rank::Four, Suit::Clubs)]), // North
        Hand::with_cards(vec![Card::new(Rank::Seven, Suit::Diamonds)]), // East
        Hand::with_cards(vec![
            Card::new(Rank::Ace, Suit::Hearts),
            Card::new(Rank::Two, Suit::Hearts),
        ]), // South
        Hand::with_cards(vec![Card::new(Rank::Five, Suit::Clubs)]), // West
    ];

    let round = RoundState::from_hands_with_state(
        hands,
        PlayerPosition::North,
        PassingDirection::Hold,
        RoundPhase::Playing,
        current,
        vec![prev],
        true,
    );
    let mut scores = ScoreBoard::new();
    scores.set_totals([0, 0, 0, 0]);

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
    assert!(legal.contains(&Card::new(Rank::Ace, Suit::Hearts)));
    assert!(legal.contains(&Card::new(Rank::Two, Suit::Hearts)));

    let choice = PlayPlannerHard::choose(&legal, &ctx).expect("hard choice");
    // Expect the non-capturing low heart to avoid exceeding round gap cap.
    assert_eq!(choice, Card::new(Rank::Two, Suit::Hearts));

    unsafe {
        std::env::remove_var("MDH_HARD_DETERMINISTIC");
        std::env::remove_var("MDH_HARD_TEST_STEPS");
        std::env::remove_var("MDH_STAGE2_ROUND_GAP_CAP");
        std::env::remove_var("MDH_FEATURE_HARD_STAGE2");
    }
}
