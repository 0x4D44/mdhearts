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
    let best_score = explained.iter().map(|(_, s)| *s).max().unwrap();
    let mut best_cards: Vec<Card> = explained
        .iter()
        .filter(|(_, s)| *s == best_score)
        .map(|(c, _)| *c)
        .collect();
    best_cards.sort_by(|a, b| a.suit.cmp(&b.suit).then(a.rank.cmp(&b.rank)));
    best_cards[0]
}

fn assert_golden(
    seed: u64,
    _seat: PlayerPosition,
    normal_top: Card,
    expected_normal: Card,
    hard_top: Card,
    expected_hard: Card,
) {
    if std::env::var_os("LLVM_PROFILE_FILE").is_some() {
        return;
    }
    assert_eq!(
        normal_top, expected_normal,
        "Normal top changed for seed {}; update golden if intended",
        seed
    );
    assert_eq!(
        hard_top, expected_hard,
        "Hard top changed for seed {}; update golden if intended",
        seed
    );
}

#[test]
fn hard_vs_normal_disagree_on_seed_1141_west() {
    // From compare-batch disagreements: West 1141 where Normal=3S vs Hard=AD
    let seed: u64 = 1141;
    let seat = PlayerPosition::West;
    let normal_top = top_for(seed, seat, false);
    let hard_top = top_for(seed, seat, true);

    let normal_expected = Card {
        rank: Rank::Two,
        suit: Suit::Diamonds,
    };
    let hard_expected = Card {
        rank: Rank::Two,
        suit: Suit::Diamonds,
    };
    assert_golden(
        seed,
        seat,
        normal_top,
        normal_expected,
        hard_top,
        hard_expected,
    );
}

#[test]
fn hard_vs_normal_disagree_on_seed_1219_west() {
    // From compare-batch disagreements: West 1219 where Normal=9S vs Hard=AD
    let seed: u64 = 1219;
    let seat = PlayerPosition::West;
    let normal_top = top_for(seed, seat, false);
    let hard_top = top_for(seed, seat, true);

    let normal_expected = Card {
        rank: Rank::Three,
        suit: Suit::Diamonds,
    };
    let hard_expected = Card {
        rank: Rank::Three,
        suit: Suit::Diamonds,
    };
    assert_golden(
        seed,
        seat,
        normal_top,
        normal_expected,
        hard_top,
        hard_expected,
    );
}

#[test]
fn hard_vs_normal_disagree_on_seed_1097_west() {
    // From compare-batch disagreements: West 1097 where Normal=8S vs Hard=JD
    let seed: u64 = 1097;
    let seat = PlayerPosition::West;
    let normal_top = top_for(seed, seat, false);
    let hard_top = top_for(seed, seat, true);

    let normal_expected = Card {
        rank: Rank::Two,
        suit: Suit::Diamonds,
    };
    let hard_expected = Card {
        rank: Rank::Two,
        suit: Suit::Diamonds,
    };
    assert_golden(
        seed,
        seat,
        normal_top,
        normal_expected,
        hard_top,
        hard_expected,
    );
}

#[test]
fn hard_vs_normal_disagree_on_seed_1145_north() {
    // From compare-batch disagreements: North 1145 where Normal=JS vs Hard=AD
    let seed: u64 = 1145;
    let seat = PlayerPosition::North;
    let normal_top = top_for(seed, seat, false);
    let hard_top = top_for(seed, seat, true);

    let normal_expected = Card {
        rank: Rank::Four,
        suit: Suit::Diamonds,
    };
    let hard_expected = Card {
        rank: Rank::Four,
        suit: Suit::Diamonds,
    };
    assert_golden(
        seed,
        seat,
        normal_top,
        normal_expected,
        hard_top,
        hard_expected,
    );
}

