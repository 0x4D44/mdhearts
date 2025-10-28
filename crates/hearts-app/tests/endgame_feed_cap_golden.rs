use hearts_app::bot::{BotContext, BotDifficulty, PlayPlanner, UnseenTracker};
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
    // Seed a prior completed trick to avoid first-trick restrictions
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
fn endgame_feed_cap_golden_qs_to_leader() {
    // Constructed golden: leader near 100; we cannot follow suit; hearts are broken; we hold QS and a small heart.
    // Expect: choose QS to feed the leader, even with a small MDH_W_ENDGAME_FEED_CAP set.
    let starting = PlayerPosition::East; // leader plays clubs Ace
    let our_seat = PlayerPosition::South;
    let hands = [
        vec![Card::new(Rank::Two, Suit::Hearts)], // North
        vec![Card::new(Rank::Ace, Suit::Clubs)],  // East (leader, near 100)
        vec![
            Card::new(Rank::Queen, Suit::Spades),
            Card::new(Rank::Five, Suit::Hearts),
        ], // South (our seat)
        vec![Card::new(Rank::King, Suit::Diamonds)], // West
    ];
    let round = build_round(
        starting,
        hands,
        &[(PlayerPosition::East, Card::new(Rank::Ace, Suit::Clubs))],
        true,
    );
    let scores = build_scores([40, 95, 45, 60]); // East is scoreboard leader near 100

    let mut tracker = UnseenTracker::new();
    tracker.reset_for_round(&round);
    let ctx = BotContext::new(
        our_seat,
        &round,
        &scores,
        PassingDirection::Hold,
        &tracker,
        BotDifficulty::NormalHeuristic,
    );

    let legal = {
        round
            .hand(our_seat)
            .iter()
            .copied()
            .filter(|card| {
                let mut probe = round.clone();
                probe.play_card(our_seat, *card).is_ok()
            })
            .collect::<Vec<_>>()
    };
    assert!(legal.contains(&Card::new(Rank::Queen, Suit::Spades)));
    assert!(legal.contains(&Card::new(Rank::Five, Suit::Hearts)));

    // Baseline: should select QS (feed to leader)
    let choice_no_cap = PlayPlanner::choose(&legal, &ctx).unwrap();
    assert_eq!(choice_no_cap, Card::new(Rank::Queen, Suit::Spades));

    // With a small cap enabled, the golden behavior remains QS
    unsafe { std::env::set_var("MDH_W_ENDGAME_FEED_CAP", "40") }
    let choice_capped = PlayPlanner::choose(&legal, &ctx).unwrap();
    assert_eq!(choice_capped, Card::new(Rank::Queen, Suit::Spades));
    unsafe { std::env::remove_var("MDH_W_ENDGAME_FEED_CAP") }
}
