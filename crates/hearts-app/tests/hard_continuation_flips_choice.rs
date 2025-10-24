use hearts_app::controller::GameController;
use hearts_core::model::player::PlayerPosition;

// Constructed scenario where Hard's continuation flips the overall top choice vs Normal.
// Normal prefers a non-capturing safe line on current trick; Hard prefers capturing now
// because it can lead a next-trick suit that feeds penalties to the scoreboard leader.
#[test]
fn hard_continuation_flips_choice_vs_normal() {
    // Use a known seed/seat combo similar to our constructed favorable shape.
    // Deterministic mode not required here; if this ever becomes unstable, consider adding a snapshot.
    let seed: u64 = 1141; // chosen from disagreement set; kept stable locally
    let seat = PlayerPosition::West;

    // Normal
    let mut normal = GameController::new_with_seed(Some(seed), PlayerPosition::North);
    normal.set_bot_difficulty(hearts_app::bot::BotDifficulty::NormalHeuristic);
    if normal.in_passing_phase() {
        if let Some(cards) = normal.simple_pass_for(seat) { let _ = normal.submit_pass(seat, cards); }
        let _ = normal.submit_auto_passes_for_others(seat);
        let _ = normal.resolve_passes();
    }
    while !normal.in_passing_phase() && normal.expected_to_play() != seat {
        if normal.autoplay_one(seat).is_none() { break; }
    }
    let n_expl = normal.explain_candidates_for(seat);
    let n_top = n_expl.iter().max_by_key(|(_, s)| *s).map(|(c, _)| *c).unwrap();

    // Hard (baseline: adaptive/sampling off by default)
    let mut hard = GameController::new_with_seed(Some(seed), PlayerPosition::North);
    hard.set_bot_difficulty(hearts_app::bot::BotDifficulty::FutureHard);
    if hard.in_passing_phase() {
        if let Some(cards) = hard.simple_pass_for(seat) { let _ = hard.submit_pass(seat, cards); }
        let _ = hard.submit_auto_passes_for_others(seat);
        let _ = hard.resolve_passes();
    }
    while !hard.in_passing_phase() && hard.expected_to_play() != seat {
        if hard.autoplay_one(seat).is_none() { break; }
    }
    let h_expl = hard.explain_candidates_for(seat);
    let h_top = h_expl.iter().max_by_key(|(_, s)| *s).map(|(c, _)| *c).unwrap();

    // This curated case should disagree: flipping behavior expected (tracked in journal).
    assert_ne!(
        n_top, h_top,
        "Expected Hard to flip choice vs Normal on this seed/seat"
    );
}

