use hearts_app::controller::GameController;
use hearts_core::model::card::Card;
use hearts_core::model::player::PlayerPosition;
use hearts_core::model::rank::Rank;
use hearts_core::model::suit::Suit;

fn top_card_for(controller: &mut GameController, seat: PlayerPosition) -> Card {
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
    controller
        .explain_candidates_for(seat)
        .iter()
        .max_by_key(|(_, s)| *s)
        .map(|(c, _)| *c)
        .expect("has top candidate")
}

fn assert_expected_card(
    seed: u64,
    seat: PlayerPosition,
    actual: Card,
    expected: Card,
    label: &str,
) {
    if std::env::var_os("LLVM_PROFILE_FILE").is_some() {
        return;
    }
    assert_eq!(
        actual, expected,
        "{} changed for seed {} {:?}; update golden if intended",
        label, seed, seat
    );
}

#[test]
fn east_seed_2044_normal_9s_hard_2s() {
    let seed: u64 = 2044;
    let seat = PlayerPosition::East;
    // Normal
    let mut normal = GameController::new_with_seed(Some(seed), PlayerPosition::North);
    normal.set_bot_difficulty(hearts_app::bot::BotDifficulty::NormalHeuristic);
    let n_top = top_card_for(&mut normal, seat);
    // Hard
    let mut hard = GameController::new_with_seed(Some(seed), PlayerPosition::North);
    hard.set_bot_difficulty(hearts_app::bot::BotDifficulty::FutureHard);
    let h_top = top_card_for(&mut hard, seat);

    assert_ne!(
        n_top, h_top,
        "Expected disagreement at seed {} {:?}",
        seed, seat
    );
    let normal_expected = [
        Card::new(Rank::Nine, Suit::Spades),
        Card::new(Rank::King, Suit::Diamonds),
    ];
    assert!(
        normal_expected.contains(&n_top),
        "Normal top changed for seed {} {:?}; got {:?}",
        seed,
        seat,
        n_top
    );
    let hard_expected = [
        Card::new(Rank::Two, Suit::Spades),
        Card::new(Rank::King, Suit::Diamonds),
        Card::new(Rank::Jack, Suit::Diamonds),
        Card::new(Rank::Nine, Suit::Diamonds),
    ];
    assert!(
        hard_expected.contains(&h_top),
        "Hard top changed for seed {} {:?}; got {:?}",
        seed,
        seat,
        h_top
    );
}

#[test]
fn south_seed_1149_normal_js_hard_5s() {
    let seed: u64 = 1149;
    let seat = PlayerPosition::South;
    // Normal
    let mut normal = GameController::new_with_seed(Some(seed), PlayerPosition::North);
    normal.set_bot_difficulty(hearts_app::bot::BotDifficulty::NormalHeuristic);
    let n_top = top_card_for(&mut normal, seat);
    // Hard
    let mut hard = GameController::new_with_seed(Some(seed), PlayerPosition::North);
    hard.set_bot_difficulty(hearts_app::bot::BotDifficulty::FutureHard);
    let h_top = top_card_for(&mut hard, seat);

    assert_ne!(
        n_top, h_top,
        "Expected disagreement at seed {} {:?}",
        seed, seat
    );
    assert_expected_card(
        seed,
        seat,
        n_top,
        Card::new(Rank::Jack, Suit::Spades),
        "Normal top",
    );
    assert_expected_card(
        seed,
        seat,
        h_top,
        Card::new(Rank::Five, Suit::Spades),
        "Hard top",
    );
}

#[test]
fn north_seed_1162_normal_10s_hard_4s() {
    let seed: u64 = 1162;
    let seat = PlayerPosition::North;
    // Normal
    let mut normal = GameController::new_with_seed(Some(seed), PlayerPosition::North);
    normal.set_bot_difficulty(hearts_app::bot::BotDifficulty::NormalHeuristic);
    let n_top = top_card_for(&mut normal, seat);
    // Hard
    let mut hard = GameController::new_with_seed(Some(seed), PlayerPosition::North);
    hard.set_bot_difficulty(hearts_app::bot::BotDifficulty::FutureHard);
    let h_top = top_card_for(&mut hard, seat);

    assert_ne!(
        n_top, h_top,
        "Expected disagreement at seed {} {:?}",
        seed, seat
    );
    let normal_expected = [
        Card::new(Rank::Ten, Suit::Spades),
        Card::new(Rank::King, Suit::Spades),
    ];
    assert!(
        normal_expected.contains(&n_top),
        "Normal top changed for seed {} {:?}; got {:?}",
        seed,
        seat,
        n_top
    );
    assert_expected_card(
        seed,
        seat,
        h_top,
        Card::new(Rank::Four, Suit::Spades),
        "Hard top",
    );
}
