use hearts_app::bot::{BotDifficulty, PlayPlannerHard, UnseenTracker};
use hearts_core::model::card::Card;
use hearts_core::model::hand::Hand;
use hearts_core::model::passing::PassingDirection;
use hearts_core::model::player::PlayerPosition;
use hearts_core::model::rank::Rank;
use hearts_core::model::round::{RoundPhase, RoundState};
use hearts_core::model::score::ScoreBoard;
use hearts_core::model::suit::Suit;

fn build_two_trick_minimal(seat: PlayerPosition) -> (RoundState, ScoreBoard) {
    // East leads; 2 cards per hand; hearts broken.
    // East has A♣ and 2♥; North (leader) can capture next hearts trick with K♥.
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
    // Seed a previous non-penalty trick to avoid first-trick special rules
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
        true,
    );
    let mut scores = ScoreBoard::new();
    // Make North the score leader to target feeding
    scores.set_totals([90, 10, 20, 30]);
    (round, scores)
}

#[test]
#[ignore]
fn hard_endgame_dp_minimal_flip() {
    // Deterministic and endgame-only boosts to surface DP effect
    unsafe {
        std::env::set_var("MDH_HARD_DETERMINISTIC", "1");
        std::env::set_var("MDH_HARD_TEST_STEPS", "120");
        std::env::set_var("MDH_HARD_CONT_FEED_PERPEN", "0");
        std::env::set_var("MDH_HARD_CONT_CAP", "1200");
        std::env::set_var("MDH_HARD_NEXT2_FEED_PERPEN", "400");
        std::env::set_var("MDH_HARD_NEXT2_SELF_CAPTURE_PERPEN", "60");
        std::env::set_var("MDH_HARD_CTRL_HANDOFF_PEN", "150");
    }
    let seat = PlayerPosition::East;
    let (round, scores) = build_two_trick_minimal(seat);
    let mut tracker = UnseenTracker::new();
    tracker.reset_for_round(&round);
    let ctx = hearts_app::bot::BotContext::new(
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
    // OFF
    unsafe { std::env::remove_var("MDH_HARD_ENDGAME_DP_ENABLE"); }
    let off = PlayPlannerHard::choose(&legal, &ctx);
    // ON
    unsafe { std::env::set_var("MDH_HARD_ENDGAME_DP_ENABLE", "1"); }
    let on = PlayPlannerHard::choose(&legal, &ctx);
    assert_ne!(off, on, "expected DP to flip top under boosts in a minimal endgame");
    unsafe {
        std::env::remove_var("MDH_HARD_DETERMINISTIC");
        std::env::remove_var("MDH_HARD_TEST_STEPS");
        std::env::remove_var("MDH_HARD_ENDGAME_DP_ENABLE");
        std::env::remove_var("MDH_HARD_CONT_FEED_PERPEN");
        std::env::remove_var("MDH_HARD_CONT_CAP");
        std::env::remove_var("MDH_HARD_NEXT2_FEED_PERPEN");
        std::env::remove_var("MDH_HARD_NEXT2_SELF_CAPTURE_PERPEN");
        std::env::remove_var("MDH_HARD_CTRL_HANDOFF_PEN");
    }
}
