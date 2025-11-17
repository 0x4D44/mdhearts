use hearts_app::bot::BotDifficulty;
use hearts_app::controller::GameController;
use hearts_core::model::player::PlayerPosition;

#[test]
fn hard_probe_skip_reduces_scans_under_margin() {
    // Deterministic to keep scanning stable
    unsafe {
        std::env::set_var("MDH_HARD_DETERMINISTIC", "1");
        std::env::set_var("MDH_HARD_TEST_STEPS", "200");
        std::env::set_var("MDH_HARD_PHASEB_TOPK", "3");
        std::env::remove_var("MDH_HARD_PROBE_AB_MARGIN");
        // Disable endgame solver and deep search to ensure we test the regular search path stats
        std::env::set_var("MDH_ENDGAME_SOLVER_ENABLED", "0");
        std::env::set_var("MDH_SEARCH_DEEPER_ENABLED", "0");
    }

    let seed: u64 = 1145; // known stable seat
    let seat = PlayerPosition::North;
    let mut controller = GameController::new_with_seed(Some(seed), PlayerPosition::North);
    controller.set_bot_difficulty(BotDifficulty::FutureHard);

    if controller.in_passing_phase() {
        if let Some(cards) = controller.simple_pass_for(seat) {
            let _ = controller.submit_pass(seat, cards);
        }
        let _ = controller.submit_auto_passes_for_others(seat);
        let _ = controller.resolve_passes();
    }
    while !controller.in_passing_phase() && controller.expected_to_play() != seat {
        if controller.autoplay_one(seat).is_none() {
            break;
        }
    }
    let legal = controller.legal_moves(seat);
    let ctx = controller.bot_context(seat);

    // Baseline choose without probe AB margin
    let _ = hearts_app::bot::PlayPlannerHard::choose(&legal, &ctx);
    let base = hearts_app::bot::search::last_stats().expect("baseline stats");

    // Enable probe AB margin and choose again
    unsafe {
        std::env::set_var("MDH_HARD_PROBE_AB_MARGIN", "40");
    }
    let _ = hearts_app::bot::PlayPlannerHard::choose(&legal, &ctx);
    let ab = hearts_app::bot::search::last_stats().expect("ab stats");

    assert!(
        ab.scanned <= base.scanned,
        "probe-AB scanned={} should be <= baseline {}",
        ab.scanned,
        base.scanned
    );

    // Cleanup
    unsafe {
        std::env::remove_var("MDH_HARD_DETERMINISTIC");
        std::env::remove_var("MDH_HARD_TEST_STEPS");
        std::env::remove_var("MDH_HARD_PHASEB_TOPK");
        std::env::remove_var("MDH_HARD_PROBE_AB_MARGIN");
        std::env::remove_var("MDH_ENDGAME_SOLVER_ENABLED");
        std::env::remove_var("MDH_SEARCH_DEEPER_ENABLED");
    }
}
