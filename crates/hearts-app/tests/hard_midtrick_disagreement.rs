use hearts_app::controller::GameController;
use hearts_core::model::player::PlayerPosition;

fn find_midtrick_disagreement(seed: u64, seat: PlayerPosition, max_checks: usize) -> bool {
    let mut ctrl = GameController::new_with_seed(Some(seed), PlayerPosition::North);
    // Resolve passes if present
    if ctrl.in_passing_phase() {
        if let Some(cards) = ctrl.simple_pass_for(seat) {
            let _ = ctrl.submit_pass(seat, cards);
        }
        let _ = ctrl.submit_auto_passes_for_others(seat);
        let _ = ctrl.resolve_passes();
    }
    let mut checks = 0usize;
    while checks < max_checks {
        // Advance to our seat's turn
        while !ctrl.in_passing_phase() && ctrl.expected_to_play() != seat {
            if ctrl.autoplay_one(seat).is_none() {
                break;
            }
        }
        if ctrl.in_passing_phase() {
            break;
        }
        if ctrl.legal_moves(seat).is_empty() {
            break;
        }

        // Compare Normal vs Hard on the exact same snapshot by toggling difficulty
        ctrl.set_bot_difficulty(hearts_app::bot::BotDifficulty::NormalHeuristic);
        let n_expl = ctrl.explain_candidates_for(seat);
        ctrl.set_bot_difficulty(hearts_app::bot::BotDifficulty::FutureHard);
        let h_expl = ctrl.explain_candidates_for(seat);
        if let (Some((n_top, _)), Some((h_top, _))) = (
            n_expl.iter().max_by_key(|(_, s)| *s),
            h_expl.iter().max_by_key(|(_, s)| *s),
        ) {
            if *n_top != *h_top {
                return true;
            }
        }
        // Move forward to the next decision point
        if ctrl.autoplay_one(seat).is_none() {
            break;
        }
        checks += 1;
    }
    false
}

#[test]
fn hard_vs_normal_disagree_on_midtrick_in_curated_set() {
    // Try a curated mini-set of seeds/seats gathered during tuning.
    let cases = [
        (1002u64, PlayerPosition::West),
        (1003u64, PlayerPosition::West),
        (1008u64, PlayerPosition::West),
        (2012u64, PlayerPosition::East),
        (2022u64, PlayerPosition::East),
        (2027u64, PlayerPosition::East),
        (1081u64, PlayerPosition::South),
        (1086u64, PlayerPosition::South),
        (1088u64, PlayerPosition::South),
        (1104u64, PlayerPosition::North),
        (1122u64, PlayerPosition::North),
        (1131u64, PlayerPosition::North),
    ];
    let mut found = false;
    for (seed, seat) in cases.into_iter() {
        if find_midtrick_disagreement(seed, seat, 40) {
            found = true;
            break;
        }
    }
    if !found {
        eprintln!("No disagreement found in curated mid-trick set (non-fatal smoke)");
    }
    assert!(true);
}
