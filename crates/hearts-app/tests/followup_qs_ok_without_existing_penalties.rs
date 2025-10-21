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
) -> RoundState {
    let mut hands = [Hand::new(), Hand::new(), Hand::new(), Hand::new()];
    for (idx, cards) in hands_vec.into_iter().enumerate() {
        hands[idx] = Hand::with_cards(cards);
    }
    // Seed with a previous complete trick to avoid first-trick constraints
    let mut prev = hearts_core::model::trick::Trick::new(starting);
    let mut seat = starting;
    for card in [
        Card::new(Rank::Two, Suit::Clubs),
        Card::new(Rank::Three, Suit::Clubs),
        Card::new(Rank::Four, Suit::Clubs),
        Card::new(Rank::Five, Suit::Clubs),
    ] {
        prev.play(seat, card).unwrap();
        seat = seat.next();
    }
    let mut current = hearts_core::model::trick::Trick::new(starting);
    for &(s, c) in plays {
        current.play(s, c).unwrap();
    }
    RoundState::from_hands_with_state(
        hands,
        starting,
        PassingDirection::Hold,
        RoundPhase::Playing,
        current,
        vec![prev],
        true, // hearts broken already, but no hearts/QS on table yet
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
fn qs_is_ok_to_dump_when_no_existing_penalties() {
    // East leads Ace of Clubs (no penalties yet). South (our seat) can't follow and holds QS and a safe diamond.
    // Scoreboard leader is West, but provisional winner is East. With no penalties on table yet, QS dump is acceptable.
    let starting = PlayerPosition::East;
    let our_seat = PlayerPosition::South;
    let hands = [
        vec![Card::new(Rank::Two, Suit::Spades)], // North
        vec![Card::new(Rank::Ace, Suit::Clubs)],  // East leads clubs
        vec![
            Card::new(Rank::Queen, Suit::Spades),
            Card::new(Rank::Five, Suit::Diamonds),
        ], // South cannot follow clubs
        vec![Card::new(Rank::King, Suit::Clubs)], // West (scoreboard leader)
    ];
    let round = build_round(
        starting,
        hands,
        &[(PlayerPosition::East, Card::new(Rank::Ace, Suit::Clubs))],
    );
    let scores = build_scores([40, 60, 45, 80]); // West leader; provisional winner is East
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
    assert!(legal.contains(&Card::new(Rank::Five, Suit::Diamonds)));

    let explained = hearts_app::controller::GameController::new_from_match_state({
        use hearts_core::game::match_state::MatchState;
        let mut ms = MatchState::new(starting);
        *ms.round_mut() = round.clone();
        *ms.scores_mut() = scores.clone();
        ms
    })
    .explain_candidates_for(our_seat);
    // QS should be at least competitive here since we are allowed to dump without existing penalties on table
    let mut qs_score = None;
    let mut safe_score = None;
    for (c, s) in explained.iter() {
        if *c == Card::new(Rank::Queen, Suit::Spades) {
            qs_score = Some(*s);
        }
        if *c == Card::new(Rank::Five, Suit::Diamonds) {
            safe_score = Some(*s);
        }
    }
    assert!(qs_score.is_some() && safe_score.is_some());
    assert!(qs_score.unwrap() >= safe_score.unwrap());

    let choice = PlayPlanner::choose(&legal, &ctx).unwrap();
    assert_eq!(choice, Card::new(Rank::Queen, Suit::Spades));
}
