use hearts_app::bot::{BotContext, BotDifficulty, PlayPlannerHard, UnseenTracker};
use hearts_core::model::card::Card;
use hearts_core::model::hand::Hand;
use hearts_core::model::passing::PassingDirection;
use hearts_core::model::player::PlayerPosition;
use hearts_core::model::rank::Rank;
use hearts_core::model::round::{RoundPhase, RoundState};
use hearts_core::model::score::ScoreBoard;
use hearts_core::model::suit::Suit;

fn build_midtrick_follow_suit(
    starting: PlayerPosition,
    plays: &[(PlayerPosition, Card)],
    hands: [Vec<Card>; 4],
) -> RoundState {
    let mut hs = [Hand::new(), Hand::new(), Hand::new(), Hand::new()];
    for (i, cards) in hands.into_iter().enumerate() {
        hs[i] = Hand::with_cards(cards);
    }
    // Seed a previous trick to avoid first-trick rules
    let mut prev = hearts_core::model::trick::Trick::new(starting);
    let mut seat = starting;
    for c in [
        Card::new(Rank::Two, Suit::Clubs),
        Card::new(Rank::Three, Suit::Clubs),
        Card::new(Rank::Four, Suit::Clubs),
        Card::new(Rank::Five, Suit::Clubs),
    ] {
        prev.play(seat, c).unwrap();
        seat = seat.next();
    }
    let mut current = hearts_core::model::trick::Trick::new(starting);
    for &(pos, card) in plays {
        current.play(pos, card).unwrap();
    }
    RoundState::from_hands_with_state(
        hs,
        starting,
        PassingDirection::Hold,
        RoundPhase::Playing,
        current,
        vec![prev],
        true,
    )
}

fn scores_with_self(pos: PlayerPosition, val: u32) -> ScoreBoard {
    let mut s = ScoreBoard::new();
    s.set_score(pos, val);
    s
}

#[test]
fn hard_endgame_avoids_self_capture_when_near_100() {
    // Trick: West leads hearts 7♥, North plays 9♥, East plays 3♥, South to play with 8♥ and 10♥.
    // Current winner is North (9♥). Playing 10♥ would capture a heart point; 8♥ loses safely.
    let starting = PlayerPosition::West;
    let plays = [
        (PlayerPosition::West, Card::new(Rank::Seven, Suit::Hearts)),
        (PlayerPosition::North, Card::new(Rank::Nine, Suit::Hearts)),
        (PlayerPosition::East, Card::new(Rank::Three, Suit::Hearts)),
    ];
    let south_hand = vec![
        Card::new(Rank::Eight, Suit::Hearts),
        Card::new(Rank::Ten, Suit::Hearts),
        Card::new(Rank::Ace, Suit::Clubs),
    ];
    let north_hand = vec![Card::new(Rank::Two, Suit::Spades)];
    let east_hand = vec![Card::new(Rank::Four, Suit::Spades)];
    let west_hand = vec![Card::new(Rank::Five, Suit::Spades)];
    let round = build_midtrick_follow_suit(
        starting,
        &plays,
        [north_hand, east_hand, south_hand.clone(), west_hand],
    );
    let our_seat = PlayerPosition::South;
    let scores = scores_with_self(our_seat, 92);
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
            .filter(|c| {
                let mut p = round.clone();
                p.play_card(our_seat, *c).is_ok()
            })
            .collect::<Vec<_>>()
    };
    assert!(
        legal.contains(&Card::new(Rank::Eight, Suit::Hearts))
            && legal.contains(&Card::new(Rank::Ten, Suit::Hearts))
    );
    let choice = PlayPlannerHard::choose(&legal, &ctx).unwrap();
    assert_eq!(
        choice,
        Card::new(Rank::Eight, Suit::Hearts),
        "Hard should avoid capturing near 100"
    );
}
