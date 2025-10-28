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
    let explained = controller.explain_candidates_for(seat);
    explained
        .iter()
        .max_by_key(|(_, s)| *s)
        .map(|(c, _)| *c)
        .expect("has top candidate")
}

#[test]
fn west_seed_1082_normal_10s_hard_2s() {
    let seed: u64 = 1082;
    let seat = PlayerPosition::West;
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
    assert_eq!(n_top, Card::new(Rank::Ten, Suit::Spades));
    assert_eq!(h_top, Card::new(Rank::Two, Suit::Spades));
}

#[test]
fn west_seed_1097_normal_8s_hard_jd() {
    let seed: u64 = 1097;
    let seat = PlayerPosition::West;
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
    assert_eq!(n_top, Card::new(Rank::Eight, Suit::Spades));
    assert_eq!(h_top, Card::new(Rank::Jack, Suit::Diamonds));
}
