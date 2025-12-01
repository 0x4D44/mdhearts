use hearts_app::controller::GameController;
use hearts_core::model::card::Card;
use hearts_core::model::player::PlayerPosition;
use hearts_core::model::rank::Rank;
use hearts_core::model::suit::Suit;

#[test]
fn west_seed_1141_expected_tops_under_adaptive_and_sampling() {
    // Enable the gated Hard features used to derive the CSV disagreement.
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

    assert_eq!(
        n_top,
        Card::new(Rank::Three, Suit::Spades),
        "Normal top changed; update golden if intentional"
    );
    let expected = [
        Card::new(Rank::Four, Suit::Diamonds),
        Card::new(Rank::Ace, Suit::Diamonds),
    ];
    assert!(
        expected.contains(&h_top),
        "Hard top changed; got {:?}; update golden if intentional",
        h_top
    );
}

#[test]
fn west_seed_1082_expected_tops_under_adaptive_and_sampling() {
    unsafe {
        std::env::set_var("MDH_HARD_ADAPTIVE", "1");
        std::env::set_var("MDH_HARD_THIRD_BRANCH", "1");
        std::env::set_var("MDH_HARD_SAMPLE", "1");
        std::env::set_var("MDH_HARD_SAMPLE_N", "2");
    }

    let seed: u64 = 1082;
    let seat = PlayerPosition::West;

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
    let n_top = normal
        .explain_candidates_for(seat)
        .into_iter()
        .max_by_key(|(_, s)| s.clone())
        .map(|(c, _)| c)
        .unwrap();

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
    let h_top = hard
        .explain_candidates_for(seat)
        .into_iter()
        .max_by_key(|(_, s)| s.clone())
        .map(|(c, _)| c)
        .unwrap();

    assert_eq!(
        n_top,
        Card::new(Rank::Ten, Suit::Spades),
        "Normal top changed; update golden if intentional"
    );
    let expected = [
        Card::new(Rank::Jack, Suit::Diamonds),
        Card::new(Rank::Two, Suit::Spades),
    ];
    assert!(
        expected.contains(&h_top),
        "Hard top changed; got {:?}; update golden if intentional",
        h_top
    );
}
