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
    // seed with a complete trick to avoid first-trick constraints
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
fn hard_verbose_contains_continuation_terms() {
    // East is leaderboard leader and leads AC; South (our seat) is void and can dump QS or a safe diamond.
    // In Hard, QS should show positive continuation (feed leader) relative to a safe diamond.
    let starting = PlayerPosition::East;
    let our_seat = PlayerPosition::South;
    let hands = [
        vec![Card::new(Rank::Two, Suit::Hearts)], // North
        vec![Card::new(Rank::Ace, Suit::Clubs)],  // East (leader, winning)
        vec![
            Card::new(Rank::Queen, Suit::Spades), // South (our seat)
            Card::new(Rank::Five, Suit::Diamonds),
        ],
        vec![Card::new(Rank::King, Suit::Diamonds)], // West
    ];
    let round = build_round(
        starting,
        hands,
        &[(PlayerPosition::East, Card::new(Rank::Ace, Suit::Clubs))],
        true,
    );
    let scores = build_scores([40, 95, 45, 50]); // East as clear leader
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
    assert!(legal.contains(&Card::new(Rank::Five, Suit::Diamonds)));

    let verbose = PlayPlannerHard::explain_candidates_verbose(&legal, &ctx);
    // Find entries for QS and 5D
    let mut qs = None;
    let mut d5 = None;
    for (c, base, cont, total) in verbose.iter().copied() {
        if c == Card::new(Rank::Queen, Suit::Spades) {
            qs = Some((base, cont, total));
        }
        if c == Card::new(Rank::Five, Suit::Diamonds) {
            d5 = Some((base, cont, total));
        }
    }
    let (qs_base, qs_cont, qs_total) = qs.expect("QS present");
    let (_d_base, d_cont, _d_total) = d5.expect("5D present");
    // Continuation should be non-zero and favor feeding QS to leader
    assert!(
        qs_cont > 0,
        "QS continuation should be positive, got {}",
        qs_cont
    );
    assert!(
        qs_total >= qs_base,
        "QS total should reflect non-negative continuation"
    );
    // Safe diamond should have no or lower continuation than QS in this setup
    assert!(
        qs_cont >= d_cont,
        "QS cont {} should be >= D5 cont {}",
        qs_cont,
        d_cont
    );
    // Sanity: totals are returned in explain order; choice uses totals in Hard mode
    let top = verbose
        .iter()
        .max_by_key(|(_, _, _, t)| *t)
        .map(|(c, _, _, _)| *c)
        .unwrap();
    assert!(legal.contains(&top));
}
