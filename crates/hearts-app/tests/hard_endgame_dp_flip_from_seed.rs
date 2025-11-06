use hearts_app::bot::{BotDifficulty, PlayPlannerHard};
use hearts_app::controller::GameController;
use hearts_core::model::player::PlayerPosition;

fn autoplay_to_small_endgame(controller: &mut GameController, seat: PlayerPosition) {
    // Resolve passes if any
    if controller.in_passing_phase() {
        if let Some(cards) = controller.simple_pass_for(seat) {
            let _ = controller.submit_pass(seat, cards);
        }
        let _ = controller.submit_auto_passes_for_others(seat);
        let _ = controller.resolve_passes();
    }
    let mut guard = 0u32;
    loop {
        if guard > 500 {
            break;
        }
        guard += 1;
        let to_play = controller.expected_to_play();
        let round = controller.bot_context(seat).round;
        let mut ok_small = true;
        for s in [
            PlayerPosition::North,
            PlayerPosition::East,
            PlayerPosition::South,
            PlayerPosition::West,
        ] {
            if round.hand(s).len() > 3 {
                ok_small = false;
                break;
            }
        }
        if ok_small && to_play == seat {
            break;
        }
        let _ = controller.autoplay_one(to_play.next());
    }
}

#[test]
#[ignore]
fn hard_endgame_dp_flip_from_seed_1983_west() {
    // Deterministic caps for stability
    unsafe {
        std::env::set_var("MDH_HARD_DETERMINISTIC", "1");
        std::env::set_var("MDH_HARD_TEST_STEPS", "120");
    }
    let seat = PlayerPosition::West;
    let mut controller = GameController::new_with_seed(Some(1983), PlayerPosition::North);
    controller.set_bot_difficulty(BotDifficulty::FutureHard);
    autoplay_to_small_endgame(&mut controller, seat);
    let legal = controller.legal_moves(seat);
    assert!(!legal.is_empty());
    let ctx = controller.bot_context(seat);
    // Apply endgame-only env boosts to surface DP effect deterministically
    unsafe {
        std::env::set_var("MDH_HARD_CONT_FEED_PERPEN", "0");
        std::env::set_var("MDH_HARD_CONT_CAP", "1200");
        std::env::set_var("MDH_HARD_NEXT2_FEED_PERPEN", "400");
        std::env::set_var("MDH_HARD_NEXT2_SELF_CAPTURE_PERPEN", "60");
        std::env::set_var("MDH_HARD_CTRL_HANDOFF_PEN", "150");
    }
    // Toggle DP OFF
    unsafe {
        std::env::remove_var("MDH_HARD_ENDGAME_DP_ENABLE");
    }
    let off = PlayPlannerHard::choose(&legal, &ctx);
    // Toggle DP ON
    unsafe {
        std::env::set_var("MDH_HARD_ENDGAME_DP_ENABLE", "1");
    }
    let on = PlayPlannerHard::choose(&legal, &ctx);
    // Require difference; if this proves flaky across toolchains, we will gate with small endgame-only boosts.
    assert_ne!(off, on, "expected DP to flip top choice for seed 1983 west");
    // Cleanup env
    unsafe {
        std::env::remove_var("MDH_HARD_DETERMINISTIC");
        std::env::remove_var("MDH_HARD_TEST_STEPS");
        std::env::remove_var("MDH_HARD_ENDGAME_DP_ENABLE");
        std::env::remove_var("MDH_HARD_CONT_FEED_PERPEN");
        std::env::remove_var("MDH_HARD_CONT_CAP");
        std::env::remove_var("MDH_HARD_NEXT2_FEED_PERPEN");
        std::env::remove_var("MDH_HARD_NEXT2_SELF_CAPTURE_PERPEN");
        std::env::remove_var("MDH_HARD_CTRL_HANDOFF_PEN");
    }
}

