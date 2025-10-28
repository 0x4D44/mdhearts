use hearts_app::bot::{BotDifficulty, PlayPlannerHard};
use hearts_app::controller::GameController;
use hearts_core::model::card::Card;
use hearts_core::model::player::PlayerPosition;
use std::sync::{Mutex, OnceLock};

type FlipResult = (usize, Option<Card>, Option<Card>);

fn env_lock() -> &'static Mutex<()> {
    static LOCK: OnceLock<Mutex<()>> = OnceLock::new();
    LOCK.get_or_init(|| Mutex::new(()))
}

fn compute_flip(seed: u64, seat: PlayerPosition) -> FlipResult {
    let mut controller = GameController::new_with_seed(Some(seed), PlayerPosition::North);
    controller.set_bot_difficulty(BotDifficulty::FutureHard);
    if controller.in_passing_phase() {
        if let Some(cards) = controller.simple_pass_for(seat) {
            let _ = controller.submit_pass(seat, cards);
        }
        let _ = controller.submit_auto_passes_for_others(seat);
        let _ = controller.resolve_passes();
    }
    let mut guard = 0u32;
    loop {
        if guard > 600 {
            panic!("failed to reach endgame state for seed {seed:?}");
        }
        guard += 1;
        let to_play = controller.expected_to_play();
        let round = controller.bot_context(seat).round;
        let mut ok_small = true;
        for s in PlayerPosition::LOOP.iter().copied() {
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

    let legal = controller.legal_moves(seat);
    if legal.is_empty() {
        return (0, None, None);
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
    unsafe {
        std::env::remove_var("MDH_HARD_ENDGAME_DP_ENABLE");
    }
    (legal.len(), off, on)
}

fn run_flip_assert(seed: u64, seat: PlayerPosition) {
    let _guard = env_lock().lock().unwrap();
    unsafe {
        std::env::set_var("MDH_HARD_DETERMINISTIC", "1");
        std::env::set_var("MDH_HARD_TEST_STEPS", "160");
    }
    let (count, off, on) = compute_flip(seed, seat);
    assert!(count > 0, "expected legal moves");
    assert_ne!(off, on, "expected DP flip for seed {seed} {seat:?}");
    unsafe {
        std::env::remove_var("MDH_HARD_DETERMINISTIC");
        std::env::remove_var("MDH_HARD_TEST_STEPS");
    }
}

#[test]
fn hard_endgame_dp_flip_w2263_default_weights() {
    run_flip_assert(2263, PlayerPosition::West);
}

#[test]
fn hard_endgame_dp_flip_s2325_default_weights() {
    run_flip_assert(2325, PlayerPosition::South);
}

#[test]
fn hard_endgame_dp_flip_e1383_default_weights() {
    run_flip_assert(1383, PlayerPosition::East);
}
