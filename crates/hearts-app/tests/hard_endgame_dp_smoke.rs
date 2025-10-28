use hearts_app::bot::{BotContext, BotDifficulty, PlayPlannerHard, UnseenTracker};
use hearts_core::model::card::Card;
use hearts_core::model::hand::Hand;
use hearts_core::model::passing::PassingDirection;
use hearts_core::model::player::PlayerPosition;
use hearts_core::model::rank::Rank;
use hearts_core::model::round::{RoundPhase, RoundState};
use hearts_core::model::score::ScoreBoard;
use hearts_core::model::suit::Suit;

fn build_small_endgame(_seat: PlayerPosition) -> (RoundState, ScoreBoard) {
    // <=3 cards per hand, hearts broken
    let leader = PlayerPosition::North;
    let north_cards = vec![
        Card::new(Rank::Two, Suit::Clubs),
        Card::new(Rank::Three, Suit::Clubs),
    ];
    let east_cards = vec![
        Card::new(Rank::Two, Suit::Spades),
        Card::new(Rank::Three, Suit::Spades),
    ];
    let south_cards = vec![
        Card::new(Rank::Two, Suit::Hearts),
        Card::new(Rank::Three, Suit::Hearts),
    ];
    let west_cards = vec![
        Card::new(Rank::Two, Suit::Diamonds),
        Card::new(Rank::Three, Suit::Diamonds),
    ];
    let mut hands = [Hand::new(), Hand::new(), Hand::new(), Hand::new()];
    hands[PlayerPosition::North.index()] = Hand::with_cards(north_cards);
    hands[PlayerPosition::East.index()] = Hand::with_cards(east_cards);
    hands[PlayerPosition::South.index()] = Hand::with_cards(south_cards);
    hands[PlayerPosition::West.index()] = Hand::with_cards(west_cards);
    let mut current = hearts_core::model::trick::Trick::new(leader);
    current
        .play(leader, Card::new(Rank::Four, Suit::Clubs))
        .unwrap();
    // Seed previous trick to break hearts
    let mut prev = hearts_core::model::trick::Trick::new(leader);
    prev.play(leader, Card::new(Rank::Five, Suit::Hearts))
        .unwrap();
    prev.play(leader.next(), Card::new(Rank::Six, Suit::Hearts))
        .unwrap();
    prev.play(leader.next().next(), Card::new(Rank::Seven, Suit::Hearts))
        .unwrap();
    prev.play(
        leader.next().next().next(),
        Card::new(Rank::Eight, Suit::Hearts),
    )
    .unwrap();
    let round = RoundState::from_hands_with_state(
        hands,
        leader,
        PassingDirection::Hold,
        RoundPhase::Playing,
        current,
        vec![prev],
        true,
    );
    let mut scores = ScoreBoard::new();
    scores.set_totals([40, 60, 50, 45]);
    (round, scores)
}

#[test]
fn hard_endgame_dp_smoke() {
    let (round, scores) = build_small_endgame(PlayerPosition::East);
    let seat = PlayerPosition::East;
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
    assert!(!legal.is_empty());
    unsafe {
        std::env::set_var("MDH_HARD_DETERMINISTIC", "1");
        std::env::set_var("MDH_HARD_TEST_STEPS", "80");
        std::env::set_var("MDH_HARD_ENDGAME_DP_ENABLE", "1");
        std::env::set_var("MDH_HARD_ENDGAME_MAX_CARDS", "3");
        // keep bonus at 0 by default; this is a smoke for path execution
        std::env::remove_var("MDH_HARD_ENDGAME_BONUS");
    }
    let _ = PlayPlannerHard::explain_candidates(&legal, &ctx);
    // choose path should also run without panicking
    let _ = hearts_app::bot::PlayPlannerHard::choose(&legal, &ctx);
    unsafe {
        std::env::remove_var("MDH_HARD_DETERMINISTIC");
        std::env::remove_var("MDH_HARD_TEST_STEPS");
        std::env::remove_var("MDH_HARD_ENDGAME_DP_ENABLE");
        std::env::remove_var("MDH_HARD_ENDGAME_MAX_CARDS");
    }
}
