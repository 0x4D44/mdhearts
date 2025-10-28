use hearts_app::controller::GameController;
use hearts_core::model::player::PlayerPosition;

// Additional deterministic disagreement (from quick scans): 1146 South
#[test]
fn hard_vs_normal_disagree_seed_1146_south() {
    unsafe {
        std::env::set_var("MDH_HARD_DETERMINISTIC", "1");
        std::env::set_var("MDH_HARD_TEST_STEPS", "80");
    }

    let seed: u64 = 1146;
    let seat = PlayerPosition::South;

    // Normal
    let mut normal = GameController::new_with_seed(Some(seed), PlayerPosition::North);
    normal.set_bot_difficulty(hearts_app::bot::BotDifficulty::NormalHeuristic);
    if normal.in_passing_phase() {
        if let Some(cards) = normal.simple_pass_for(seat) {
            let _ = normal.submit_pass(seat, cards);
        }
        let _ = normal.submit_auto_passes_for_others(seat);
        let _ = normal.resolve_passes();
    }
    while !normal.in_passing_phase() && normal.expected_to_play() != seat {
        if normal.autoplay_one(seat).is_none() {
            break;
        }
    }
    let n_top = normal
        .explain_candidates_for(seat)
        .into_iter()
        .max_by_key(|(_, s)| s.clone())
        .map(|(c, _)| c)
        .unwrap();

    // Hard
    let mut hard = GameController::new_with_seed(Some(seed), PlayerPosition::North);
    hard.set_bot_difficulty(hearts_app::bot::BotDifficulty::FutureHard);
    if hard.in_passing_phase() {
        if let Some(cards) = hard.simple_pass_for(seat) {
            let _ = hard.submit_pass(seat, cards);
        }
        let _ = hard.submit_auto_passes_for_others(seat);
        let _ = hard.resolve_passes();
    }
    while !hard.in_passing_phase() && hard.expected_to_play() != seat {
        if hard.autoplay_one(seat).is_none() {
            break;
        }
    }
    let h_top = hard
        .explain_candidates_for(seat)
        .into_iter()
        .max_by_key(|(_, s)| s.clone())
        .map(|(c, _)| c)
        .unwrap();

    assert_ne!(
        n_top, h_top,
        "Expected Normal and Hard to disagree deterministically on 1146/South"
    );

    unsafe {
        std::env::remove_var("MDH_HARD_DETERMINISTIC");
        std::env::remove_var("MDH_HARD_TEST_STEPS");
    }
}
