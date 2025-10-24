use hearts_app::controller::GameController;
use hearts_app::bot::BotDifficulty;
use hearts_core::model::player::PlayerPosition;

#[test]
fn hard_probe_margin_does_not_change_explain() {
    unsafe {
        std::env::set_var("MDH_HARD_DETERMINISTIC", "1");
        std::env::set_var("MDH_HARD_TEST_STEPS", "120");
        std::env::set_var("MDH_HARD_PHASEB_TOPK", "3");
    }
    let seed: u64 = 1145;
    let seat = PlayerPosition::North;
    let mut controller = GameController::new_with_seed(Some(seed), PlayerPosition::North);
    controller.set_bot_difficulty(BotDifficulty::FutureHard);
    if controller.in_passing_phase() {
        if let Some(cards) = controller.simple_pass_for(seat) { let _ = controller.submit_pass(seat, cards); }
        let _ = controller.submit_auto_passes_for_others(seat);
        let _ = controller.resolve_passes();
    }
    while !controller.in_passing_phase() && controller.expected_to_play() != seat {
        if controller.autoplay_one(seat).is_none() { break; }
    }
    let legal = controller.legal_moves(seat);
    let ctx = controller.bot_context(seat);

    // Explain without margin
    unsafe { std::env::remove_var("MDH_HARD_PROBE_AB_MARGIN"); }
    let expl_base = hearts_app::bot::PlayPlannerHard::explain_candidates(&legal, &ctx);
    // Explain with margin
    unsafe { std::env::set_var("MDH_HARD_PROBE_AB_MARGIN", "40"); }
    let expl_ab = hearts_app::bot::PlayPlannerHard::explain_candidates(&legal, &ctx);
    assert_eq!(expl_base, expl_ab, "probe margin must not affect explain output");

    unsafe {
        std::env::remove_var("MDH_HARD_DETERMINISTIC");
        std::env::remove_var("MDH_HARD_TEST_STEPS");
        std::env::remove_var("MDH_HARD_PHASEB_TOPK");
        std::env::remove_var("MDH_HARD_PROBE_AB_MARGIN");
    }
}

