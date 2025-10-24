use hearts_app::bot::{BotContext, BotDifficulty, PlayPlannerHard, UnseenTracker};
use hearts_core::model::card::Card;
use hearts_core::model::hand::Hand;
use hearts_core::model::passing::PassingDirection;
use hearts_core::model::player::PlayerPosition;
use hearts_core::model::rank::Rank;
use hearts_core::model::round::{RoundPhase, RoundState};
use hearts_core::model::score::ScoreBoard;
use hearts_core::model::suit::Suit;

fn build_round(
    starting: PlayerPosition,
    hands_vec: [Vec<Card>; 4],
    plays: &[(PlayerPosition, Card)],
    hearts_broken: bool,
) -> RoundState {
    let mut hands = [Hand::new(), Hand::new(), Hand::new(), Hand::new()];
    for (idx, cards) in hands_vec.into_iter().enumerate() { hands[idx] = Hand::with_cards(cards); }
    let mut seed_trick = hearts_core::model::trick::Trick::new(starting);
    let mut seat_iter = starting;
    for card in [
        Card::new(Rank::Two, Suit::Clubs),
        Card::new(Rank::Three, Suit::Clubs),
        Card::new(Rank::Four, Suit::Clubs),
        Card::new(Rank::Five, Suit::Clubs),
    ] { seed_trick.play(seat_iter, card).unwrap(); seat_iter = seat_iter.next(); }
    let mut current_trick = hearts_core::model::trick::Trick::new(starting);
    for &(seat, card) in plays { current_trick.play(seat, card).unwrap(); }
    RoundState::from_hands_with_state(
        hands,
        starting,
        PassingDirection::Hold,
        RoundPhase::Playing,
        current_trick,
        vec![seed_trick],
        hearts_broken,
    )
}

fn scores(values: [u32; 4]) -> ScoreBoard {
    let mut sc = ScoreBoard::new();
    for (idx, value) in values.iter().enumerate() {
        if let Some(pos) = PlayerPosition::from_index(idx) { sc.set_score(pos, *value); }
    }
    sc
}

#[test]
fn hard_third_branch_increases_or_equals_continuation() {
    // Construct a case where after we capture now and lead next, multiple opponents have useful off-suit dumps.
    // Enabling third-opponent branching should not reduce continuation (local_best picks max).
    unsafe {
        std::env::set_var("MDH_HARD_DETERMINISTIC", "1");
        std::env::set_var("MDH_HARD_TEST_STEPS", "200");
        std::env::set_var("MDH_HARD_PHASEB_TOPK", "8");
        std::env::remove_var("MDH_HARD_THIRD_BRANCH");
    }
    let starting = PlayerPosition::South; // our seat plays first on current trick
    let our_seat = PlayerPosition::South;
    let hands = [
        // North: void in diamonds, holds QS to dump on diamond lead
        vec![Card::new(Rank::Queen, Suit::Spades), Card::new(Rank::Seven, Suit::Clubs)],
        // East: void in diamonds, holds a heart to dump
        vec![Card::new(Rank::Five, Suit::Hearts), Card::new(Rank::Six, Suit::Clubs)],
        // South (our seat): AC (to win now) and 3D (to lead next), plus a safe loser
        vec![
            Card::new(Rank::Ace, Suit::Clubs),
            Card::new(Rank::Three, Suit::Diamonds),
            Card::new(Rank::Two, Suit::Spades),
        ],
        // West (leader): likely to win diamond follow if high, but here mainly for scoring target
        vec![Card::new(Rank::King, Suit::Diamonds), Card::new(Rank::Four, Suit::Clubs)],
    ];
    // No plays yet on current trick; South can play AC (capture) or 2S (duck).
    let round = build_round(starting, hands, &[], true);
    let sc = scores([40, 60, 45, 80]); // West leader

    // Build context
    let mut tracker = UnseenTracker::new();
    tracker.reset_for_round(&round);
    let ctx = BotContext::new(
        our_seat,
        &round,
        &sc,
        PassingDirection::Hold,
        &tracker,
        BotDifficulty::FutureHard,
    );
    let legal = round
        .hand(our_seat)
        .iter()
        .copied()
        .filter(|c| { let mut p = round.clone(); p.play_card(our_seat, *c).is_ok() })
        .collect::<Vec<_>>();
    assert!(legal.contains(&Card::new(Rank::Ace, Suit::Clubs)));

    // Continuation without third-opponent branching
    let verbose1 = PlayPlannerHard::explain_candidates_verbose(&legal, &ctx);
    let mut ac_cont_no3: Option<i32> = None;
    for (c, _b, cont, _t) in verbose1.iter().copied() {
        if c == Card::new(Rank::Ace, Suit::Clubs) { ac_cont_no3 = Some(cont); }
    }
    let ac_no3 = ac_cont_no3.expect("AC present");

    // Enable third-opponent branching
    unsafe { std::env::set_var("MDH_HARD_THIRD_BRANCH", "1"); }
    let verbose2 = PlayPlannerHard::explain_candidates_verbose(&legal, &ctx);
    let mut ac_cont_3: Option<i32> = None;
    for (c, _b, cont, _t) in verbose2.iter().copied() {
        if c == Card::new(Rank::Ace, Suit::Clubs) { ac_cont_3 = Some(cont); }
    }
    let ac_3 = ac_cont_3.expect("AC present under third-branch");

    assert!(ac_3 >= ac_no3, "third-branch continuation {} should be >= baseline {}", ac_3, ac_no3);
}

