use hearts_app::bot::{BotContext, BotDifficulty, PlayPlannerHard, UnseenTracker};
use hearts_core::model::card::Card;
use hearts_core::model::hand::Hand;
use hearts_core::model::passing::PassingDirection;
use hearts_core::model::player::PlayerPosition;
use hearts_core::model::rank::Rank;
use hearts_core::model::round::{RoundPhase, RoundState};
use hearts_core::model::score::ScoreBoard;
use hearts_core::model::suit::Suit;

fn build_midtrick_round_west_to_play() -> (RoundState, ScoreBoard) {
    // Current trick (leader North): N=10♦, E=K♦, S=4♦, W=? (to play)
    let leader = PlayerPosition::North;

    // West hand with capture (A♦) and lose (2♦), plus singletons for start bonus on capture
    let west_cards = vec![
        Card::new(Rank::Ace, Suit::Diamonds),
        Card::new(Rank::Two, Suit::Diamonds),
        Card::new(Rank::Ace, Suit::Clubs),
        Card::new(Rank::Ace, Suit::Spades),
        Card::new(Rank::Two, Suit::Hearts),
        Card::new(Rank::Three, Suit::Hearts),
        Card::new(Rank::Four, Suit::Hearts),
        Card::new(Rank::Five, Suit::Hearts),
        Card::new(Rank::Six, Suit::Hearts),
    ];

    let north_cards = vec![
        Card::new(Rank::Two, Suit::Clubs),
        Card::new(Rank::Three, Suit::Clubs),
    ];
    let east_cards = vec![
        Card::new(Rank::Two, Suit::Spades),
        Card::new(Rank::Three, Suit::Spades),
    ];
    let south_cards = vec![
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
        .play(PlayerPosition::North, Card::new(Rank::Ten, Suit::Diamonds))
        .unwrap();
    current
        .play(PlayerPosition::East, Card::new(Rank::King, Suit::Diamonds))
        .unwrap();
    current
        .play(PlayerPosition::South, Card::new(Rank::Four, Suit::Diamonds))
        .unwrap();

    // Seed a previous trick to avoid first-trick constraints; hearts broken
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

    // Make East the scoreboard leader to define leader_target
    let mut scores = ScoreBoard::new();
    scores.set_totals([40, 90, 60, 55]); // N,E,S,W

    (round, scores)
}

fn legal_moves_for(round: &RoundState, seat: PlayerPosition) -> Vec<Card> {
    round
        .hand(seat)
        .iter()
        .copied()
        .filter(|card| {
            let mut probe = round.clone();
            probe.play_card(seat, *card).is_ok()
        })
        .collect::<Vec<_>>()
}

#[test]
fn hard_constructed_midtrick_neartie_continuation_decides() {
    let (round, scores) = build_midtrick_round_west_to_play();
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
    let legal = legal_moves_for(&round, seat);
    assert!(legal.contains(&Card::new(Rank::Ace, Suit::Diamonds)));
    assert!(legal.contains(&Card::new(Rank::Two, Suit::Diamonds)));

    // Deterministic budget and tie-break boost to let continuation dominate near-tie cases
    unsafe {
        std::env::set_var("MDH_HARD_DETERMINISTIC", "1");
        std::env::set_var("MDH_HARD_TEST_STEPS", "100");
        std::env::set_var("MDH_HARD_PHASEB_TOPK", "6");
        // Large boost so that positive continuation for capture (start bonus) outweighs base's avoid-capture bias
        std::env::set_var("MDH_HARD_CONT_BOOST_GAP", "10000");
        std::env::set_var("MDH_HARD_CONT_BOOST_FACTOR", "200");
        // Strengthen start bonus and add a small handoff penalty when we don't win
        std::env::set_var("MDH_HARD_NEXTTRICK_SINGLETON", "60");
        std::env::set_var("MDH_HARD_CTRL_HANDOFF_PEN", "40");
    }

    let verbose = PlayPlannerHard::explain_candidates_verbose(&legal, &ctx);
    let mut base_a = 0;
    let mut base_2 = 0;
    let mut total_a = 0;
    let mut total_2 = 0;
    for (c, base, _cont, total) in verbose.iter().copied() {
        if c == Card::new(Rank::Ace, Suit::Diamonds) {
            base_a = base;
            total_a = total;
        }
        if c == Card::new(Rank::Two, Suit::Diamonds) {
            base_2 = base;
            total_2 = total;
        }
    }
    // Base should favor not capturing (2♦) over capturing (A♦)
    assert!(
        base_2 > base_a,
        "Expected base to prefer 2♦ over A♦ (avoid capture): base_2={} base_a={}",
        base_2,
        base_a
    );
    // With continuation boost, total should flip in favor of A♦ (capture for lead/start bonus)
    assert!(
        total_a > total_2,
        "Expected boosted total to prefer A♦ over 2♦: total_a={} total_2={}",
        total_a,
        total_2
    );

    // Cleanup env
    unsafe {
        std::env::remove_var("MDH_HARD_DETERMINISTIC");
        std::env::remove_var("MDH_HARD_TEST_STEPS");
        std::env::remove_var("MDH_HARD_PHASEB_TOPK");
        std::env::remove_var("MDH_HARD_CONT_BOOST_GAP");
        std::env::remove_var("MDH_HARD_CONT_BOOST_FACTOR");
        std::env::remove_var("MDH_HARD_NEXTTRICK_SINGLETON");
        std::env::remove_var("MDH_HARD_CTRL_HANDOFF_PEN");
    }
}
