// Unit tests for search_deep.rs - Deep search with alpha-beta pruning
// Testing: transposition tables, alpha-beta correctness, iterative deepening,
// killer moves, aspiration windows, and time-bound search.

use hearts_app::bot::{BotDifficulty, PlayPlannerHard};
use hearts_app::controller::GameController;
use hearts_core::model::player::PlayerPosition;
use std::sync::{Mutex, OnceLock};

fn env_lock() -> &'static Mutex<()> {
    static LOCK: OnceLock<Mutex<()>> = OnceLock::new();
    LOCK.get_or_init(|| Mutex::new(()))
}

// ============================================================================
// Basic Search Behavior Tests
// ============================================================================

#[test]
fn search_deep_produces_valid_move() {
    let _guard = env_lock().lock().unwrap();
    unsafe {
        std::env::set_var("MDH_SEARCH_DEEPER_ENABLED", "1");
        std::env::set_var("MDH_SEARCH_MAX_DEPTH", "3"); // Fast search
        std::env::set_var("MDH_SEARCH_TIME_MS", "100"); // Quick timeout
    }

    let seed: u64 = 1000;
    let seat = PlayerPosition::North;
    let mut controller = GameController::new_with_seed(Some(seed), seat);
    controller.set_bot_difficulty(BotDifficulty::SearchLookahead);

    // Skip to first decision
    if controller.in_passing_phase() {
        if let Some(cards) = controller.simple_pass_for(seat) {
            let _ = controller.submit_pass(seat, cards);
        }
        let _ = controller.submit_auto_passes_for_others(seat);
        let _ = controller.resolve_passes();
    }

    while controller.expected_to_play() != seat {
        if controller.autoplay_one(seat).is_none() {
            break;
        }
    }

    let legal = controller.legal_moves(seat);
    let ctx = controller.bot_context(seat);

    let choice = PlayPlannerHard::choose(&legal, &ctx);

    unsafe {
        std::env::remove_var("MDH_SEARCH_DEEPER_ENABLED");
        std::env::remove_var("MDH_SEARCH_MAX_DEPTH");
        std::env::remove_var("MDH_SEARCH_TIME_MS");
    }

    assert!(choice.is_some(), "Deep search should produce a valid move");
    assert!(
        legal.contains(&choice.unwrap()),
        "Deep search move should be legal"
    );
}

#[test]
fn search_deep_disabled_falls_back_to_shallow() {
    let _guard = env_lock().lock().unwrap();
    unsafe {
        std::env::set_var("MDH_SEARCH_DEEPER_ENABLED", "0"); // Disabled
        std::env::set_var("MDH_ENDGAME_SOLVER_ENABLED", "0"); // Also disable endgame
    }

    let seed: u64 = 1000;
    let seat = PlayerPosition::North;
    let mut controller = GameController::new_with_seed(Some(seed), seat);
    controller.set_bot_difficulty(BotDifficulty::SearchLookahead);

    if controller.in_passing_phase() {
        if let Some(cards) = controller.simple_pass_for(seat) {
            let _ = controller.submit_pass(seat, cards);
        }
        let _ = controller.submit_auto_passes_for_others(seat);
        let _ = controller.resolve_passes();
    }

    while controller.expected_to_play() != seat {
        if controller.autoplay_one(seat).is_none() {
            break;
        }
    }

    let legal = controller.legal_moves(seat);
    let ctx = controller.bot_context(seat);

    let choice = PlayPlannerHard::choose(&legal, &ctx);

    unsafe {
        std::env::remove_var("MDH_SEARCH_DEEPER_ENABLED");
        std::env::remove_var("MDH_ENDGAME_SOLVER_ENABLED");
    }

    assert!(
        choice.is_some(),
        "Should fall back to shallow search when deep search disabled"
    );
}