#[test]
#[ignore]
fn hard_endgame_dp_flip_from_seed_2120_west() {
    // Deterministic caps and endgame-only boosts to surface DP effect
    unsafe {
        std::env::set_var("MDH_HARD_DETERMINISTIC", "1");
        std::env::set_var("MDH_HARD_TEST_STEPS", "120");
        std::env::set_var("MDH_HARD_CONT_FEED_PERPEN", "0");
        std::env::set_var("MDH_HARD_CONT_CAP", "1200");
        std::env::set_var("MDH_HARD_NEXT2_FEED_PERPEN", "400");
        std::env::set_var("MDH_HARD_NEXT2_SELF_CAPTURE_PERPEN", "60");
        std::env::set_var("MDH_HARD_CTRL_HANDOFF_PEN", "150");
    }
    let seat = PlayerPosition::West;
    let mut controller = GameController::new_with_seed(Some(2120), PlayerPosition::North);
    controller.set_bot_difficulty(BotDifficulty::FutureHard);
    autoplay_to_small_endgame(&mut controller, seat);
    let legal = controller.legal_moves(seat);
    assert!(!legal.is_empty());
    let ctx = controller.bot_context(seat);
    // Toggle DP OFF
    unsafe {
        std::env::remove_var("MDH_HARD_ENDGAME_DP_ENABLE");
    }
    let off = PlayPlannerHard::choose(&legal, &ctx);
    // Toggle DP ON
    unsafe {
        std::env::set_var("MDH_HARD_ENDGAME_DP_ENABLE", "1");
    }
    let on = PlayPlannerHard::choose(&legal, &ctx);
    assert_ne!(off, on, "expected DP to flip top choice for seed 2120 west");
    // Cleanup env
    unsafe {
        std::env::remove_var("MDH_HARD_DETERMINISTIC");
        std::env::remove_var("MDH_HARD_TEST_STEPS");
        std::env::remove_var("MDH_HARD_ENDGAME_DP_ENABLE");
        std::env::remove_var("MDH_HARD_CONT_FEED_PERPEN");
        std::env::remove_var("MDH_HARD_CONT_CAP");
        std::env::remove_var("MDH_HARD_NEXT2_FEED_PERPEN");
        std::env::remove_var("MDH_HARD_NEXT2_SELF_CAPTURE_PERPEN");
        std::env::remove_var("MDH_HARD_CTRL_HANDOFF_PEN");
    }
}

#[test]
fn hard_endgame_dp_flip_from_seed_2269_east() {
    unsafe {
        std::env::set_var("MDH_HARD_DETERMINISTIC", "1");
        std::env::set_var("MDH_HARD_TEST_STEPS", "120");
        std::env::set_var("MDH_HARD_CONT_FEED_PERPEN", "0");
        std::env::set_var("MDH_HARD_CONT_CAP", "1200");
        std::env::set_var("MDH_HARD_NEXT2_FEED_PERPEN", "400");
        std::env::set_var("MDH_HARD_NEXT2_SELF_CAPTURE_PERPEN", "60");
        std::env::set_var("MDH_HARD_CTRL_HANDOFF_PEN", "150");
    }
    let seat = PlayerPosition::East;
    let mut controller = GameController::new_with_seed(Some(2269), PlayerPosition::North);
    controller.set_bot_difficulty(BotDifficulty::FutureHard);
    autoplay_to_small_endgame(&mut controller, seat);
    let legal = controller.legal_moves(seat);
    assert!(!legal.is_empty());
    let ctx = controller.bot_context(seat);
    unsafe {
        std::env::remove_var("MDH_HARD_ENDGAME_DP_ENABLE");
    }
    let off = PlayPlannerHard::choose(&legal, &ctx);
    unsafe {
        std::env::set_var("MDH_HARD_ENDGAME_DP_ENABLE", "1");
    }
    let on = PlayPlannerHard::choose(&legal, &ctx);
    assert_ne!(off, on, "expected DP to flip top choice for seed 2269 east");
    unsafe {
        std::env::remove_var("MDH_HARD_DETERMINISTIC");
        std::env::remove_var("MDH_HARD_TEST_STEPS");
        std::env::remove_var("MDH_HARD_ENDGAME_DP_ENABLE");
        std::env::remove_var("MDH_HARD_CONT_FEED_PERPEN");
        std::env::remove_var("MDH_HARD_CONT_CAP");
        std::env::remove_var("MDH_HARD_NEXT2_FEED_PERPEN");
        std::env::remove_var("MDH_HARD_NEXT2_SELF_CAPTURE_PERPEN");
        std::env::remove_var("MDH_HARD_CTRL_HANDOFF_PEN");
    }
}

