use hearts_app::bot::{BotContext, BotDifficulty, PlayPlannerHard, UnseenTracker};
use hearts_core::model::card::Card;
use hearts_core::model::hand::Hand;
use hearts_core::model::passing::PassingDirection;
use hearts_core::model::player::PlayerPosition;
use hearts_core::model::rank::Rank;
use hearts_core::model::round::{RoundPhase, RoundState};
use hearts_core::model::score::ScoreBoard;
use hearts_core::model::suit::Suit;
use std::collections::HashMap;

fn build_two_trick_endgame(seat: PlayerPosition) -> (RoundState, ScoreBoard) {
    // 2 cards per hand; hearts broken. East leads a fresh trick.
    // East: [A♣, 2♥]. Others have clubs to avoid feeding now when A♣ is led.
    // Next trick (after A♣ wins), East leads 2♥ and North (leader) holds K♥ to capture p>0 (DP should add positive).
    let east = vec![Card::new(Rank::Ace, Suit::Clubs), Card::new(Rank::Two, Suit::Hearts)];
    let north = vec![Card::new(Rank::King, Suit::Hearts), Card::new(Rank::Three, Suit::Clubs)];
    let south = vec![Card::new(Rank::Five, Suit::Hearts), Card::new(Rank::Jack, Suit::Clubs)];
    let west = vec![Card::new(Rank::Four, Suit::Hearts), Card::new(Rank::Queen, Suit::Clubs)];

    let mut hands = [Hand::new(), Hand::new(), Hand::new(), Hand::new()];
    hands[PlayerPosition::North.index()] = Hand::with_cards(north);
    hands[PlayerPosition::East.index()] = Hand::with_cards(east);
    hands[PlayerPosition::South.index()] = Hand::with_cards(south);
    hands[PlayerPosition::West.index()] = Hand::with_cards(west);

    let current = hearts_core::model::trick::Trick::new(seat);
    // Seed previous trick to avoid first-trick rules
    let mut prev = hearts_core::model::trick::Trick::new(seat);
    let _ = prev.play(seat, Card::new(Rank::Nine, Suit::Diamonds));
    let _ = prev.play(seat.next(), Card::new(Rank::Ten, Suit::Diamonds));
    let _ = prev.play(seat.next().next(), Card::new(Rank::Jack, Suit::Diamonds));
    let _ = prev.play(seat.next().next().next(), Card::new(Rank::Queen, Suit::Diamonds));

    let round = RoundState::from_hands_with_state(
        hands,
        seat,
        PassingDirection::Hold,
        RoundPhase::Playing,
        current,
        vec![prev],
        true, // hearts broken
    );
    let mut scores = ScoreBoard::new();
    // Make North the leader target on score
    scores.set_totals([90, 10, 20, 30]);
    (round, scores)
}

fn explain_map(legal: &[Card], ctx: &BotContext<'_>) -> HashMap<Card, i32> {
    PlayPlannerHard::explain_candidates(legal, ctx)
        .into_iter()
        .collect::<HashMap<_, _>>()
}

#[test]
fn hard_endgame_dp_explain_parity_in_explain() {
    let seat = PlayerPosition::East;
    let (round, scores) = build_two_trick_endgame(seat);
    let mut tracker = UnseenTracker::new();
    tracker.reset_for_round(&round);
    let ctx = BotContext::new(
        seat,
        &round,
        &scores,
        PassingDirection::Hold,
        &tracker,
        BotDifficulty::FutureHard,
    );
    let legal = round
        .hand(seat)
        .iter()
        .copied()
        .filter(|c| {
            let mut r = round.clone();
            r.play_card(seat, *c).is_ok()
        })
        .collect::<Vec<_>>();
    assert_eq!(legal.len(), 2);
    assert!(legal.contains(&Card::new(Rank::Ace, Suit::Clubs)));
    assert!(legal.contains(&Card::new(Rank::Two, Suit::Hearts)));

    unsafe {
        std::env::set_var("MDH_HARD_DETERMINISTIC", "1");
        std::env::set_var("MDH_HARD_TEST_STEPS", "120");
        std::env::set_var("MDH_HARD_PHASEB_TOPK", "8");
        std::env::set_var("MDH_HARD_BRANCH_LIMIT", "8");
        // Higher cap so DP’s continuation can surface in totals
        std::env::set_var("MDH_HARD_CONT_CAP", "500");
        // DP OFF first
        std::env::remove_var("MDH_HARD_ENDGAME_DP_ENABLE");
    }
    let off_map = explain_map(&legal, &ctx);

    unsafe {
        std::env::set_var("MDH_HARD_ENDGAME_DP_ENABLE", "1");
        std::env::set_var("MDH_HARD_ENDGAME_MAX_CARDS", "3");
    }
    let on_map = explain_map(&legal, &ctx);

    let a_clubs = Card::new(Rank::Ace, Suit::Clubs);
    let two_hearts = Card::new(Rank::Two, Suit::Hearts);
    let off_a = *off_map.get(&a_clubs).expect("A♣ present off");
    let on_a = *on_map.get(&a_clubs).expect("A♣ present on");
    let off_h = *off_map.get(&two_hearts).expect("2♥ present off");
    let on_h = *on_map.get(&two_hearts).expect("2♥ present on");

    // Explain path is deterministic and does not include endgame DP (choose-only feature),
    // so totals should be identical regardless of DP env.
    assert_eq!(on_a, off_a, "explain parity: A♣ totals should match");
    assert_eq!(on_h, off_h, "explain parity: 2♥ totals should match");

    unsafe {
        std::env::remove_var("MDH_HARD_DETERMINISTIC");
        std::env::remove_var("MDH_HARD_TEST_STEPS");
        std::env::remove_var("MDH_HARD_PHASEB_TOPK");
        std::env::remove_var("MDH_HARD_BRANCH_LIMIT");
        std::env::remove_var("MDH_HARD_CONT_CAP");
        std::env::remove_var("MDH_HARD_ENDGAME_DP_ENABLE");
        std::env::remove_var("MDH_HARD_ENDGAME_MAX_CARDS");
    }
}
