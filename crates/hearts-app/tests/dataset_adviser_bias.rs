use hearts_app::bot::BotDifficulty;
use hearts_app::bot::search::PlayPlannerHard;
use hearts_app::controller::GameController;
use hearts_app::dataset::collect_play_sample;
use hearts_core::model::card::Card;
use hearts_core::model::player::PlayerPosition;
use hearts_core::model::rank::Rank;
use hearts_core::model::suit::Suit;
use std::path::PathBuf;

fn build_controller(seed: u64) -> GameController {
    let mut controller = GameController::new_with_seed(Some(seed), PlayerPosition::North);
    controller.set_bot_difficulty(BotDifficulty::FutureHard);
    controller
}

fn prepare_for_play(mut controller: GameController, seat: PlayerPosition) -> GameController {
    if controller.in_passing_phase() {
        if let Some(cards) = controller.simple_pass_for(seat) {
            let _ = controller.submit_pass(seat, cards);
        }
        let _ = controller.submit_auto_passes_for_others(seat);
        let _ = controller.resolve_passes();
    }
    for _ in 0..52 {
        if controller.expected_to_play() == seat {
            break;
        }
        if controller.autoplay_one(seat).is_none() {
            break;
        }
    }
    assert_eq!(
        controller.expected_to_play(),
        seat,
        "controller should be ready for the target seat to act"
    );
    controller
}

#[test]
fn dataset_records_adviser_bias_toggle() {
    unsafe {
        std::env::remove_var("MDH_HARD_ADVISER_PLAY");
        std::env::remove_var("MDH_ADVISER_PLAY_PATH");
        std::env::remove_var("MDH_SEARCH_MIX_HINT");
    }

    let seed = 1234;
    let seat = PlayerPosition::West;
    let controller = prepare_for_play(build_controller(seed), seat);
    assert_eq!(controller.bot_difficulty(), BotDifficulty::FutureHard);
    let legal = controller.legal_moves(seat);
    assert!(!legal.is_empty(), "expected legal moves for target seat");
    {
        let ctx = controller.bot_context(seat);
        assert!(
            !PlayPlannerHard::explain_candidates_verbose_parts(&legal, &ctx).is_empty(),
            "expected hard planner to return candidates"
        );
    }
    let baseline = collect_play_sample(&controller, seat, seed).expect("baseline sample");
    assert!(
        baseline
            .candidates
            .iter()
            .all(|candidate| candidate.adviser_bias == 0),
        "baseline candidates should not include adviser bias when disabled"
    );

    let bias_path =
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/adviser_play_bias.json");
    unsafe {
        std::env::set_var("MDH_HARD_ADVISER_PLAY", "1");
        std::env::set_var("MDH_ADVISER_PLAY_PATH", &bias_path);
    }

    let controller = prepare_for_play(build_controller(seed), seat);
    let biased = collect_play_sample(&controller, seat, seed).expect("biased sample");
    let four_clubs = Card::new(Rank::Four, Suit::Clubs).to_string();
    let five_clubs = Card::new(Rank::Five, Suit::Clubs).to_string();
    assert!(
        biased
            .candidates
            .iter()
            .any(|candidate| candidate.card == four_clubs && candidate.adviser_bias == 5000),
        "expected 4C candidate to reflect configured adviser bias"
    );
    assert!(
        biased
            .candidates
            .iter()
            .any(|candidate| candidate.card == five_clubs && candidate.adviser_bias == 2500),
        "expected 5C candidate to reflect configured adviser bias"
    );

    unsafe {
        std::env::remove_var("MDH_HARD_ADVISER_PLAY");
        std::env::remove_var("MDH_ADVISER_PLAY_PATH");
        std::env::remove_var("MDH_SEARCH_MIX_HINT");
    }
}
