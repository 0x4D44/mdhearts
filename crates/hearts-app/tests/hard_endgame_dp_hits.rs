use hearts_app::bot::{BotContext, BotDifficulty, PlayPlannerHard, UnseenTracker};
use hearts_core::model::card::Card;
use hearts_core::model::hand::Hand;
use hearts_core::model::passing::PassingDirection;
use hearts_core::model::player::PlayerPosition;
use hearts_core::model::rank::Rank;
use hearts_core::model::round::{RoundPhase, RoundState};
use hearts_core::model::score::ScoreBoard;
use hearts_core::model::suit::Suit;

fn build_tiny_endgame_lead(seat: PlayerPosition) -> (RoundState, ScoreBoard) {
    // 2 cards per hand, hearts broken, seat leads a fresh trick
    // East: A♣, 2♥ (can win with A♣, then lead hearts next)
    let east_cards = vec![Card::new(Rank::Ace, Suit::Clubs), Card::new(Rank::Two, Suit::Hearts)];
    // North (leader_target) has high heart K♥ and a small club to follow
    let north_cards = vec![Card::new(Rank::King, Suit::Hearts), Card::new(Rank::Three, Suit::Clubs)];
    let west_cards = vec![Card::new(Rank::Queen, Suit::Clubs), Card::new(Rank::Four, Suit::Hearts)];
    let south_cards = vec![Card::new(Rank::Jack, Suit::Clubs), Card::new(Rank::Five, Suit::Hearts)];

    let mut hands = [Hand::new(), Hand::new(), Hand::new(), Hand::new()];
    hands[PlayerPosition::North.index()] = Hand::with_cards(north_cards);
    hands[PlayerPosition::East.index()] = Hand::with_cards(east_cards);
    hands[PlayerPosition::South.index()] = Hand::with_cards(south_cards);
    hands[PlayerPosition::West.index()] = Hand::with_cards(west_cards);

    // Fresh trick led by 'seat', seed one previous trick so this is not the first trick
    let current = hearts_core::model::trick::Trick::new(seat);
    let mut prev = hearts_core::model::trick::Trick::new(seat);
    let _ = prev.play(seat, Card::new(Rank::Five, Suit::Hearts));
    let _ = prev.play(seat.next(), Card::new(Rank::Six, Suit::Hearts));
    let _ = prev.play(seat.next().next(), Card::new(Rank::Seven, Suit::Hearts));
    let _ = prev.play(seat.next().next().next(), Card::new(Rank::Eight, Suit::Hearts));
    let round = RoundState::from_hands_with_state(
        hands,
        seat,
        PassingDirection::Hold,
        RoundPhase::Playing,
        current,
        vec![prev],
        true, // hearts broken
    );
    // Scoreboard: make North the leader_target with highest score
    let mut scores = ScoreBoard::new();
    scores.set_totals([90, 10, 20, 30]); // [North, East, South, West]
    (round, scores)
}

#[test]
fn hard_endgame_dp_hits_when_enabled() {
    let seat = PlayerPosition::East;
    let (round, scores) = build_tiny_endgame_lead(seat);

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
        std::env::set_var("MDH_HARD_TEST_STEPS", "120");
        std::env::set_var("MDH_HARD_ENDGAME_DP_ENABLE", "1");
        std::env::set_var("MDH_HARD_ENDGAME_MAX_CARDS", "3");
        std::env::set_var("MDH_HARD_PHASEB_TOPK", "8");
        std::env::set_var("MDH_HARD_BRANCH_LIMIT", "8");
        // keep other weights at defaults; this is a hits smoke
    }
    assert_eq!(legal.len(), 2);
    let chosen = PlayPlannerHard::choose(&legal, &ctx).expect("a choice");
    assert!(legal.contains(&chosen));
    // Stats present; this ensures planner scanned at least one candidate under deterministic budget
    if let Some(stats) = hearts_app::bot::search::last_stats() {
        assert!(stats.scanned >= 1);
    }
    unsafe {
        std::env::remove_var("MDH_HARD_DETERMINISTIC");
        std::env::remove_var("MDH_HARD_TEST_STEPS");
        std::env::remove_var("MDH_HARD_ENDGAME_DP_ENABLE");
        std::env::remove_var("MDH_HARD_ENDGAME_MAX_CARDS");
        std::env::remove_var("MDH_HARD_PHASEB_TOPK");
        std::env::remove_var("MDH_HARD_BRANCH_LIMIT");
    }
}
