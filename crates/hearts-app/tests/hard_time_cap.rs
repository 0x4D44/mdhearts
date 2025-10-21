use hearts_app::bot::BotDifficulty;
use hearts_app::controller::GameController;
use hearts_core::model::player::PlayerPosition;

#[test]
fn hard_time_cap_respected_smoke() {
    // Set a tiny time cap to force early stop in scanning
    unsafe {
        std::env::set_var("MDH_HARD_TIME_CAP_MS", "1");
        std::env::set_var("MDH_HARD_BRANCH_LIMIT", "12");
    }
    let seed = 24680u64;
    let seat = PlayerPosition::North;
    let mut controller = GameController::new_with_seed(Some(seed), PlayerPosition::North);
    controller.set_bot_difficulty(BotDifficulty::FutureHard);

    // Resolve passing if needed and play up to our seat
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
    // Trigger Hard explain which records stats
    let explained = controller.explain_candidates_for(seat);
    assert!(!explained.is_empty());
    if let Some(stats) = hearts_app::bot::search::last_stats() {
        // elapsed should be within a small multiple of cap (allowing coarse timer granularity)
        assert!(
            stats.elapsed_ms <= 5,
            "elapsed {}ms exceeds expected window",
            stats.elapsed_ms
        );
        assert!(
            stats.scanned <= 12,
            "scanned {} exceeds branch limit",
            stats.scanned
        );
    } else {
        panic!("no hard stats recorded");
    }
}
