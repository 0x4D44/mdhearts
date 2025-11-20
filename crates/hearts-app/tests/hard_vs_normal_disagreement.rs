use hearts_app::controller::GameController;
use hearts_core::model::card::Card;
use hearts_core::model::player::PlayerPosition;
use hearts_core::model::rank::Rank;
use hearts_core::model::suit::Suit;

#[test]
fn hard_vs_normal_disagree_on_seed_1040_west() {
    let seed: u64 = 1040;
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

    // Hard
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
        "Expected a disagreement between Normal and Hard top choices"
    );

    // Current known golden: Normal=K?, Hard=2? for this snapshot.
    let ten_spades = Card::new(Rank::Ten, Suit::Spades);
    let king_spades = Card::new(Rank::King, Suit::Spades);
    let two_spades = Card::new(Rank::Two, Suit::Spades);
    let normal_candidates = [ten_spades, king_spades];
    assert!(
        normal_candidates.contains(&n_top),
        "Normal top changed for seed {}; got {:?}",
        seed,
        n_top
    );
    let hard_candidates = [two_spades, Card::new(Rank::King, Suit::Diamonds)];
    assert!(
        hard_candidates.contains(&h_top),
        "Hard top changed for seed {}; got {:?}",
        seed,
        h_top
    );
}

#[test]
fn hard_vs_normal_disagree_on_seed_1080_south() {
    let seed: u64 = 1080;
    let seat = PlayerPosition::South;

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

    // Hard
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
        "Expected a disagreement between Normal and Hard top choices (seed {} {:?})",
        seed, seat
    );
}

#[test]
fn hard_vs_normal_disagree_on_seed_1145_north() {
    let seed: u64 = 1145;
    let seat = PlayerPosition::North;

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
        "Expected a disagreement between Normal and Hard top choices (seed {} {:?})",
        seed, seat
    );
}

#[test]
fn hard_vs_normal_disagree_on_seed_2031_east() {
    let seed: u64 = 2031;
    let seat = PlayerPosition::East;

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
        "Expected a disagreement between Normal and Hard top choices (seed {} {:?})",
        seed, seat
    );
}
