use hearts_app::bot::{BotContext, BotDifficulty, PlayPlannerHard, UnseenTracker};
use hearts_core::model::card::Card;
use hearts_core::model::hand::Hand;
use hearts_core::model::passing::PassingDirection;
use hearts_core::model::player::PlayerPosition;
use hearts_core::model::rank::Rank;
use hearts_core::model::round::{RoundPhase, RoundState};
use hearts_core::model::score::ScoreBoard;
use hearts_core::model::suit::Suit;

fn build_endgame_2120_west() -> (RoundState, ScoreBoard) {
    // Exported from --export-endgame 2120 west: leader=West, hearts_broken=true
    // Hands: E: [5H,6H,7H], N: [4S,10S,AS], S: [9H,10H,JH], W: [9S,JS,KS]
    let east = vec![
        Card::new(Rank::Five, Suit::Hearts),
        Card::new(Rank::Six, Suit::Hearts),
        Card::new(Rank::Seven, Suit::Hearts),
    ];
    let north = vec![
        Card::new(Rank::Four, Suit::Spades),
        Card::new(Rank::Ten, Suit::Spades),
        Card::new(Rank::Ace, Suit::Spades),
    ];
    let south = vec![
        Card::new(Rank::Nine, Suit::Hearts),
        Card::new(Rank::Ten, Suit::Hearts),
        Card::new(Rank::Jack, Suit::Hearts),
    ];
    let west = vec![
        Card::new(Rank::Nine, Suit::Spades),
        Card::new(Rank::Jack, Suit::Spades),
        Card::new(Rank::King, Suit::Spades),
    ];
    let mut hands = [Hand::new(), Hand::new(), Hand::new(), Hand::new()];
    hands[PlayerPosition::North.index()] = Hand::with_cards(north);
    hands[PlayerPosition::East.index()] = Hand::with_cards(east);
    hands[PlayerPosition::South.index()] = Hand::with_cards(south);
    hands[PlayerPosition::West.index()] = Hand::with_cards(west);
    let current = hearts_core::model::trick::Trick::new(PlayerPosition::West);
    // Seed previous trick (non-penalty) to avoid first-trick special-casing
    let mut prev = hearts_core::model::trick::Trick::new(PlayerPosition::North);
    let _ = prev.play(PlayerPosition::North, Card::new(Rank::Nine, Suit::Diamonds));
    let _ = prev.play(PlayerPosition::East, Card::new(Rank::Ten, Suit::Diamonds));
    let _ = prev.play(PlayerPosition::South, Card::new(Rank::Jack, Suit::Diamonds));
    let _ = prev.play(PlayerPosition::West, Card::new(Rank::Queen, Suit::Diamonds));
    let round = RoundState::from_hands_with_state(
        hands,
        PlayerPosition::West,
        PassingDirection::Hold,
        RoundPhase::Playing,
        current,
        vec![prev],
        true,
    );
    let mut scores = ScoreBoard::new();
    scores.set_totals([90, 10, 20, 30]); // North as score leader for feed targeting
    (round, scores)
}

#[test]
#[ignore]
fn hard_endgame_dp_minimal_w2120_flip_difference() {
    // Deterministic with boosted endgame-only continuation to surface DP influence
    // (matches env used by DP flip seeker for seed 2120/west).
    unsafe {
        std::env::set_var("MDH_HARD_DETERMINISTIC", "1");
        std::env::set_var("MDH_HARD_TEST_STEPS", "120");
        // Endgame-only tuning knobs
        std::env::set_var("MDH_HARD_ENDGAME_DP_ENABLE", "1");
        std::env::set_var("MDH_HARD_ENDGAME_MAX_CARDS", "3");
        // Boosted continuation cap and next-trick continuation
        std::env::set_var("MDH_HARD_CONT_CAP", "1200");
        std::env::set_var("MDH_HARD_NEXT2_FEED_PERPEN", "400");
        std::env::set_var("MDH_HARD_NEXT2_SELF_CAPTURE_PERPEN", "60");
        // Penalize handing off control
        std::env::set_var("MDH_HARD_CTRL_HANDOFF_PEN", "150");
        // Use default current-trick continuation parts
        std::env::remove_var("MDH_HARD_CONT_FEED_PERPEN");
        std::env::remove_var("MDH_HARD_CONT_SELF_CAPTURE_PERPEN");
    }
    let seat = PlayerPosition::West;
    let (round, scores) = build_endgame_2120_west();
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
    assert_eq!(legal.len(), 3);
    unsafe { std::env::remove_var("MDH_HARD_ENDGAME_DP_ENABLE"); }
    let off = PlayPlannerHard::choose(&legal, &ctx);
    unsafe { std::env::set_var("MDH_HARD_ENDGAME_DP_ENABLE", "1"); }
    let on = PlayPlannerHard::choose(&legal, &ctx);
    // Require difference; if this flakes under current caps, we will adjust or gate narrowly.
    assert_ne!(off, on, "expected DP to change top choice in minimal w2120 endgame");
    unsafe {
        std::env::remove_var("MDH_HARD_DETERMINISTIC");
        std::env::remove_var("MDH_HARD_TEST_STEPS");
        std::env::remove_var("MDH_HARD_ENDGAME_DP_ENABLE");
        std::env::remove_var("MDH_HARD_CONT_CAP");
        std::env::remove_var("MDH_HARD_ENDGAME_MAX_CARDS");
        std::env::remove_var("MDH_HARD_NEXT2_FEED_PERPEN");
        std::env::remove_var("MDH_HARD_NEXT2_SELF_CAPTURE_PERPEN");
        std::env::remove_var("MDH_HARD_CTRL_HANDOFF_PEN");
        std::env::remove_var("MDH_HARD_CONT_FEED_PERPEN");
        std::env::remove_var("MDH_HARD_CONT_SELF_CAPTURE_PERPEN");
    }
}
