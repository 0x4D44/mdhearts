use hearts_app::bot::{BotContext, BotDifficulty, PlayPlanner, UnseenTracker};
use hearts_core::model::card::Card;
use hearts_core::model::hand::Hand;
use hearts_core::model::passing::PassingDirection;
use hearts_core::model::player::PlayerPosition;
use hearts_core::model::rank::Rank;
use hearts_core::model::round::{RoundPhase, RoundState};
use hearts_core::model::score::ScoreBoard;
use hearts_core::model::suit::Suit;

fn build_round_lead(
    leader: PlayerPosition,
    hands_vec: [Vec<Card>; 4],
    hearts_broken: bool,
) -> RoundState {
    let mut hands = [Hand::new(), Hand::new(), Hand::new(), Hand::new()];
    for (idx, cards) in hands_vec.into_iter().enumerate() {
        hands[idx] = Hand::with_cards(cards);
    }
    let cur = hearts_core::model::trick::Trick::new(leader);
    // Seed with a previous complete trick so this isn't the first trick
    let mut prev = hearts_core::model::trick::Trick::new(leader);
    let mut seat = leader;
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
        leader,
        PassingDirection::Hold,
        RoundPhase::Playing,
        cur,
        vec![prev],
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
fn cautious_early_lead_avoids_hearts_when_possible() {
    // Hearts are already broken, but in early tricks (few cards played), cautious style should avoid leading hearts
    // if a safe low off-suit lead exists.
    let leader = PlayerPosition::South;
    let our_hand = vec![
        Card::new(Rank::Two, Suit::Diamonds), // single diamond -> creates void
        Card::new(Rank::Seven, Suit::Clubs),  // safe club
        Card::new(Rank::Ten, Suit::Hearts),   // hearts available but should be dispreferred early
        Card::new(Rank::Four, Suit::Spades),
        Card::new(Rank::Six, Suit::Spades),
    ];
    // Arrange opponents so that a diamond lead won't cause us to capture (North holds a higher diamond)
    let north = vec![Card::new(Rank::Nine, Suit::Diamonds)];
    let east = vec![Card::new(Rank::Ace, Suit::Clubs)];
    let west = vec![Card::new(Rank::King, Suit::Clubs)];
    let round = build_round_lead(leader, [north, east, our_hand.clone(), west], true);
    let scores = build_scores([10, 15, 12, 16]);
    let mut tracker = UnseenTracker::new();
    tracker.reset_for_round(&round);
    // Simulate early phase by telling context that few cards have been played (tracker already reset → unseen high)
    let ctx = BotContext::new(
        leader,
        &round,
        &scores,
        PassingDirection::Hold,
        &tracker,
        BotDifficulty::NormalHeuristic,
    );
    let legal = {
        round
            .hand(leader)
            .iter()
            .copied()
            .filter(|card| {
                let mut probe = round.clone();
                probe.play_card(leader, *card).is_ok()
            })
            .collect::<Vec<_>>()
    };
    assert!(round.hearts_broken());
    // Expect the planner to choose the low diamond to create a void rather than leading hearts early
    let choice = PlayPlanner::choose(&legal, &ctx).unwrap();
    assert_eq!(choice, Card::new(Rank::Two, Suit::Diamonds));
}

#[test]
fn void_creation_preferred_on_lead_over_neutral_alternative() {
    // When leading, if we can create a void with a single low card in a suit,
    // prefer that over a similar low neutral lead.
    let leader = PlayerPosition::East;
    let our_hand = vec![
        Card::new(Rank::Two, Suit::Diamonds), // single card in suit → void creation bonus
        Card::new(Rank::Three, Suit::Clubs),  // neutral low
        Card::new(Rank::Four, Suit::Spades),
        Card::new(Rank::Six, Suit::Spades),
        Card::new(Rank::Eight, Suit::Hearts),
    ];
    // Ensure a diamond follower will out-rank our 2♦ so we don't capture
    let north = vec![Card::new(Rank::Nine, Suit::Diamonds)];
    let east_hand = our_hand.clone();
    let south = vec![Card::new(Rank::Ace, Suit::Clubs)];
    let west = vec![Card::new(Rank::King, Suit::Clubs)];
    let round = build_round_lead(leader, [north, east_hand, south, west], true);
    let scores = build_scores([10, 10, 10, 10]);
    let mut tracker = UnseenTracker::new();
    tracker.reset_for_round(&round);
    let ctx = BotContext::new(
        leader,
        &round,
        &scores,
        PassingDirection::Hold,
        &tracker,
        BotDifficulty::NormalHeuristic,
    );
    let legal = {
        round
            .hand(leader)
            .iter()
            .copied()
            .filter(|card| {
                let mut probe = round.clone();
                probe.play_card(leader, *card).is_ok()
            })
            .collect::<Vec<_>>()
    };
    assert!(legal.contains(&Card::new(Rank::Two, Suit::Diamonds)));
    assert!(legal.contains(&Card::new(Rank::Three, Suit::Clubs)));
    let choice = PlayPlanner::choose(&legal, &ctx).unwrap();
    assert_eq!(choice, Card::new(Rank::Two, Suit::Diamonds));
}
