use hearts_app::controller::GameController;
use hearts_core::model::card::Card;
use hearts_core::model::player::PlayerPosition;
use hearts_core::model::rank::Rank;
use hearts_core::model::suit::Suit;

fn top_for(seed: u64, seat: PlayerPosition, hard: bool) -> Card {
    let mut ctrl = GameController::new_with_seed(Some(seed), PlayerPosition::North);
    if hard {
        ctrl.set_bot_difficulty(hearts_app::bot::BotDifficulty::FutureHard);
    }
    if ctrl.in_passing_phase() {
        if let Some(cards) = ctrl.simple_pass_for(seat) {
            let _ = ctrl.submit_pass(seat, cards);
        }
        let _ = ctrl.submit_auto_passes_for_others(seat);
        let _ = ctrl.resolve_passes();
    }
    while !ctrl.in_passing_phase() && ctrl.expected_to_play() != seat {
        if ctrl.autoplay_one(seat).is_none() {
            break;
        }
    }
    let explained = ctrl.explain_candidates_for(seat);
    explained
        .iter()
        .max_by_key(|(_, s)| *s)
        .map(|(c, _)| *c)
        .unwrap()
}

#[test]
fn hard_vs_normal_disagree_on_seed_1141_west() {
    // From compare-batch disagreements: West 1141 where Normal=3S vs Hard=AD
    let seed: u64 = 1141;
    let seat = PlayerPosition::West;
    let normal_top = top_for(seed, seat, false);
    let hard_top = top_for(seed, seat, true);
    assert_ne!(normal_top, hard_top, "Expected disagreement on seed {} {:?}", seed, seat);

    let three_spades = Card { rank: Rank::Three, suit: Suit::Spades };
    let ace_diamonds = Card { rank: Rank::Ace, suit: Suit::Diamonds };
    assert_eq!(
        normal_top, three_spades,
        "Normal top changed for seed {}; update golden if intended",
        seed
    );
    assert_eq!(
        hard_top, ace_diamonds,
        "Hard top changed for seed {}; update golden if intended",
        seed
    );
}

#[test]
fn hard_vs_normal_disagree_on_seed_1219_west() {
    // From compare-batch disagreements: West 1219 where Normal=9S vs Hard=AD
    let seed: u64 = 1219;
    let seat = PlayerPosition::West;
    let normal_top = top_for(seed, seat, false);
    let hard_top = top_for(seed, seat, true);
    assert_ne!(normal_top, hard_top, "Expected disagreement on seed {} {:?}", seed, seat);

    let nine_spades = Card { rank: Rank::Nine, suit: Suit::Spades };
    let ace_diamonds = Card { rank: Rank::Ace, suit: Suit::Diamonds };
    assert_eq!(normal_top, nine_spades, "Normal top changed for seed {}; update golden if intended", seed);
    assert_eq!(hard_top, ace_diamonds, "Hard top changed for seed {}; update golden if intended", seed);
}

#[test]
fn hard_vs_normal_disagree_on_seed_1097_west() {
    // From compare-batch disagreements: West 1097 where Normal=8S vs Hard=JD
    let seed: u64 = 1097;
    let seat = PlayerPosition::West;
    let normal_top = top_for(seed, seat, false);
    let hard_top = top_for(seed, seat, true);
    assert_ne!(normal_top, hard_top, "Expected disagreement on seed {} {:?}", seed, seat);

    let eight_spades = Card { rank: Rank::Eight, suit: Suit::Spades };
    let jack_diamonds = Card { rank: Rank::Jack, suit: Suit::Diamonds };
    assert_eq!(normal_top, eight_spades, "Normal top changed for seed {}; update golden if intended", seed);
    assert_eq!(hard_top, jack_diamonds, "Hard top changed for seed {}; update golden if intended", seed);
}

#[test]
fn hard_vs_normal_disagree_on_seed_1145_north() {
    // From compare-batch disagreements: North 1145 where Normal=JS vs Hard=AD
    let seed: u64 = 1145;
    let seat = PlayerPosition::North;
    let normal_top = top_for(seed, seat, false);
    let hard_top = top_for(seed, seat, true);
    assert_ne!(normal_top, hard_top, "Expected disagreement on seed {} {:?}", seed, seat);

    let jack_spades = Card { rank: Rank::Jack, suit: Suit::Spades };
    let ace_diamonds = Card { rank: Rank::Ace, suit: Suit::Diamonds };
    assert_eq!(normal_top, jack_spades, "Normal top changed for seed {}; update golden if intended", seed);
    assert_eq!(hard_top, ace_diamonds, "Hard top changed for seed {}; update golden if intended", seed);
}

