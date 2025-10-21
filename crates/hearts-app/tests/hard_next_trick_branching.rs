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
    hearts_broken: bool,
) -> RoundState {
    // Seed a completed trick to avoid first-trick rules.
    let mut hands = [Hand::new(), Hand::new(), Hand::new(), Hand::new()];
    for (idx, cards) in hands_vec.into_iter().enumerate() {
        hands[idx] = Hand::with_cards(cards);
    }
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
    RoundState::from_hands_with_state(
        hands,
        starting,
        PassingDirection::Hold,
        RoundPhase::Playing,
        hearts_core::model::trick::Trick::new(starting),
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
fn hard_next_trick_probe_second_opponent_branching_influences_choice() {
    // Current trick: South leads. If South plays AC, South wins the trick.
    // On next trick, South can lead 3D; West follows low diamond; North is void diamonds and can dump QS;
    // East holds KD and wins the trick, collecting QS (feed to leader). Hard should add positive continuation for AC.
    // If South instead plays TD now, South does not capture current trick => no next-trick probe bonus.

    let starting = PlayerPosition::South; // our seat is leader
    let our_seat = PlayerPosition::South;
    let hands = [
        // North: void in diamonds; holds QS to dump on diamond lead next trick
        vec![Card::new(Rank::Queen, Suit::Spades), Card::new(Rank::Seven, Suit::Clubs)],
        // East (leader target): has KD to win diamond lead, plus some clubs
        vec![Card::new(Rank::King, Suit::Diamonds), Card::new(Rank::Six, Suit::Clubs)],
        // South (our seat): AC to win now; TD (losing now); 3D to lead next; plus a small spade
        vec![
            Card::new(Rank::Ace, Suit::Clubs),
            Card::new(Rank::Ten, Suit::Diamonds),
            Card::new(Rank::Three, Suit::Diamonds),
            Card::new(Rank::Two, Suit::Spades),
        ],
        // West: can follow clubs now and diamonds next with low cards
        vec![Card::new(Rank::Five, Suit::Clubs), Card::new(Rank::Four, Suit::Diamonds)],
    ];
    let round = build_round(starting, hands, true);
    // To ensure AC wins current trick, make others able to follow clubs with lower than Ace.
    // No current trick plays yet.
    let scores = build_scores([40, 95, 45, 50]); // East is scoreboard leader (target)
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

    // Legal moves include AC and TD
    let legal = round
        .hand(our_seat)
        .iter()
        .copied()
        .filter(|card| {
            let mut probe = round.clone();
            probe.play_card(our_seat, *card).is_ok()
        })
        .collect::<Vec<_>>();
    assert!(legal.contains(&Card::new(Rank::Ace, Suit::Clubs)));
    assert!(legal.contains(&Card::new(Rank::Ten, Suit::Diamonds)));

    let verbose = PlayPlannerHard::explain_candidates_verbose(&legal, &ctx);
    let mut ac = None;
    let mut td = None;
    for (c, base, cont, total) in verbose.iter().copied() {
        if c == Card::new(Rank::Ace, Suit::Clubs) {
            ac = Some((base, cont, total));
        }
        if c == Card::new(Rank::Ten, Suit::Diamonds) {
            td = Some((base, cont, total));
        }
    }
    let (_ac_base, ac_cont, _ac_total) = ac.expect("AC present");
    let (_td_base, td_cont, _td_total) = td.expect("TD present");
    // Continuation for AC should be positive due to next-trick probe feeding QS to leader
    assert!(ac_cont > 0, "AC continuation should be positive, got {}", ac_cont);
    // TD should have zero or smaller continuation (no next-trick lead)
    assert!(ac_cont >= td_cont, "AC cont {} should be >= TD cont {}", ac_cont, td_cont);
    // Note: total may still be lower due to base heuristic penalizing current-trick capture.
    // We only assert the continuation signal is present and stronger for AC.
}
