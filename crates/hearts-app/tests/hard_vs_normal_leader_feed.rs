use hearts_app::bot::{BotContext, BotDifficulty, PlayPlanner, PlayPlannerHard, UnseenTracker};
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
    // seed with a previous complete trick to avoid first-trick restrictions
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
        true,
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
fn hard_increases_margin_for_leader_feed_qs() {
    // East (leader) leads AC. South cannot follow clubs and can play QS or a safe diamond (5D).
    // Hard continuation should increase QS margin vs safe diamond (due to feed-to-leader bonus).
    let starting = PlayerPosition::East;
    let our_seat = PlayerPosition::South;
    let hands = [
        vec![Card::new(Rank::Two, Suit::Clubs)], // North can follow clubs
        vec![Card::new(Rank::Ace, Suit::Clubs)], // East leads AC
        vec![
            Card::new(Rank::Queen, Suit::Spades), // South off-suit choices
            Card::new(Rank::Five, Suit::Diamonds),
        ],
        vec![Card::new(Rank::King, Suit::Clubs)], // West can follow clubs
    ];
    let round = build_round(
        starting,
        hands,
        &[(PlayerPosition::East, Card::new(Rank::Ace, Suit::Clubs))],
    );
    let scores = build_scores([10, 95, 20, 30]); // East is scoreboard leader (max score in Hearts rules)
    let mut tracker = UnseenTracker::new();
    tracker.reset_for_round(&round);
    let ctx_normal = BotContext::new(
        our_seat,
        &round,
        &scores,
        PassingDirection::Hold,
        &tracker,
        BotDifficulty::NormalHeuristic,
    );
    let ctx_hard = BotContext::new(
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

    let normal_expl = PlayPlanner::explain_candidates(&legal, &ctx_normal);
    let hard_expl = PlayPlannerHard::explain_candidates(&legal, &ctx_hard);

    let score_of = |list: &Vec<(Card, i32)>, card: Card| -> i32 {
        list.iter()
            .find(|(c, _)| *c == card)
            .map(|(_, s)| *s)
            .expect("card present")
    };
    let qs = Card::new(Rank::Queen, Suit::Spades);
    let d5 = Card::new(Rank::Five, Suit::Diamonds);
    let margin_normal = score_of(&normal_expl, qs) - score_of(&normal_expl, d5);
    let margin_hard = score_of(&hard_expl, qs) - score_of(&hard_expl, d5);
    assert!(
        margin_hard > margin_normal,
        "hard margin {} <= normal margin {}",
        margin_hard,
        margin_normal
    );
}
