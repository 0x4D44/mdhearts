use hearts_app::bot::BotDifficulty;
use hearts_app::controller::GameController;
use hearts_core::model::player::PlayerPosition;

#[test]
fn hard_sampling_is_deterministic_under_step_cap() {
    unsafe {
        std::env::set_var("MDH_HARD_DETERMINISTIC", "1");
        std::env::set_var("MDH_HARD_TEST_STEPS", "12");
        std::env::set_var("MDH_HARD_BRANCH_LIMIT", "10");
        std::env::set_var("MDH_HARD_NEXT_BRANCH_LIMIT", "4");
        std::env::set_var("MDH_HARD_SAMPLE", "1");
        std::env::set_var("MDH_HARD_SAMPLE_N", "2");
    }
    let seed = 7777u64;
    let seat = PlayerPosition::East;

    let mut c1 = GameController::new_with_seed(Some(seed), PlayerPosition::North);
    c1.set_bot_difficulty(BotDifficulty::FutureHard);
    if c1.in_passing_phase() {
        if let Some(cards) = c1.simple_pass_for(seat) {
            let _ = c1.submit_pass(seat, cards);
        }
        let _ = c1.submit_auto_passes_for_others(seat);
        let _ = c1.resolve_passes();
    }
    while !c1.in_passing_phase() && c1.expected_to_play() != seat {
        if c1.autoplay_one(seat).is_none() {
            break;
        }
    }
    let v1 = c1.explain_candidates_for(seat);
    assert!(!v1.is_empty());

    let mut c2 = GameController::new_with_seed(Some(seed), PlayerPosition::North);
    c2.set_bot_difficulty(BotDifficulty::FutureHard);
    if c2.in_passing_phase() {
        if let Some(cards) = c2.simple_pass_for(seat) {
            let _ = c2.submit_pass(seat, cards);
        }
        let _ = c2.submit_auto_passes_for_others(seat);
        let _ = c2.resolve_passes();
    }
    while !c2.in_passing_phase() && c2.expected_to_play() != seat {
        if c2.autoplay_one(seat).is_none() {
            break;
        }
    }
    let v2 = c2.explain_candidates_for(seat);

    assert_eq!(
        v1, v2,
        "sampling path should be deterministic under step cap"
    );
}