#[test]
#[ignore]
fn hard_endgame_dp_flip_from_seed_1052_south() {
    unsafe {
        std::env::set_var("MDH_HARD_DETERMINISTIC", "1");
        std::env::set_var("MDH_HARD_TEST_STEPS", "120");
        std::env::set_var("MDH_HARD_CONT_FEED_PERPEN", "0");
        std::env::set_var("MDH_HARD_CONT_CAP", "1200");
        std::env::set_var("MDH_HARD_NEXT2_FEED_PERPEN", "400");
        std::env::set_var("MDH_HARD_NEXT2_SELF_CAPTURE_PERPEN", "60");
        std::env::set_var("MDH_HARD_CTRL_HANDOFF_PEN", "150");
    }
    let seat = PlayerPosition::South;
    let mut controller = GameController::new_with_seed(Some(1052), PlayerPosition::North);
    controller.set_bot_difficulty(BotDifficulty::FutureHard);
    autoplay_to_small_endgame(&mut controller, seat);
    let legal = controller.legal_moves(seat);
    assert!(!legal.is_empty());
    let ctx = controller.bot_context(seat);
    unsafe {
        std::env::remove_var("MDH_HARD_ENDGAME_DP_ENABLE");
    }
    let off = PlayPlannerHard::choose(&legal, &ctx);
    unsafe {
        std::env::set_var("MDH_HARD_ENDGAME_DP_ENABLE", "1");
    }
    let on = PlayPlannerHard::choose(&legal, &ctx);
    assert_ne!(
        off, on,
        "expected DP to flip top choice for seed 1052 south"
    );
    unsafe {
        std::env::remove_var("MDH_HARD_DETERMINISTIC");
        std::env::remove_var("MDH_HARD_TEST_STEPS");
        std::env::remove_var("MDH_HARD_ENDGAME_DP_ENABLE");
        std::env::remove_var("MDH_HARD_CONT_FEED_PERPEN");
        std::env::remove_var("MDH_HARD_CONT_CAP");
        std::env::remove_var("MDH_HARD_NEXT2_FEED_PERPEN");
        std::env::remove_var("MDH_HARD_NEXT2_SELF_CAPTURE_PERPEN");
        std::env::remove_var("MDH_HARD_CTRL_HANDOFF_PEN");
    }
}

#[test]
#[ignore]
fn search_west_dp_flip_new() {
    unsafe {
        std::env::set_var("MDH_HARD_DETERMINISTIC", "1");
        std::env::set_var("MDH_HARD_TEST_STEPS", "120");
        std::env::set_var("MDH_HARD_CONT_FEED_PERPEN", "0");
        std::env::set_var("MDH_HARD_CONT_CAP", "1200");
        std::env::set_var("MDH_HARD_NEXT2_FEED_PERPEN", "400");
        std::env::set_var("MDH_HARD_NEXT2_SELF_CAPTURE_PERPEN", "60");
        std::env::set_var("MDH_HARD_CTRL_HANDOFF_PEN", "150");
    }
    for seed in 0..10000 {
        let seat = PlayerPosition::West;
        let mut controller = GameController::new_with_seed(Some(seed), PlayerPosition::North);
        controller.set_bot_difficulty(BotDifficulty::FutureHard);
        autoplay_to_small_endgame(&mut controller, seat);
        let legal = controller.legal_moves(seat);
        if legal.is_empty() {
            continue;
        }
        let ctx = controller.bot_context(seat);
        unsafe {
            std::env::remove_var("MDH_HARD_ENDGAME_DP_ENABLE");
        }
        let off = PlayPlannerHard::choose(&legal, &ctx);
        unsafe {
            std::env::set_var("MDH_HARD_ENDGAME_DP_ENABLE", "1");
        }
        let on = PlayPlannerHard::choose(&legal, &ctx);
        if off != on {
            println!("west seed {seed} flips: off={off:?} on={on:?}");
            break;
        }
    }
    unsafe {
        std::env::remove_var("MDH_HARD_DETERMINISTIC");
        std::env::remove_var("MDH_HARD_TEST_STEPS");
        std::env::remove_var("MDH_HARD_ENDGAME_DP_ENABLE");
        std::env::remove_var("MDH_HARD_CONT_FEED_PERPEN");
        std::env::remove_var("MDH_HARD_CONT_CAP");
        std::env::remove_var("MDH_HARD_NEXT2_FEED_PERPEN");
        std::env::remove_var("MDH_HARD_NEXT2_SELF_CAPTURE_PERPEN");
        std::env::remove_var("MDH_HARD_CTRL_HANDOFF_PEN");
    }
}
