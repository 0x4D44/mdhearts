use hearts_app::bot::{BotContext, BotDifficulty, PlayPlannerHard, UnseenTracker};
use hearts_core::model::card::Card;
use hearts_core::model::hand::Hand;
use hearts_core::model::passing::PassingDirection;
use hearts_core::model::player::PlayerPosition;
use hearts_core::model::rank::Rank;
use hearts_core::model::round::{RoundPhase, RoundState};
use hearts_core::model::score::ScoreBoard;
use hearts_core::model::suit::Suit;

fn build_round(starting: PlayerPosition, hands_vec: [Vec<Card>; 4], plays: &[(PlayerPosition, Card)]) -> RoundState {
    let mut hands = [Hand::new(), Hand::new(), Hand::new(), Hand::new()];
    for (idx, cards) in hands_vec.into_iter().enumerate() { hands[idx] = Hand::with_cards(cards); }
    // Seed previous trick to avoid first-trick rules
    let mut prev = hearts_core::model::trick::Trick::new(starting);
    let mut seat = starting;
    for c in [
        Card::new(Rank::Two, Suit::Clubs),
        Card::new(Rank::Three, Suit::Clubs),
        Card::new(Rank::Four, Suit::Clubs),
        Card::new(Rank::Five, Suit::Clubs),
    ] { prev.play(seat, c).unwrap(); seat = seat.next(); }
    let mut current = hearts_core::model::trick::Trick::new(starting);
    for &(p, c) in plays { current.play(p, c).unwrap(); }
    RoundState::from_hands_with_state(hands, starting, PassingDirection::Hold, RoundPhase::Playing, current, vec![prev], true)
}

fn scores(n: u32, e: u32, s: u32, w: u32) -> ScoreBoard {
    let mut sb = ScoreBoard::new();
    sb.set_totals([n, e, s, w]);
    sb
}

#[test]
fn hard_prefers_dumping_qs_over_small_heart_to_leader() {
    // West (leader) plays AC, North plays 2C, East plays 3C. South is void and can dump QS or 5H; West is scoreboard leader.
    // Expect Hard to prefer QS to maximize penalties fed to leader on the trick.
    let starting = PlayerPosition::West;
    let hands = [
        vec![Card::new(Rank::Six, Suit::Spades)],           // North
        vec![Card::new(Rank::Seven, Suit::Spades)],          // East
        vec![Card::new(Rank::Queen, Suit::Spades), Card::new(Rank::Five, Suit::Hearts)], // South (our seat)
        vec![Card::new(Rank::Four, Suit::Diamonds)],         // West
    ];
    let plays = [
        (PlayerPosition::West, Card::new(Rank::Ace, Suit::Clubs)),
        (PlayerPosition::North, Card::new(Rank::Two, Suit::Clubs)),
        (PlayerPosition::East, Card::new(Rank::Three, Suit::Clubs)),
    ];
    let round = build_round(starting, hands, &plays);
    let scores = scores(40, 50, 45, 80); // West (leader) has highest score
    let mut tracker = UnseenTracker::new();
    tracker.reset_for_round(&round);
    let seat = PlayerPosition::South;
    let ctx = BotContext::new(seat, &round, &scores, PassingDirection::Hold, &tracker, BotDifficulty::FutureHard);
    let legal = {
        round.hand(seat).iter().copied().filter(|c| { let mut p = round.clone(); p.play_card(seat, *c).is_ok() }).collect::<Vec<_>>()
    };
    assert!(legal.contains(&Card::new(Rank::Queen, Suit::Spades)) && legal.contains(&Card::new(Rank::Five, Suit::Hearts)));
    let verbose = PlayPlannerHard::explain_candidates_verbose(&legal, &ctx);
    let mut qs_total = None; let mut h_total = None;
    for (c, _b, _cont, t) in verbose.iter().copied() {
        if c == Card::new(Rank::Queen, Suit::Spades) { qs_total = Some(t); }
        if c == Card::new(Rank::Five, Suit::Hearts) { h_total = Some(t); }
    }
    let qs_total = qs_total.expect("QS present");
    let h_total = h_total.expect("5H present");
    assert!(qs_total >= h_total, "QS total {} should be >= heart total {} when feeding leader", qs_total, h_total);
    let choice = PlayPlannerHard::choose(&legal, &ctx).unwrap();
    assert_eq!(choice, Card::new(Rank::Queen, Suit::Spades));
}