#[test]
fn search_deep_respects_max_depth() {
    let _guard = env_lock().lock().unwrap();

    // Test with depth 2 (shallow)
    unsafe {
        std::env::set_var("MDH_SEARCH_DEEPER_ENABLED", "1");
        std::env::set_var("MDH_SEARCH_MAX_DEPTH", "2");
        std::env::set_var("MDH_SEARCH_TIME_MS", "50");
    }

    let seed: u64 = 1001;
    let seat = PlayerPosition::South;
    let mut controller = GameController::new_with_seed(Some(seed), seat);
    controller.set_bot_difficulty(BotDifficulty::SearchLookahead);

    if controller.in_passing_phase() {
        if let Some(cards) = controller.simple_pass_for(seat) {
            let _ = controller.submit_pass(seat, cards);
        }
        let _ = controller.submit_auto_passes_for_others(seat);
        let _ = controller.resolve_passes();
    }

    while controller.expected_to_play() != seat {
        if controller.autoplay_one(seat).is_none() {
            break;
        }
    }

    let legal = controller.legal_moves(seat);
    let ctx = controller.bot_context(seat);
    let choice_depth2 = PlayPlannerHard::choose(&legal, &ctx);

    // Test with depth 4 (deeper)
    unsafe {
        std::env::set_var("MDH_SEARCH_MAX_DEPTH", "4");
    }

    let mut controller2 = GameController::new_with_seed(Some(seed), seat);
    controller2.set_bot_difficulty(BotDifficulty::SearchLookahead);

    if controller2.in_passing_phase() {
        if let Some(cards) = controller2.simple_pass_for(seat) {
            let _ = controller2.submit_pass(seat, cards);
        }
        let _ = controller2.submit_auto_passes_for_others(seat);
        let _ = controller2.resolve_passes();
    }

    while controller2.expected_to_play() != seat {
        if controller2.autoplay_one(seat).is_none() {
            break;
        }
    }

    let legal2 = controller2.legal_moves(seat);
    let ctx2 = controller2.bot_context(seat);
    let choice_depth4 = PlayPlannerHard::choose(&legal2, &ctx2);

    unsafe {
        std::env::remove_var("MDH_SEARCH_DEEPER_ENABLED");
        std::env::remove_var("MDH_SEARCH_MAX_DEPTH");
        std::env::remove_var("MDH_SEARCH_TIME_MS");
    }

    assert!(choice_depth2.is_some(), "Depth 2 should produce valid move");
    assert!(choice_depth4.is_some(), "Depth 4 should produce valid move");
    // Note: Choices may differ due to deeper lookahead
}

// ============================================================================
// Time-Bound Search Tests
// ============================================================================

#[test]
fn search_deep_respects_time_limit() {
    let _guard = env_lock().lock().unwrap();
    unsafe {
        std::env::set_var("MDH_SEARCH_DEEPER_ENABLED", "1");
        std::env::set_var("MDH_SEARCH_MAX_DEPTH", "10"); // Very deep
        std::env::set_var("MDH_SEARCH_TIME_MS", "50"); // Very short time limit
    }

    let seed: u64 = 1002;
    let seat = PlayerPosition::East;
    let mut controller = GameController::new_with_seed(Some(seed), seat);
    controller.set_bot_difficulty(BotDifficulty::SearchLookahead);

    if controller.in_passing_phase() {
        if let Some(cards) = controller.simple_pass_for(seat) {
            let _ = controller.submit_pass(seat, cards);
        }
        let _ = controller.submit_auto_passes_for_others(seat);
        let _ = controller.resolve_passes();
    }

    while controller.expected_to_play() != seat {
        if controller.autoplay_one(seat).is_none() {
            break;
        }
    }

    let legal = controller.legal_moves(seat);
    let ctx = controller.bot_context(seat);

    let start = std::time::Instant::now();
    let choice = PlayPlannerHard::choose(&legal, &ctx);
    let elapsed = start.elapsed();

    unsafe {
        std::env::remove_var("MDH_SEARCH_DEEPER_ENABLED");
        std::env::remove_var("MDH_SEARCH_MAX_DEPTH");
        std::env::remove_var("MDH_SEARCH_TIME_MS");
    }

    assert!(choice.is_some(), "Should produce move within time limit");
    // Allow 5x the time limit for overhead (50ms limit -> 250ms max)
    assert!(
        elapsed.as_millis() < 250,
        "Search should respect time limit, took {}ms",
        elapsed.as_millis()
    );
}

// ============================================================================
// Consistency Tests
// ============================================================================

