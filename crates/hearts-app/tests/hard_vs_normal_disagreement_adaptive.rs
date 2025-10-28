use hearts_app::controller::GameController;
use hearts_core::model::player::PlayerPosition;

#[test]
fn west_seed_1141_disagreement_under_adaptive_and_sampling() {
    // Enable adaptive limits, third-opponent branching, and sampling to reproduce this curated disagreement.
    unsafe {
        std::env::set_var("MDH_HARD_ADAPTIVE", "1");
        std::env::set_var("MDH_HARD_THIRD_BRANCH", "1");
        std::env::set_var("MDH_HARD_SAMPLE", "1");
        std::env::set_var("MDH_HARD_SAMPLE_N", "2");
    }

    let seed: u64 = 1141;
    let seat = PlayerPosition::West;

    // Normal
    let mut normal = GameController::new_with_seed(Some(seed), PlayerPosition::North);
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
    let n_expl = normal.explain_candidates_for(seat);
    let n_top = n_expl
        .iter()
        .max_by_key(|(_, s)| *s)
        .map(|(c, _)| *c)
        .unwrap();

    // Hard with features enabled
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
    let h_expl = hard.explain_candidates_for(seat);
    let h_top = h_expl
        .iter()
        .max_by_key(|(_, s)| *s)
        .map(|(c, _)| *c)
        .unwrap();

    assert_ne!(
        n_top, h_top,
        "Expected disagreement at seed {} {:?}",
        seed, seat
    );
}
