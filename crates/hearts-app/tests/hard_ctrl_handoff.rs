use hearts_app::bot::{BotContext, BotDifficulty, PlayPlannerHard, UnseenTracker};
use hearts_core::model::card::Card;
use hearts_core::model::hand::Hand;
use hearts_core::model::passing::PassingDirection;
use hearts_core::model::player::PlayerPosition;
use hearts_core::model::rank::Rank;
use hearts_core::model::round::{RoundPhase, RoundState};
use hearts_core::model::score::ScoreBoard;
use hearts_core::model::suit::Suit;

fn build_round_not_first_trick(starting: PlayerPosition, hands_vec: [Vec<Card>; 4]) -> RoundState {
    let mut hands = [Hand::new(), Hand::new(), Hand::new(), Hand::new()];
    for (idx, cards) in hands_vec.into_iter().enumerate() {
        hands[idx] = Hand::with_cards(cards);
    }
    // Previous trick just to avoid first-trick rules
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
    RoundState::from_hands_with_state(
        hands,
        starting,
        PassingDirection::Hold,
        RoundPhase::Playing,
        hearts_core::model::trick::Trick::new(starting),
        vec![prev],
        false,
    )
}

fn empty_scores() -> ScoreBoard { ScoreBoard::new() }

#[test]
fn hard_ctrl_handoff_penalty_reduces_cont_when_leading_void_suit() {
    // West leads; has low clubs and low diamonds. East can win clubs; South can win diamonds.
    // Mark North void in clubs to trigger ctrl_handoff penalty when we lose a clubs lead.
    let starting = PlayerPosition::West;
    let west = vec![Card::new(Rank::Two, Suit::Clubs), Card::new(Rank::Two, Suit::Diamonds)];
    let north = vec![Card::new(Rank::Three, Suit::Diamonds), Card::new(Rank::Four, Suit::Diamonds)];
    let east = vec![Card::new(Rank::Ace, Suit::Clubs)];
    let south = vec![Card::new(Rank::Ace, Suit::Diamonds)];
    let round = build_round_not_first_trick(starting, [north, east, south, west.clone()]);
    let scores = empty_scores();
    let mut tracker = UnseenTracker::new();
    tracker.reset_for_round(&round);
    tracker.note_void(PlayerPosition::North, Suit::Clubs);
    let ctx = BotContext::new(
        starting,
        &round,
        &scores,
        PassingDirection::Hold,
        &tracker,
        BotDifficulty::FutureHard,
    );
    let legal = {
        round
            .hand(starting)
            .iter()
            .copied()
            .filter(|card| {
                let mut probe = round.clone();
                probe.play_card(starting, *card).is_ok()
            })
            .collect::<Vec<_>>()
    };
    assert!(legal.contains(&Card::new(Rank::Two, Suit::Clubs)));
    assert!(legal.contains(&Card::new(Rank::Two, Suit::Diamonds)));
    let verbose = PlayPlannerHard::explain_candidates_verbose(&legal, &ctx);
    let mut cont_clubs = None;
    let mut cont_diams = None;
    for (c, _b, cont, _t) in verbose {
        if c == Card::new(Rank::Two, Suit::Clubs) { cont_clubs = Some(cont); }
        if c == Card::new(Rank::Two, Suit::Diamonds) { cont_diams = Some(cont); }
    }
    let cont_clubs = cont_clubs.expect("2C present");
    let cont_diams = cont_diams.expect("2D present");
    assert!(
        cont_clubs <= cont_diams,
        "expected clubs lead cont {} <= diamonds cont {} due to ctrl_handoff_pen",
        cont_clubs,
        cont_diams
    );
}
