use hearts_app::bot::{BotContext, BotDifficulty, PlayPlannerHard, UnseenTracker};
use hearts_core::model::card::Card;
use hearts_core::model::hand::Hand;
use hearts_core::model::passing::PassingDirection;
use hearts_core::model::player::PlayerPosition;
use hearts_core::model::rank::Rank;
use hearts_core::model::round::{RoundPhase, RoundState};
use hearts_core::model::score::ScoreBoard;
use hearts_core::model::suit::Suit;

fn build_endgame_flip(seat: PlayerPosition) -> (RoundState, ScoreBoard) {
    // ≤3 cards per hand; hearts broken. East leads. After A♣ wins, only hearts remain so we lead hearts next.
    // Configure other hands so North (leader) captures the heart trick with K♥.
    let east = vec![
        Card::new(Rank::Ace, Suit::Clubs),
        Card::new(Rank::Two, Suit::Hearts),
    ];
    let north = vec![
        Card::new(Rank::King, Suit::Hearts),
        Card::new(Rank::Three, Suit::Clubs),
    ];
    let south = vec![
        Card::new(Rank::Five, Suit::Hearts),
        Card::new(Rank::Jack, Suit::Clubs),
    ];
    let west = vec![
        Card::new(Rank::Four, Suit::Hearts),
        Card::new(Rank::Queen, Suit::Clubs),
    ];

    let mut hands = [Hand::new(), Hand::new(), Hand::new(), Hand::new()];
    hands[PlayerPosition::North.index()] = Hand::with_cards(north);
    hands[PlayerPosition::East.index()] = Hand::with_cards(east);
    hands[PlayerPosition::South.index()] = Hand::with_cards(south);
    hands[PlayerPosition::West.index()] = Hand::with_cards(west);

    let current = hearts_core::model::trick::Trick::new(seat);
    // Seed previous trick so we are not in the first-trick special case
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
    scores.set_totals([90, 10, 20, 30]); // make North the leader
    (round, scores)
}

#[test]
#[ignore]
fn hard_endgame_dp_strict_flip_golden() {
    // This golden encodes an intended flip under endgame DP. It is ignored by default until
    // endgame-only continuation/cap defaults are promoted sufficiently.
    let seat = PlayerPosition::East;
    let (round, scores) = build_endgame_flip(seat);
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
        // Deterministic + widen a bit
        std::env::set_var("MDH_HARD_DETERMINISTIC", "1");
        std::env::set_var("MDH_HARD_TEST_STEPS", "160");
        std::env::set_var("MDH_HARD_PHASEB_TOPK", "8");
        std::env::set_var("MDH_HARD_BRANCH_LIMIT", "8");
        // Endgame influence stronger in this test to force the flip
        std::env::set_var("MDH_HARD_ENDGAME_DP_ENABLE", "1");
        std::env::set_var("MDH_HARD_ENDGAME_MAX_CARDS", "3");
        // Suppress current-trick feed bonus; amplify next-trick feed so DP signal dominates
        std::env::set_var("MDH_HARD_CONT_FEED_PERPEN", "0");
        std::env::set_var("MDH_HARD_CONT_CAP", "1200");
        std::env::set_var("MDH_HARD_NEXT2_FEED_PERPEN", "400");
        std::env::set_var("MDH_HARD_NEXT2_SELF_CAPTURE_PERPEN", "60");
        // Neutralize planner base leader-feed nudge
        std::env::set_var("MDH_W_LEADER_FEED_BASE", "0");
        std::env::set_var("MDH_W_LEADER_FEED_GAP_PER10", "0");
        // Penalize losing control on current trick
        std::env::set_var("MDH_HARD_CTRL_HANDOFF_PEN", "150");
    }
    let with_dp = PlayPlannerHard::choose(&legal, &ctx).expect("choice with DP");
    unsafe {
        // DP off path
        std::env::remove_var("MDH_HARD_ENDGAME_DP_ENABLE");
    }
    let without_dp = PlayPlannerHard::choose(&legal, &ctx).expect("choice without DP");

    assert_ne!(
        with_dp, without_dp,
        "expected DP to flip under test weights"
    );

    unsafe {
        std::env::remove_var("MDH_HARD_DETERMINISTIC");
        std::env::remove_var("MDH_HARD_TEST_STEPS");
        std::env::remove_var("MDH_HARD_PHASEB_TOPK");
        std::env::remove_var("MDH_HARD_BRANCH_LIMIT");
        std::env::remove_var("MDH_HARD_CONT_FEED_PERPEN");
        std::env::remove_var("MDH_HARD_CONT_CAP");
        std::env::remove_var("MDH_HARD_NEXT2_FEED_PERPEN");
        std::env::remove_var("MDH_HARD_NEXT2_SELF_CAPTURE_PERPEN");
        std::env::remove_var("MDH_HARD_ENDGAME_MAX_CARDS");
        std::env::remove_var("MDH_W_LEADER_FEED_BASE");
        std::env::remove_var("MDH_W_LEADER_FEED_GAP_PER10");
        std::env::remove_var("MDH_HARD_CTRL_HANDOFF_PEN");
    }
}