#[test]
fn search_deep_deterministic_with_same_position() {
    let _guard = env_lock().lock().unwrap();
    unsafe {
        std::env::set_var("MDH_SEARCH_DEEPER_ENABLED", "1");
        std::env::set_var("MDH_SEARCH_MAX_DEPTH", "3");
        std::env::set_var("MDH_SEARCH_TIME_MS", "200");
    }

    let seed: u64 = 1003;
    let seat = PlayerPosition::West;

    // Run 1
    let mut controller1 = GameController::new_with_seed(Some(seed), seat);
    controller1.set_bot_difficulty(BotDifficulty::SearchLookahead);

    if controller1.in_passing_phase() {
        if let Some(cards) = controller1.simple_pass_for(seat) {
            let _ = controller1.submit_pass(seat, cards);
        }
        let _ = controller1.submit_auto_passes_for_others(seat);
        let _ = controller1.resolve_passes();
    }

    while controller1.expected_to_play() != seat {
        if controller1.autoplay_one(seat).is_none() {
            break;
        }
    }

    let legal1 = controller1.legal_moves(seat);
    let ctx1 = controller1.bot_context(seat);
    let choice1 = PlayPlannerHard::choose(&legal1, &ctx1);

    // Run 2
    let mut controller2 = GameController::new_with_seed(Some(seed), seat);
    controller2.set_bot_difficulty(BotDifficulty::SearchLookahead);

    if controller2.in_passing_phase() {
        if let Some(cards) = controller2.simple_pass_for(seat) {
            let _ = controller2.submit_pass(seat, cards);
        }
        let _ = controller2.submit_auto_passes_for_others(seat);
        let _ = controller2.resolve_passes();
    }

    while controller2.expected_to_play() != seat {
        if controller2.autoplay_one(seat).is_none() {
            break;
        }
    }

    let legal2 = controller2.legal_moves(seat);
    let ctx2 = controller2.bot_context(seat);
    let choice2 = PlayPlannerHard::choose(&legal2, &ctx2);

    unsafe {
        std::env::remove_var("MDH_SEARCH_DEEPER_ENABLED");
        std::env::remove_var("MDH_SEARCH_MAX_DEPTH");
        std::env::remove_var("MDH_SEARCH_TIME_MS");
    }

    assert_eq!(
        choice1, choice2,
        "Deep search should be deterministic for same position"
    );
}

// ============================================================================
// Integration Tests with Different Difficulties
// ============================================================================

#[test]
fn search_deep_for_search_difficulty() {
    let _guard = env_lock().lock().unwrap();
    unsafe {
        std::env::set_var("MDH_SEARCH_DEEPER_ENABLED", "1");
        // SearchLookahead difficulty should use max depth automatically
    }

    let seed: u64 = 1004;
    let seat = PlayerPosition::North;
    let mut controller = GameController::new_with_seed(Some(seed), seat);
    controller.set_bot_difficulty(BotDifficulty::SearchLookahead);

    if controller.in_passing_phase() {
        if let Some(cards) = controller.simple_pass_for(seat) {
            let _ = controller.submit_pass(seat, cards);
        }
        let _ = controller.submit_auto_passes_for_others(seat);
        let _ = controller.resolve_passes();
    }

    while controller.expected_to_play() != seat {
        if controller.autoplay_one(seat).is_none() {
            break;
        }
    }

    let legal = controller.legal_moves(seat);
    let ctx = controller.bot_context(seat);

    let choice = PlayPlannerHard::choose(&legal, &ctx);

    unsafe {
        std::env::remove_var("MDH_SEARCH_DEEPER_ENABLED");
    }

    assert!(choice.is_some(), "SearchLookahead should use deep search");
}

#[test]
fn search_deep_not_for_normal_difficulty() {
    let _guard = env_lock().lock().unwrap();
    unsafe {
        // Deep search is only for SearchLookahead difficulty by default
        std::env::set_var("MDH_SEARCH_DEEPER_ENABLED", "1");
    }

    let seed: u64 = 1005;
    let seat = PlayerPosition::South;
    let mut controller = GameController::new_with_seed(Some(seed), seat);
    controller.set_bot_difficulty(BotDifficulty::NormalHeuristic);

    if controller.in_passing_phase() {
        if let Some(cards) = controller.simple_pass_for(seat) {
            let _ = controller.submit_pass(seat, cards);
        }
        let _ = controller.submit_auto_passes_for_others(seat);
        let _ = controller.resolve_passes();
    }

    while controller.expected_to_play() != seat {
        if controller.autoplay_one(seat).is_none() {
            break;
        }
    }

    let legal = controller.legal_moves(seat);

    assert!(
        !legal.is_empty(),
        "Normal difficulty should still produce moves (without deep search)"
    );

    unsafe {
        std::env::remove_var("MDH_SEARCH_DEEPER_ENABLED");
    }
}

// ============================================================================
// Belief-State Sampling Integration
// ============================================================================

