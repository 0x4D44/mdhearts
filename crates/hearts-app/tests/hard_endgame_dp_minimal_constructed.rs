use hearts_app::bot::{BotContext, BotDifficulty, PlayPlannerHard, UnseenTracker};
use hearts_core::model::card::Card;
use hearts_core::model::hand::Hand;
use hearts_core::model::passing::PassingDirection;
use hearts_core::model::player::PlayerPosition;
use hearts_core::model::rank::Rank;
use hearts_core::model::round::{RoundPhase, RoundState};
use hearts_core::model::score::ScoreBoard;
use hearts_core::model::suit::Suit;

fn build_constructed_endgame(seat: PlayerPosition) -> (RoundState, ScoreBoard) {
    // Constructed minimal endgame (2 cards each), hearts broken.
    // Goal: Without DP, base prefers to avoid capturing (play hearts 2H);
    // With DP, winning with AC now sets up a next-trick heart feed to the leader (North).
    let east = vec![
        Card::new(Rank::Ace, Suit::Clubs),
        Card::new(Rank::Two, Suit::Hearts),
    ];
    let north = vec![
        Card::new(Rank::Three, Suit::Clubs),
        Card::new(Rank::King, Suit::Hearts),
    ];
    let south = vec![
        Card::new(Rank::Jack, Suit::Clubs),
        Card::new(Rank::Five, Suit::Hearts),
    ];
    let west = vec![
        Card::new(Rank::Queen, Suit::Clubs),
        Card::new(Rank::Four, Suit::Hearts),
    ];

    let mut hands = [Hand::new(), Hand::new(), Hand::new(), Hand::new()];
    hands[PlayerPosition::North.index()] = Hand::with_cards(north);
    hands[PlayerPosition::East.index()] = Hand::with_cards(east);
    hands[PlayerPosition::South.index()] = Hand::with_cards(south);
    hands[PlayerPosition::West.index()] = Hand::with_cards(west);

    let current = hearts_core::model::trick::Trick::new(seat);
    // Seed a previous trick (no penalties) to avoid first-trick rules
    let mut prev = hearts_core::model::trick::Trick::new(seat);
    let _ = prev.play(seat, Card::new(Rank::Nine, Suit::Diamonds));
    let _ = prev.play(seat.next(), Card::new(Rank::Ten, Suit::Diamonds));
    let _ = prev.play(seat.next().next(), Card::new(Rank::Jack, Suit::Diamonds));
    let _ = prev.play(
        seat.next().next().next(),
        Card::new(Rank::Queen, Suit::Diamonds),
    );

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
    scores.set_totals([90, 10, 20, 30]); // North is score leader (feed target)
    (round, scores)
}

#[test]
#[ignore]
fn hard_endgame_dp_minimal_constructed_flip() {
    let seat = PlayerPosition::East;
    let (round, scores) = build_constructed_endgame(seat);
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

    unsafe {
        // Keep deterministic with a modest step cap
        std::env::set_var("MDH_HARD_DETERMINISTIC", "1");
        std::env::set_var("MDH_HARD_TEST_STEPS", "160");
        // Neutralize base leader-feed nudges so DP can dominate
        std::env::set_var("MDH_W_LEADER_FEED_BASE", "0");
        std::env::set_var("MDH_W_LEADER_FEED_GAP_PER10", "0");
        // Suppress current-trick continuation influence; focus on next-trick DP
        std::env::set_var("MDH_HARD_CONT_FEED_PERPEN", "0");
        std::env::set_var("MDH_HARD_CONT_SELF_CAPTURE_PERPEN", "0");
        // Strengthen DP next-trick continuation signals within a cap
        std::env::set_var("MDH_HARD_CONT_CAP", "800");
        std::env::set_var("MDH_HARD_NEXT2_FEED_PERPEN", "250");
        std::env::set_var("MDH_HARD_NEXT2_SELF_CAPTURE_PERPEN", "0");
        // Enable endgame DP over ≤3 cards
        std::env::set_var("MDH_HARD_ENDGAME_DP_ENABLE", "1");
        std::env::set_var("MDH_HARD_ENDGAME_MAX_CARDS", "3");
        // Ensure continuation path runs for our winning-now line
        std::env::set_var("MDH_HARD_PHASEB_TOPK", "6");
        // Disable early cutoff so lower-base candidate still gets continuation scored
        std::env::set_var("MDH_HARD_AB_MARGIN", "0");
    }

    // DP OFF first
    unsafe {
        std::env::remove_var("MDH_HARD_ENDGAME_DP_ENABLE");
    }
    let off = PlayPlannerHard::choose(&legal, &ctx);
    // DP ON
    unsafe {
        std::env::set_var("MDH_HARD_ENDGAME_DP_ENABLE", "1");
    }
    let on = PlayPlannerHard::choose(&legal, &ctx);

    assert_ne!(off, on, "expected DP to flip choice in constructed endgame");

    unsafe {
        // Cleanup envs
        std::env::remove_var("MDH_HARD_DETERMINISTIC");
        std::env::remove_var("MDH_HARD_TEST_STEPS");
        std::env::remove_var("MDH_W_LEADER_FEED_BASE");
        std::env::remove_var("MDH_W_LEADER_FEED_GAP_PER10");
        std::env::remove_var("MDH_HARD_CONT_FEED_PERPEN");
        std::env::remove_var("MDH_HARD_CONT_SELF_CAPTURE_PERPEN");
        std::env::remove_var("MDH_HARD_CONT_CAP");
        std::env::remove_var("MDH_HARD_NEXT2_FEED_PERPEN");
        std::env::remove_var("MDH_HARD_NEXT2_SELF_CAPTURE_PERPEN");
        std::env::remove_var("MDH_HARD_ENDGAME_DP_ENABLE");
        std::env::remove_var("MDH_HARD_ENDGAME_MAX_CARDS");
        std::env::remove_var("MDH_HARD_PHASEB_TOPK");
        std::env::remove_var("MDH_HARD_AB_MARGIN");
    }
}
