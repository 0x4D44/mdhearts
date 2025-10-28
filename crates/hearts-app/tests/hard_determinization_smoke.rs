use hearts_app::bot::BotDifficulty;
use hearts_app::controller::GameController;
use hearts_core::model::player::PlayerPosition;

#[test]
fn hard_determinization_smoke() {
    // Enable determinization with small K and probe widening env; ensure it runs and returns candidates
    unsafe {
        std::env::set_var("MDH_HARD_DET_ENABLE", "1");
        std::env::set_var("MDH_HARD_DET_SAMPLE_K", "3");
        std::env::set_var("MDH_HARD_DET_PROBE_WIDE_LIKE", "1");
        std::env::set_var("MDH_HARD_DETERMINISTIC", "1");
        std::env::set_var("MDH_HARD_TEST_STEPS", "120");
    }

    let seed = 13579u64;
    let seat = PlayerPosition::East;
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
    assert!(!legal.is_empty());
    let explained = controller.explain_candidates_for(seat);
    assert!(!explained.is_empty());
    for (c, _) in explained.iter() {
        assert!(legal.contains(c));
    }

    // Cleanup env to avoid side-effects on other tests
    unsafe {
        std::env::remove_var("MDH_HARD_DET_ENABLE");
        std::env::remove_var("MDH_HARD_DET_SAMPLE_K");
        std::env::remove_var("MDH_HARD_DET_PROBE_WIDE_LIKE");
        std::env::remove_var("MDH_HARD_DETERMINISTIC");
        std::env::remove_var("MDH_HARD_TEST_STEPS");
    }
}
