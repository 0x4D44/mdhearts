use hearts_app::bot::BotDifficulty;
use hearts_app::controller::GameController;
use hearts_core::model::player::PlayerPosition;

#[test]
fn hard_branch_limit_respected() {
    // Force a small branch limit and verify explain returns no more than that many candidates.
    unsafe {
        std::env::set_var("MDH_HARD_BRANCH_LIMIT", "3");
    }
    let seed = 777u64;
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

    let explained = controller.explain_candidates_for(seat);
    assert!(!explained.is_empty());
    assert!(
        explained.len() <= 3,
        "explain returned {} > 3",
        explained.len()
    );
}
