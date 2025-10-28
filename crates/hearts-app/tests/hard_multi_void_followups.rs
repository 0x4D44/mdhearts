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
    for (idx, cards) in hands_vec.into_iter().enumerate() {
        hands[idx] = Hand::with_cards(cards);
    }
    // Seed with a completed trick to avoid first-trick constraints
    let mut seed_trick = hearts_core::model::trick::Trick::new(starting);
    let mut seat_iter = starting;
    for card in [
        Card::new(Rank::Two, Suit::Clubs),
        Card::new(Rank::Three, Suit::Clubs),
        Card::new(Rank::Four, Suit::Clubs),
        Card::new(Rank::Five, Suit::Clubs),
    ] {
        seed_trick.play(seat_iter, card).unwrap();
        seat_iter = seat_iter.next();
    }
    let mut current_trick = hearts_core::model::trick::Trick::new(starting);
    for &(seat, card) in plays {
        current_trick.play(seat, card).unwrap();
    }
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

fn build_scores(values: [u32; 4]) -> ScoreBoard {
    let mut scores = ScoreBoard::new();
    for (idx, value) in values.iter().enumerate() {
        if let Some(pos) = PlayerPosition::from_index(idx) {
            scores.set_score(pos, *value);
        }
    }
    scores
}

#[test]
fn hard_multi_void_followups_self_capture_vs_leader_feed() {
    // Scenario: West leads clubs. North and East are void in clubs and dump penalties (QS and a heart).
    // South (our seat) plays last and can choose AC (capture 15) or 2C (lose; feed penalties to West).
    // With West as scoreboard leader, continuation should penalize self-capture (AC) and prefer feeding leader (2C).
    let starting = PlayerPosition::West;
    let our_seat = PlayerPosition::South; // plays last on this trick
    let hands = [
        // North: no clubs (void), can dump QS
        vec![
            Card::new(Rank::Queen, Suit::Spades),
            Card::new(Rank::Seven, Suit::Spades),
        ],
        // East: no clubs (void), can dump a heart
        vec![
            Card::new(Rank::Five, Suit::Hearts),
            Card::new(Rank::King, Suit::Spades),
        ],
        // South (our seat): AC and 2C to decide capture vs avoid; add a small spade
        vec![
            Card::new(Rank::Ace, Suit::Clubs),
            Card::new(Rank::Two, Suit::Clubs),
            Card::new(Rank::Two, Suit::Spades),
        ],
        // West (leader): leads low club
        vec![
            Card::new(Rank::Seven, Suit::Clubs),
            Card::new(Rank::Four, Suit::Diamonds),
        ],
    ];
    let plays = vec![
        (PlayerPosition::West, Card::new(Rank::Seven, Suit::Clubs)),
        (PlayerPosition::North, Card::new(Rank::Queen, Suit::Spades)), // dump QS (void clubs)
        (PlayerPosition::East, Card::new(Rank::Five, Suit::Hearts)),   // dump heart (void clubs)
    ];
    let round = build_round(
        starting, hands, &plays,
        false, /* hearts not necessarily broken; off-suit dump allowed when void */
    );
    let scores = build_scores([40, 50, 55, 80]); // West is scoreboard leader

    let mut tracker = UnseenTracker::new();
    tracker.reset_for_round(&round);
    let ctx = BotContext::new(
        our_seat,
        &round,
        &scores,
        PassingDirection::Hold,
        &tracker,
        BotDifficulty::FutureHard,
    );

    let legal = round
        .hand(our_seat)
        .iter()
        .copied()
        .filter(|card| {
            let mut probe = round.clone();
            probe.play_card(our_seat, *card).is_ok()
        })
        .collect::<Vec<_>>();
    assert!(legal.contains(&Card::new(Rank::Ace, Suit::Clubs)));
    assert!(legal.contains(&Card::new(Rank::Two, Suit::Clubs)));

    let verbose = PlayPlannerHard::explain_candidates_verbose(&legal, &ctx);
    let mut ac = None;
    let mut c2 = None;
    for (c, base, cont, total) in verbose.iter().copied() {
        if c == Card::new(Rank::Ace, Suit::Clubs) {
            ac = Some((base, cont, total));
        }
        if c == Card::new(Rank::Two, Suit::Clubs) {
            c2 = Some((base, cont, total));
        }
    }
    let (_ac_base, ac_cont, ac_total) = ac.expect("AC present");
    let (_c2_base, c2_cont, c2_total) = c2.expect("2C present");

    // Continuation should penalize AC (self-capture of QS+heart) and prefer 2C (feed to leader West)
    assert!(
        ac_cont < 0,
        "AC continuation should be negative (self-capture), got {}",
        ac_cont
    );
    assert!(
        c2_cont >= 0,
        "2C continuation should be non-negative (feed to leader), got {}",
        c2_cont
    );
    assert!(
        c2_total >= ac_total,
        "2C total {} should be >= AC total {}",
        c2_total,
        ac_total
    );

    // And Hard should choose 2C here
    let choice = PlayPlannerHard::choose(&legal, &ctx).unwrap();
    assert_eq!(choice, Card::new(Rank::Two, Suit::Clubs));
}
