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
    // seed with a complete trick to avoid first-trick constraints
    let mut seed_trick = hearts_core::model::trick::Trick::new(starting);
    let mut seat_iter = starting;
    let seed_cards = [
        Card::new(Rank::Two, Suit::Clubs),
        Card::new(Rank::Three, Suit::Clubs),
        Card::new(Rank::Four, Suit::Clubs),
        Card::new(Rank::Five, Suit::Clubs),
    ];
    for card in seed_cards {
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
fn leader_target_dump_under_90_prefers_qs() {
    // East is scoreboard leader (70) and leads clubs; South (our seat) is void and can dump QS or a safe diamond.
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
    let scores = build_scores([40, 70, 45, 50]);
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
    // Ensure QS scores higher than the safe diamond due to leader-target bias below near-100
    let mut qs_score = None;
    let mut d_score = None;
    for (c, s) in explained.iter() {
        if *c == Card::new(Rank::Queen, Suit::Spades) {
            qs_score = Some(*s);
        }
        if *c == Card::new(Rank::Five, Suit::Diamonds) {
            d_score = Some(*s);
        }
    }
    assert!(qs_score.is_some() && d_score.is_some());
    assert!(qs_score.unwrap() > d_score.unwrap());

    let choice = PlayPlanner::choose(&legal, &ctx).unwrap();
    assert_eq!(choice, Card::new(Rank::Queen, Suit::Spades));
}
