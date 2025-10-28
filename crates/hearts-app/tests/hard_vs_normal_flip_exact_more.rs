use hearts_app::controller::GameController;
use hearts_core::model::card::Card;
use hearts_core::model::player::PlayerPosition;
use hearts_core::model::rank::Rank;
use hearts_core::model::suit::Suit;

fn assert_exact_flip(seed: u64, seat: PlayerPosition, expected_normal: Card, expected_hard: Card) {
    unsafe {
        std::env::set_var("MDH_HARD_DETERMINISTIC", "1");
        std::env::set_var("MDH_HARD_TEST_STEPS", "80");
    }

    let mut normal = GameController::new_with_seed(Some(seed), PlayerPosition::North);
    normal.set_bot_difficulty(hearts_app::bot::BotDifficulty::NormalHeuristic);
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
        expected_normal,
        "Normal top changed for {}/{}",
        seed,
        format!("{:?}", seat)
    );
    assert_eq!(
        h_top,
        expected_hard,
        "Hard top changed for {}/{}",
        seed,
        format!("{:?}", seat)
    );

    unsafe {
        std::env::remove_var("MDH_HARD_DETERMINISTIC");
        std::env::remove_var("MDH_HARD_TEST_STEPS");
    }
}

#[test]
fn exact_flip_seed_1145_north() {
    // From deterministic compare: Normal=J♠, Hard=A♦
    assert_exact_flip(
        1145,
        PlayerPosition::North,
        Card::new(Rank::Jack, Suit::Spades),
        Card::new(Rank::Ace, Suit::Diamonds),
    );
}

#[test]
fn exact_flip_seed_1080_south() {
    // From deterministic compare: Normal=J♠, Hard=7♠
    assert_exact_flip(
        1080,
        PlayerPosition::South,
        Card::new(Rank::Jack, Suit::Spades),
        Card::new(Rank::Seven, Suit::Spades),
    );
}

#[test]
fn exact_flip_seed_2044_east() {
    // From deterministic compare: Normal=9♠, Hard=2♠
    assert_exact_flip(
        2044,
        PlayerPosition::East,
        Card::new(Rank::Nine, Suit::Spades),
        Card::new(Rank::Two, Suit::Spades),
    );
}

#[test]
fn exact_flip_seed_1040_west() {
    // From deterministic compare: Normal=10♠, Hard=2♠
    assert_exact_flip(
        1040,
        PlayerPosition::West,
        Card::new(Rank::Ten, Suit::Spades),
        Card::new(Rank::Two, Suit::Spades),
    );
}
