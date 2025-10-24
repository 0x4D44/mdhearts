use hearts_app::bot::{BotContext, BotDifficulty, PlayPlannerHard, UnseenTracker};
use hearts_core::model::card::Card;
use hearts_core::model::hand::Hand;
use hearts_core::model::passing::PassingDirection;
use hearts_core::model::player::PlayerPosition;
use hearts_core::model::rank::Rank;
use hearts_core::model::round::{RoundPhase, RoundState};
use hearts_core::model::score::ScoreBoard;
use hearts_core::model::suit::Suit;

fn build_midtrick_round_east_to_play() -> (RoundState, ScoreBoard) {
    // Current trick (leader South): S=9♣, W=2♣, N=3♣, E=? (to play)
    // East holds options to capture (K♣) vs lose (4♣). Capturing yields lead next with some singletons.
    let leader = PlayerPosition::South;

    let north = vec![Card::new(Rank::Three, Suit::Clubs), Card::new(Rank::Two, Suit::Spades)];
    let west = vec![Card::new(Rank::Two, Suit::Clubs), Card::new(Rank::Two, Suit::Diamonds)];
    let south = vec![Card::new(Rank::Nine, Suit::Clubs), Card::new(Rank::Two, Suit::Hearts)];
    let east = vec![
        Card::new(Rank::King, Suit::Clubs), // capture option
        Card::new(Rank::Four, Suit::Clubs), // lose option
        Card::new(Rank::Ace, Suit::Spades), // singleton for start bonus if we capture
        Card::new(Rank::Ace, Suit::Diamonds),
    ];

    let mut hands = [Hand::new(), Hand::new(), Hand::new(), Hand::new()];
    hands[PlayerPosition::North.index()] = Hand::with_cards(north);
    hands[PlayerPosition::West.index()] = Hand::with_cards(west);
    hands[PlayerPosition::South.index()] = Hand::with_cards(south);
    hands[PlayerPosition::East.index()] = Hand::with_cards(east);

    let mut current = hearts_core::model::trick::Trick::new(leader);
    current
        .play(PlayerPosition::South, Card::new(Rank::Nine, Suit::Clubs))
        .unwrap();
    current
        .play(PlayerPosition::West, Card::new(Rank::Two, Suit::Clubs))
        .unwrap();
    current
        .play(PlayerPosition::North, Card::new(Rank::Three, Suit::Clubs))
        .unwrap();

    // Seed a previous trick to avoid first-trick constraints; hearts broken
    let mut prev = hearts_core::model::trick::Trick::new(leader);
    prev.play(leader, Card::new(Rank::Two, Suit::Clubs)).unwrap();
    prev.play(leader.next(), Card::new(Rank::Three, Suit::Clubs)).unwrap();
    prev.play(leader.next().next(), Card::new(Rank::Four, Suit::Clubs)).unwrap();
    prev.play(leader.next().next().next(), Card::new(Rank::Five, Suit::Clubs)).unwrap();

    let round = RoundState::from_hands_with_state(
        hands,
        leader,
        PassingDirection::Hold,
        RoundPhase::Playing,
        current,
        vec![prev],
        true,
    );

    // Make North the scoreboard leader to influence leader targeting; East is us
    let mut scores = ScoreBoard::new();
    scores.set_totals([92, 60, 55, 50]); // N,E,S,W

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
fn hard_constructed_midtrick_neartie_east_continuation_decides() {
    let (round, scores) = build_midtrick_round_east_to_play();
    let mut tracker = UnseenTracker::new();
    tracker.reset_for_round(&round);
    let seat = PlayerPosition::East;
    let ctx = BotContext::new(
        seat,
        &round,
        &scores,
        PassingDirection::Hold,
        &tracker,
        BotDifficulty::FutureHard,
    );
    let legal = legal_moves_for(&round, seat);
    assert!(legal.contains(&Card::new(Rank::King, Suit::Clubs)));
    assert!(legal.contains(&Card::new(Rank::Four, Suit::Clubs)));

    // Deterministic budget and tie-break boost to let continuation decide a near-tie
    unsafe {
        std::env::set_var("MDH_HARD_DETERMINISTIC", "1");
        std::env::set_var("MDH_HARD_TEST_STEPS", "100");
        std::env::set_var("MDH_HARD_PHASEB_TOPK", "6");
        std::env::set_var("MDH_HARD_CONT_BOOST_GAP", "10000");
        std::env::set_var("MDH_HARD_CONT_BOOST_FACTOR", "200");
        std::env::set_var("MDH_HARD_NEXTTRICK_SINGLETON", "60");
        std::env::set_var("MDH_HARD_CTRL_HANDOFF_PEN", "40");
    }

    let verbose = PlayPlannerHard::explain_candidates_verbose(&legal, &ctx);
    let mut base_cap = 0;
    let mut base_lose = 0;
    let mut total_cap = 0;
    let mut total_lose = 0;
    for (c, base, _cont, total) in verbose.iter().copied() {
        if c == Card::new(Rank::King, Suit::Clubs) { base_cap = base; total_cap = total; }
        if c == Card::new(Rank::Four, Suit::Clubs) { base_lose = base; total_lose = total; }
    }
    // Base should prefer losing (4♣) over capturing (K♣)
    assert!(
        base_lose > base_cap,
        "Expected base to prefer 4♣ over K♣ (avoid capture): base_lose={} base_cap={}",
        base_lose,
        base_cap
    );
    // With continuation boost, total should flip to prefer capturing (K♣)
    assert!(
        total_cap > total_lose,
        "Expected boosted total to prefer K♣ over 4♣: total_cap={} total_lose={}",
        total_cap,
        total_lose
    );

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

