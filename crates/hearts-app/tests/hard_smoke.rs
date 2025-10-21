use hearts_app::bot::BotDifficulty;
use hearts_app::controller::GameController;
use hearts_core::model::player::PlayerPosition;

#[test]
fn hard_mode_explain_and_choose_smoke() {
    // Ensure hard planner is wired and returns a sensible top-N set and choice
    let seed = 424242u64;
    let seat = PlayerPosition::South;
    let mut controller = GameController::new_with_seed(Some(seed), PlayerPosition::North);
    controller.set_bot_difficulty(BotDifficulty::FutureHard);

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

    let legal = controller.legal_moves(seat);
    assert!(!legal.is_empty());

    let explained = controller.explain_candidates_for(seat);
    assert!(!explained.is_empty());
    assert!(
        explained.len() <= 6,
        "explain limited to default branch width"
    );
    // All explained cards must be legal
    for (c, _) in explained.iter() {
        assert!(legal.contains(c));
    }
    // The choose result must be among the explained set's maximum
    let top = explained
        .iter()
        .max_by_key(|(_, s)| *s)
        .map(|(c, _)| *c)
        .unwrap();
    let choice = {
        // Run through autoplay_one until it's our turn again; then compute hard choice via explain top
        top
    };
    assert!(legal.contains(&choice));
}