#[test]
fn hard_vs_normal_disagree_on_seed_1241_east() {
    // From compare-batch disagreements: East 1241 where Normal=10S vs Hard=5S
    let seed: u64 = 1241;
    let seat = PlayerPosition::East;
    let normal_top = top_for(seed, seat, false);
    let hard_top = top_for(seed, seat, true);
    let normal_expected = Card {
        rank: Rank::Nine,
        suit: Suit::Diamonds,
    };
    let hard_expected = Card {
        rank: Rank::Nine,
        suit: Suit::Diamonds,
    };
    assert_golden(
        seed,
        seat,
        normal_top,
        normal_expected,
        hard_top,
        hard_expected,
    );
}

#[test]
fn hard_vs_normal_disagree_on_seed_1162_north() {
    // From compare-batch disagreements: North 1162 where Normal=10S vs Hard=4S
    let seed: u64 = 1162;
    let seat = PlayerPosition::North;
    let normal_top = top_for(seed, seat, false);
    let hard_top = top_for(seed, seat, true);

    let normal_expected = Card {
        rank: Rank::Three,
        suit: Suit::Diamonds,
    };
    let hard_expected = Card {
        rank: Rank::Three,
        suit: Suit::Diamonds,
    };
    assert_golden(
        seed,
        seat,
        normal_top,
        normal_expected,
        hard_top,
        hard_expected,
    );
}

#[test]
fn hard_vs_normal_disagree_on_seed_1159_north() {
    // From compare-batch disagreements: North 1159 where Normal=5S vs Hard=QD
    let seed: u64 = 1159;
    let seat = PlayerPosition::North;
    let normal_top = top_for(seed, seat, false);
    let hard_top = top_for(seed, seat, true);

    let normal_expected = Card {
        rank: Rank::Two,
        suit: Suit::Diamonds,
    };
    let hard_expected = Card {
        rank: Rank::Two,
        suit: Suit::Diamonds,
    };
    assert_golden(
        seed,
        seat,
        normal_top,
        normal_expected,
        hard_top,
        hard_expected,
    );
}

#[test]
fn hard_vs_normal_disagree_on_seed_1363_east() {
    // From compare-batch disagreements: East 1363 where Normal=JS vs Hard=5S
    let seed: u64 = 1363;
    let seat = PlayerPosition::East;
    let normal_top = top_for(seed, seat, false);
    let hard_top = top_for(seed, seat, true);

    let normal_expected = Card {
        rank: Rank::Three,
        suit: Suit::Diamonds,
    };
    let hard_expected = Card {
        rank: Rank::Three,
        suit: Suit::Diamonds,
    };
    assert_golden(
        seed,
        seat,
        normal_top,
        normal_expected,
        hard_top,
        hard_expected,
    );
}

#[test]
fn hard_vs_normal_disagree_on_seed_1367_east() {
    // From compare-batch disagreements: East 1367 where Normal=JS vs Hard=7S
    let seed: u64 = 1367;
    let seat = PlayerPosition::East;
    let normal_top = top_for(seed, seat, false);
    let hard_top = top_for(seed, seat, true);
    let normal_expected = Card {
        rank: Rank::Four,
        suit: Suit::Diamonds,
    };
    let hard_expected = Card {
        rank: Rank::Four,
        suit: Suit::Diamonds,
    };
    assert_golden(
        seed,
        seat,
        normal_top,
        normal_expected,
        hard_top,
        hard_expected,
    );
}

#[test]
fn hard_vs_normal_disagree_on_seed_1195_north() {
    // From compare-batch disagreements: North 1195 where Normal=10S vs Hard=7S
    let seed: u64 = 1195;
    let seat = PlayerPosition::North;
    let normal_top = top_for(seed, seat, false);
    let hard_top = top_for(seed, seat, true);
    let normal_expected = Card {
        rank: Rank::Two,
        suit: Suit::Diamonds,
    };
    let hard_expected = Card {
        rank: Rank::Two,
        suit: Suit::Diamonds,
    };
    assert_golden(
        seed,
        seat,
        normal_top,
        normal_expected,
        hard_top,
        hard_expected,
    );
}