#[test]
fn hard_vs_normal_disagree_on_seed_1241_east() {
    // From compare-batch disagreements: East 1241 where Normal=10S vs Hard=5S
    let seed: u64 = 1241;
    let seat = PlayerPosition::East;
    let normal_top = top_for(seed, seat, false);
    let hard_top = top_for(seed, seat, true);
    assert_ne!(normal_top, hard_top, "Expected disagreement on seed {} {:?}", seed, seat);

    let ten_spades = Card { rank: Rank::Ten, suit: Suit::Spades };
    let five_spades = Card { rank: Rank::Five, suit: Suit::Spades };
    assert_eq!(normal_top, ten_spades, "Normal top changed for seed {}; update golden if intended", seed);
    assert_eq!(hard_top, five_spades, "Hard top changed for seed {}; update golden if intended", seed);
}

#[test]
fn hard_vs_normal_disagree_on_seed_1162_north() {
    // From compare-batch disagreements: North 1162 where Normal=10S vs Hard=4S
    let seed: u64 = 1162;
    let seat = PlayerPosition::North;
    let normal_top = top_for(seed, seat, false);
    let hard_top = top_for(seed, seat, true);
    assert_ne!(normal_top, hard_top, "Expected disagreement on seed {} {:?}", seed, seat);

    let ten_spades = Card { rank: Rank::Ten, suit: Suit::Spades };
    let four_spades = Card { rank: Rank::Four, suit: Suit::Spades };
    assert_eq!(normal_top, ten_spades, "Normal top changed for seed {}; update golden if intended", seed);
    assert_eq!(hard_top, four_spades, "Hard top changed for seed {}; update golden if intended", seed);
}

#[test]
fn hard_vs_normal_disagree_on_seed_1159_north() {
    // From compare-batch disagreements: North 1159 where Normal=5S vs Hard=QD
    let seed: u64 = 1159;
    let seat = PlayerPosition::North;
    let normal_top = top_for(seed, seat, false);
    let hard_top = top_for(seed, seat, true);
    assert_ne!(normal_top, hard_top, "Expected disagreement on seed {} {:?}", seed, seat);

    let five_spades = Card { rank: Rank::Five, suit: Suit::Spades };
    let queen_diamonds = Card { rank: Rank::Queen, suit: Suit::Diamonds };
    assert_eq!(normal_top, five_spades, "Normal top changed for seed {}; update golden if intended", seed);
    assert_eq!(hard_top, queen_diamonds, "Hard top changed for seed {}; update golden if intended", seed);
}

#[test]
fn hard_vs_normal_disagree_on_seed_1363_east() {
    // From compare-batch disagreements: East 1363 where Normal=JS vs Hard=5S
    let seed: u64 = 1363;
    let seat = PlayerPosition::East;
    let normal_top = top_for(seed, seat, false);
    let hard_top = top_for(seed, seat, true);
    assert_ne!(normal_top, hard_top, "Expected disagreement on seed {} {:?}", seed, seat);

    let jack_spades = Card { rank: Rank::Jack, suit: Suit::Spades };
    let five_spades = Card { rank: Rank::Five, suit: Suit::Spades };
    assert_eq!(normal_top, jack_spades, "Normal top changed for seed {}; update golden if intended", seed);
    assert_eq!(hard_top, five_spades, "Hard top changed for seed {}; update golden if intended", seed);
}

#[test]
fn hard_vs_normal_disagree_on_seed_1367_east() {
    // From compare-batch disagreements: East 1367 where Normal=JS vs Hard=7S
    let seed: u64 = 1367;
    let seat = PlayerPosition::East;
    let normal_top = top_for(seed, seat, false);
    let hard_top = top_for(seed, seat, true);
    assert_ne!(normal_top, hard_top, "Expected disagreement on seed {} {:?}", seed, seat);

    let jack_spades = Card { rank: Rank::Jack, suit: Suit::Spades };
    let seven_spades = Card { rank: Rank::Seven, suit: Suit::Spades };
    assert_eq!(normal_top, jack_spades, "Normal top changed for seed {}; update golden if intended", seed);
    assert_eq!(hard_top, seven_spades, "Hard top changed for seed {}; update golden if intended", seed);
}

#[test]
fn hard_vs_normal_disagree_on_seed_1195_north() {
    // From compare-batch disagreements: North 1195 where Normal=10S vs Hard=7S
    let seed: u64 = 1195;
    let seat = PlayerPosition::North;
    let normal_top = top_for(seed, seat, false);
    let hard_top = top_for(seed, seat, true);
    assert_ne!(normal_top, hard_top, "Expected disagreement on seed {} {:?}", seed, seat);

    let ten_spades = Card { rank: Rank::Ten, suit: Suit::Spades };
    let seven_spades = Card { rank: Rank::Seven, suit: Suit::Spades };
    assert_eq!(normal_top, ten_spades, "Normal top changed for seed {}; update golden if intended", seed);
    assert_eq!(hard_top, seven_spades, "Hard top changed for seed {}; update golden if intended", seed);
}
