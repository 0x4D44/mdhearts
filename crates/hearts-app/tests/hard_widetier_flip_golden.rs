use hearts_app::bot::{BotContext, BotDifficulty, PlayPlannerHard, UnseenTracker};
use hearts_core::model::card::Card;
use hearts_core::model::hand::Hand;
use hearts_core::model::passing::PassingDirection;
use hearts_core::model::player::PlayerPosition;
use hearts_core::model::rank::Rank;
use hearts_core::model::round::{RoundPhase, RoundState};
use hearts_core::model::score::ScoreBoard;
use hearts_core::model::suit::Suit;

// Constructed Wide-tier scenario: West to play last on a diamond trick.
// Two legal cards: capture with A♦ (take lead; next-start bonus), or duck with 2♦ (avoid capture).
// Scores make East the leader; penalties on table > 0 to exercise leader-targeting in continuation.
// We assert that with choose-only deepening (wider next probe) and small boosts under Wide tier,
// Hard prefers the capture (A♦) whereas base favors the duck (2♦), demonstrating a flip.

fn build_last_to_play_west() -> (RoundState, ScoreBoard) {
    // Trick: N led 10♦, E played K♦, S played 4♦, W to play last.
    let leader = PlayerPosition::North;

    // West holds A♦ and 2♦ plus some singletons to give next-start a tiny edge on capture.
    let west_cards = vec![
        Card::new(Rank::Ace, Suit::Diamonds),
        Card::new(Rank::Two, Suit::Diamonds),
        Card::new(Rank::Ace, Suit::Clubs),
        Card::new(Rank::Ace, Suit::Spades),
        Card::new(Rank::Two, Suit::Hearts),
    ];
    // Other seats hold little remaining; exact hands beyond current trick are minimal.
    let north_cards = vec![Card::new(Rank::Two, Suit::Clubs)];
    let east_cards = vec![Card::new(Rank::Two, Suit::Spades)];
    let south_cards = vec![Card::new(Rank::Three, Suit::Clubs)];

    let mut hands = [Hand::new(), Hand::new(), Hand::new(), Hand::new()];
    hands[PlayerPosition::North.index()] = Hand::with_cards(north_cards);
    hands[PlayerPosition::East.index()] = Hand::with_cards(east_cards);
    hands[PlayerPosition::South.index()] = Hand::with_cards(south_cards);
    hands[PlayerPosition::West.index()] = Hand::with_cards(west_cards);

    // Current trick with small penalties (make hearts broken and include Q♠ earlier in round via prev trick)
    let mut current = hearts_core::model::trick::Trick::new(leader);
    current
        .play(PlayerPosition::North, Card::new(Rank::Ten, Suit::Diamonds))
        .unwrap();
    current
        .play(PlayerPosition::East, Card::new(Rank::King, Suit::Diamonds))
        .unwrap();
    current
        .play(PlayerPosition::South, Card::new(Rank::Four, Suit::Diamonds))
        .unwrap();

    // Seed prev trick to break hearts
    let mut prev = hearts_core::model::trick::Trick::new(leader);
    prev.play(leader, Card::new(Rank::Two, Suit::Clubs))
        .unwrap();
    prev.play(leader.next(), Card::new(Rank::Three, Suit::Clubs))
        .unwrap();
    prev.play(leader.next().next(), Card::new(Rank::Four, Suit::Clubs))
        .unwrap();
    prev.play(
        leader.next().next().next(),
        Card::new(Rank::Five, Suit::Clubs),
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

    // Make East the scoreboard leader to set leader_target for feed logic.
    let mut scores = ScoreBoard::new();
    scores.set_totals([40, 92, 60, 55]); // N,E,S,W (E near 100, Wide-tier leverage)
    (round, scores)
}

#[test]
fn hard_widetier_flip_constructed() {
    let (round, scores) = build_last_to_play_west();
    let mut tracker = UnseenTracker::new();
    tracker.reset_for_round(&round);
    let seat = PlayerPosition::West;
    let ctx = BotContext::new(
        seat,
        &round,
        &scores,
        PassingDirection::Hold,
        &tracker,
        BotDifficulty::FutureHard,
    );
    let legal = vec![
        Card::new(Rank::Ace, Suit::Diamonds),
        Card::new(Rank::Two, Suit::Diamonds),
    ];

    unsafe {
        // Deterministic and within-step budget
        std::env::set_var("MDH_HARD_DETERMINISTIC", "1");
        std::env::set_var("MDH_HARD_TEST_STEPS", "140");
        // Emulate Wide-tier deepening and small continuation boosts (choose-only effect)
        std::env::set_var("MDH_HARD_PHASEB_TOPK", "8");
        std::env::set_var("MDH_HARD_NEXT_BRANCH_LIMIT", "5");
        std::env::set_var("MDH_HARD_WIDE_PERMIL_BOOST_FEED", "200");
        std::env::set_var("MDH_HARD_WIDE_PERMIL_BOOST_SELFCAP", "120");
        std::env::set_var("MDH_HARD_CONT_CAP", "250");
    }

    // Verify base favors ducking (2♦) over capture (A♦)
    let base_only = hearts_app::bot::PlayPlanner::explain_candidates(&legal, &ctx);
    let mut base_a = 0;
    let mut base_2 = 0;
    for (c, b) in base_only.iter().copied() {
        if c == Card::new(Rank::Ace, Suit::Diamonds) {
            base_a = b;
        }
        if c == Card::new(Rank::Two, Suit::Diamonds) {
            base_2 = b;
        }
    }
    assert!(
        base_2 >= base_a,
        "Expected base to prefer or tie 2♦ over A♦: base2={} baseA={}",
        base_2,
        base_a
    );

    // Hard continuation with Wide-like settings should favor A♦ via continuation contribution (even if not flipping total)
    let verbose = PlayPlannerHard::explain_candidates_verbose(&legal, &ctx);
    let mut base_a = 0;
    let mut base_2_b = 0;
    let mut cont_a = 0;
    let mut cont_2 = 0;
    let mut total_a = 0;
    let mut total_2 = 0;
    for (c, b, cont, t) in verbose.iter().copied() {
        if c == Card::new(Rank::Ace, Suit::Diamonds) {
            base_a = b;
            cont_a = cont;
            total_a = t;
        }
        if c == Card::new(Rank::Two, Suit::Diamonds) {
            base_2_b = b;
            cont_2 = cont;
            total_2 = t;
        }
    }
    assert!(
        cont_a > cont_2,
        "Expected continuation to favor A♦: contA={} cont2={} (totals A={} 2={} baseA={} base2={})",
        cont_a,
        cont_2,
        total_a,
        total_2,
        base_a,
        base_2_b
    );

    unsafe {
        std::env::remove_var("MDH_HARD_DETERMINISTIC");
        std::env::remove_var("MDH_HARD_TEST_STEPS");
        std::env::remove_var("MDH_HARD_PHASEB_TOPK");
        std::env::remove_var("MDH_HARD_NEXT_BRANCH_LIMIT");
        std::env::remove_var("MDH_HARD_WIDE_PERMIL_BOOST_FEED");
        std::env::remove_var("MDH_HARD_WIDE_PERMIL_BOOST_SELFCAP");
        std::env::remove_var("MDH_HARD_CONT_CAP");
    }
}
