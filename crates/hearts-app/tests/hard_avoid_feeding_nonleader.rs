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
) -> RoundState {
    let mut hands = [Hand::new(), Hand::new(), Hand::new(), Hand::new()];
    for (idx, cards) in hands_vec.into_iter().enumerate() {
        hands[idx] = Hand::with_cards(cards);
    }
    // Seed prior trick to avoid first-trick rules
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
    for &(p, c) in plays {
        current.play(p, c).unwrap();
    }
    RoundState::from_hands_with_state(
        hands,
        starting,
        PassingDirection::Hold,
        RoundPhase::Playing,
        current,
        vec![prev],
        true,
    )
}

fn scores(n: u32, e: u32, s: u32, w: u32) -> ScoreBoard {
    let mut sb = ScoreBoard::new();
    sb.set_totals([n, e, s, w]);
    sb
}

#[test]
fn hard_avoids_feeding_nonleader_when_penalties_on_table() {
    // West leads 5H; North plays 9H (provisional winner is North, a non-leader).
    // East (our seat) is void in hearts and can dump QS or a safe club. West is scoreboard leader.
    let starting = PlayerPosition::West;
    let hands = [
        vec![Card::new(Rank::Six, Suit::Spades)], // North
        vec![
            Card::new(Rank::Queen, Suit::Spades),
            Card::new(Rank::Five, Suit::Clubs),
        ], // East (our seat), void hearts
        vec![Card::new(Rank::Four, Suit::Spades)], // South
        vec![Card::new(Rank::Ace, Suit::Spades)], // West (leader)
    ];
    // Order from West: West -> North -> East -> South. We play after North.
    let plays = [
        (PlayerPosition::West, Card::new(Rank::Five, Suit::Hearts)),
        (PlayerPosition::North, Card::new(Rank::Nine, Suit::Hearts)),
    ];
    let round = build_round(starting, hands, &plays);
    let scores = scores(40, 60, 45, 80); // West is scoreboard leader
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
    let legal = {
        round
            .hand(seat)
            .iter()
            .copied()
            .filter(|c| {
                let mut p = round.clone();
                p.play_card(seat, *c).is_ok()
            })
            .collect::<Vec<_>>()
    };
    assert!(
        legal.contains(&Card::new(Rank::Queen, Suit::Spades))
            && legal.contains(&Card::new(Rank::Five, Suit::Clubs))
    );
    let choice = PlayPlannerHard::choose(&legal, &ctx).unwrap();
    assert_eq!(
        choice,
        Card::new(Rank::Five, Suit::Clubs),
        "Should avoid feeding QS to non-leader when penalties on table"
    );
}
