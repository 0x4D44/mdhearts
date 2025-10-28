use hearts_app::bot::{BotContext, BotDifficulty, PlayPlannerHard};
use hearts_app::endgame_export::{EndgameExport, EndgameRehydrate};
use hearts_core::model::player::PlayerPosition;
use serde_json;

fn load_endgame_2120_west() -> EndgameRehydrate {
    let export: EndgameExport =
        serde_json::from_str(include_str!("fixtures/endgame_2120_west.json"))
            .expect("valid endgame export JSON");
    export.rehydrate().expect("rehydrate endgame state")
}

#[test]
#[ignore]
fn hard_endgame_dp_minimal_w2120_flip_difference() {
    // Deterministic with boosted endgame-only continuation to surface DP influence
    // (matches env used by DP flip seeker for seed 2120/west).
    unsafe {
        std::env::set_var("MDH_HARD_DETERMINISTIC", "1");
        std::env::set_var("MDH_HARD_TEST_STEPS", "120");
        // Endgame-only tuning knobs
        std::env::set_var("MDH_HARD_ENDGAME_DP_ENABLE", "1");
        std::env::set_var("MDH_HARD_ENDGAME_MAX_CARDS", "3");
        // Boosted continuation cap and next-trick continuation
        std::env::set_var("MDH_HARD_CONT_CAP", "1200");
        std::env::set_var("MDH_HARD_NEXT2_FEED_PERPEN", "400");
        std::env::set_var("MDH_HARD_NEXT2_SELF_CAPTURE_PERPEN", "60");
        // Penalize handing off control
        std::env::set_var("MDH_HARD_CTRL_HANDOFF_PEN", "150");
        // Use default current-trick continuation parts
        std::env::remove_var("MDH_HARD_CONT_FEED_PERPEN");
        std::env::remove_var("MDH_HARD_CONT_SELF_CAPTURE_PERPEN");
    }
    let seat = PlayerPosition::West;
    let EndgameRehydrate {
        round,
        mut scores,
        passing_direction,
        tracker,
        ..
    } = load_endgame_2120_west();
    // Align scoreboard with original constructed scenario to emphasize leader-feed pressure.
    scores.set_totals([90, 10, 20, 30]);
    let ctx = BotContext::new(
        seat,
        &round,
        &scores,
        passing_direction,
        &tracker,
        BotDifficulty::FutureHard,
    );
    let legal = round
        .hand(seat)
        .iter()
        .copied()
        .filter(|c| {
            let mut r = round.clone();
            r.play_card(seat, *c).is_ok()
        })
        .collect::<Vec<_>>();
    assert_eq!(legal.len(), 3);
    unsafe {
        std::env::remove_var("MDH_HARD_ENDGAME_DP_ENABLE");
    }
    let off = PlayPlannerHard::choose(&legal, &ctx);
    unsafe {
        std::env::set_var("MDH_HARD_ENDGAME_DP_ENABLE", "1");
    }
    let on = PlayPlannerHard::choose(&legal, &ctx);
    // Require difference; if this flakes under current caps, we will adjust or gate narrowly.
    assert_ne!(
        off, on,
        "expected DP to change top choice in minimal w2120 endgame"
    );
    unsafe {
        std::env::remove_var("MDH_HARD_DETERMINISTIC");
        std::env::remove_var("MDH_HARD_TEST_STEPS");
        std::env::remove_var("MDH_HARD_ENDGAME_DP_ENABLE");
        std::env::remove_var("MDH_HARD_CONT_CAP");
        std::env::remove_var("MDH_HARD_ENDGAME_MAX_CARDS");
        std::env::remove_var("MDH_HARD_NEXT2_FEED_PERPEN");
        std::env::remove_var("MDH_HARD_NEXT2_SELF_CAPTURE_PERPEN");
        std::env::remove_var("MDH_HARD_CTRL_HANDOFF_PEN");
        std::env::remove_var("MDH_HARD_CONT_FEED_PERPEN");
        std::env::remove_var("MDH_HARD_CONT_SELF_CAPTURE_PERPEN");
    }
}
