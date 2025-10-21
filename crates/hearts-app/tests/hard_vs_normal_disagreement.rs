use hearts_app::controller::GameController;
use hearts_core::model::card::Card;
use hearts_core::model::rank::Rank;
use hearts_core::model::suit::Suit;
use hearts_core::model::player::PlayerPosition;

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
        if normal.autoplay_one(seat).is_none() { break; }
    }
    let n_expl = normal.explain_candidates_for(seat);
    let n_top = n_expl.iter().max_by_key(|(_, s)| *s).map(|(c, _)| *c).unwrap();

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
        if hard.autoplay_one(seat).is_none() { break; }
    }
    let h_expl = hard.explain_candidates_for(seat);
    let h_top = h_expl.iter().max_by_key(|(_, s)| *s).map(|(c, _)| *c).unwrap();

    assert_ne!(n_top, h_top, "Expected a disagreement between Normal and Hard top choices");

    // Current known golden: Normal=10♠, Hard=2♠ for this snapshot.
    let ten_spades = Card { rank: Rank::Ten, suit: Suit::Spades };
    let two_spades = Card { rank: Rank::Two, suit: Suit::Spades };
    assert_eq!(n_top, ten_spades, "Normal top changed for seed {}; update golden if intended", seed);
    assert_eq!(h_top, two_spades, "Hard top changed for seed {}; update golden if intended", seed);
}