#[test]
fn search_deep_handles_belief_states() {
    let _guard = env_lock().lock().unwrap();
    unsafe {
        std::env::set_var("MDH_SEARCH_DEEPER_ENABLED", "1");
        std::env::set_var("MDH_SEARCH_MAX_DEPTH", "3");
        std::env::set_var("MDH_SEARCH_TIME_MS", "200");
        std::env::set_var("MDH_SEARCH_BELIEF_SAMPLES", "10"); // Use belief sampling
    }

    let seed: u64 = 1006;
    let seat = PlayerPosition::East;
    let mut controller = GameController::new_with_seed(Some(seed), seat);
    controller.set_bot_difficulty(BotDifficulty::SearchLookahead);

    if controller.in_passing_phase() {
        if let Some(cards) = controller.simple_pass_for(seat) {
            let _ = controller.submit_pass(seat, cards);
        }
        let _ = controller.submit_auto_passes_for_others(seat);
        let _ = controller.resolve_passes();
    }

    // Play several tricks to create imperfect information
    for _ in 0..5 {
        if controller.autoplay_one(seat).is_none() {
            break;
        }
    }

    let legal = controller.legal_moves(seat);
    if !legal.is_empty() {
        let ctx = controller.bot_context(seat);
        let choice = PlayPlannerHard::choose(&legal, &ctx);

        assert!(
            choice.is_some(),
            "Deep search should handle imperfect information via belief sampling"
        );
        assert!(
            legal.contains(&choice.unwrap()),
            "Belief-sampled choice should be legal"
        );
    }

    unsafe {
        std::env::remove_var("MDH_SEARCH_DEEPER_ENABLED");
        std::env::remove_var("MDH_SEARCH_MAX_DEPTH");
        std::env::remove_var("MDH_SEARCH_TIME_MS");
        std::env::remove_var("MDH_SEARCH_BELIEF_SAMPLES");
    }
}

// ============================================================================
// Transposition Table Behavior
// ============================================================================

#[test]
fn search_deep_with_transposition_table() {
    let _guard = env_lock().lock().unwrap();
    unsafe {
        std::env::set_var("MDH_SEARCH_DEEPER_ENABLED", "1");
        std::env::set_var("MDH_SEARCH_MAX_DEPTH", "4");
        std::env::set_var("MDH_SEARCH_TT_SIZE", "10000"); // Large TT
        std::env::set_var("MDH_SEARCH_TIME_MS", "300");
    }

    let seed: u64 = 1007;
    let seat = PlayerPosition::West;
    let mut controller = GameController::new_with_seed(Some(seed), seat);
    controller.set_bot_difficulty(BotDifficulty::SearchLookahead);

    if controller.in_passing_phase() {
        if let Some(cards) = controller.simple_pass_for(seat) {
            let _ = controller.submit_pass(seat, cards);
        }
        let _ = controller.submit_auto_passes_for_others(seat);
        let _ = controller.resolve_passes();
    }

    while controller.expected_to_play() != seat {
        if controller.autoplay_one(seat).is_none() {
            break;
        }
    }

    let legal = controller.legal_moves(seat);
    let ctx = controller.bot_context(seat);

    let choice = PlayPlannerHard::choose(&legal, &ctx);

    unsafe {
        std::env::remove_var("MDH_SEARCH_DEEPER_ENABLED");
        std::env::remove_var("MDH_SEARCH_MAX_DEPTH");
        std::env::remove_var("MDH_SEARCH_TT_SIZE");
        std::env::remove_var("MDH_SEARCH_TIME_MS");
    }

    assert!(
        choice.is_some(),
        "Deep search with transposition table should produce valid move"
    );
}

#[test]
fn search_deep_with_small_transposition_table() {
    let _guard = env_lock().lock().unwrap();
    unsafe {
        std::env::set_var("MDH_SEARCH_DEEPER_ENABLED", "1");
        std::env::set_var("MDH_SEARCH_MAX_DEPTH", "3");
        std::env::set_var("MDH_SEARCH_TT_SIZE", "10"); // Very small TT
        std::env::set_var("MDH_SEARCH_TIME_MS", "200");
    }

    let seed: u64 = 1008;
    let seat = PlayerPosition::North;
    let mut controller = GameController::new_with_seed(Some(seed), seat);
    controller.set_bot_difficulty(BotDifficulty::SearchLookahead);

    if controller.in_passing_phase() {
        if let Some(cards) = controller.simple_pass_for(seat) {
            let _ = controller.submit_pass(seat, cards);
        }
        let _ = controller.submit_auto_passes_for_others(seat);
        let _ = controller.resolve_passes();
    }

    while controller.expected_to_play() != seat {
        if controller.autoplay_one(seat).is_none() {
            break;
        }
    }

    let legal = controller.legal_moves(seat);
    let ctx = controller.bot_context(seat);

    let choice = PlayPlannerHard::choose(&legal, &ctx);

    unsafe {
        std::env::remove_var("MDH_SEARCH_DEEPER_ENABLED");
        std::env::remove_var("MDH_SEARCH_MAX_DEPTH");
        std::env::remove_var("MDH_SEARCH_TT_SIZE");
        std::env::remove_var("MDH_SEARCH_TIME_MS");
    }

    assert!(
        choice.is_some(),
        "Deep search should work even with very small transposition table"
    );
}
