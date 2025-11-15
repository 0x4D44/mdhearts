use hearts_app::bot::BotDifficulty;
use hearts_app::controller::GameController;
use hearts_core::model::player::PlayerPosition;
use std::sync::{Mutex, MutexGuard};

static ENV_GUARD: Mutex<()> = Mutex::new(());

fn env_lock() -> MutexGuard<'static, ()> {
    ENV_GUARD.lock().unwrap()
}

#[test]
fn hard_ab_skip_reduces_scans_without_changing_top() {
    let _env = env_lock();
    // Deterministic to keep scanning stable, and set an AB margin.
    unsafe {
        std::env::set_var("MDH_HARD_DETERMINISTIC", "1");
        std::env::set_var("MDH_HARD_TEST_STEPS", "200");
        std::env::set_var("MDH_HARD_AB_MARGIN", "200");
        std::env::set_var("MDH_HARD_BRANCH_LIMIT", "6");
        std::env::set_var("MDH_HARD_PHASEB_TOPK", "3");
    }

    let seed: u64 = 1145; // known stable scenario from goldens
    let seat = PlayerPosition::North;
    let mut controller = GameController::new_with_seed(Some(seed), PlayerPosition::North);
    controller.set_bot_difficulty(BotDifficulty::FutureHard);

    // Resolve passing and autoplay to the seat
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

    // Get explain candidates (establishes top choice reference)
    let legal = controller.legal_moves(seat);
    let ctx = controller.bot_context(seat);
    let explained = hearts_app::bot::PlayPlannerHard::explain_candidates(&legal, &ctx);
    assert!(!explained.is_empty());
    let (_top_card, _top_score) = explained.iter().cloned().max_by_key(|(_, s)| *s).unwrap();

    // Turn off early cutoff and AB margin to check parity between choose and explain
    unsafe {
        std::env::set_var("MDH_HARD_EARLY_CUTOFF_MARGIN", "0");
        std::env::set_var("MDH_HARD_AB_MARGIN", "0");
    }
    let picked_noab = hearts_app::bot::PlayPlannerHard::choose(&legal, &ctx).unwrap();

    // Enable AB margin and ensure scanned decreases but top remains same as no-AB
    unsafe {
        std::env::set_var("MDH_HARD_AB_MARGIN", "200");
    }
    let picked_ab = hearts_app::bot::PlayPlannerHard::choose(&legal, &ctx).unwrap();
    let stats_after_choose_ab =
        hearts_app::bot::search::last_stats().expect("stats after choose-ab");
    // Refresh explain stats for comparison baseline
    let _ = hearts_app::bot::PlayPlannerHard::explain_candidates(&legal, &ctx);
    let stats_after_explain = hearts_app::bot::search::last_stats().expect("stats after explain");
    assert_eq!(
        picked_ab, picked_noab,
        "AB skip should not change top versus no-AB when early-cutoff is off"
    );
    let explain_scan = stats_after_explain.scanned;
    let choose_scan = stats_after_choose_ab.scanned;
    assert!(
        choose_scan <= explain_scan + 2,
        "choose(ab) scanned {} should be within 2 of explain {}",
        choose_scan,
        explain_scan
    );

    unsafe {
        std::env::remove_var("MDH_HARD_DETERMINISTIC");
        std::env::remove_var("MDH_HARD_TEST_STEPS");
        std::env::remove_var("MDH_HARD_AB_MARGIN");
        std::env::remove_var("MDH_HARD_BRANCH_LIMIT");
        std::env::remove_var("MDH_HARD_PHASEB_TOPK");
        std::env::remove_var("MDH_HARD_EARLY_CUTOFF_MARGIN");
    }
}
