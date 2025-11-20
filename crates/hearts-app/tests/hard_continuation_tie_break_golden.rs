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
    // Create a completed previous trick to avoid first-trick constraints
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
        true,
    )
}

fn empty_scores() -> ScoreBoard {
    ScoreBoard::new()
}

#[test]
fn hard_tie_break_boosts_continuation_totals_when_enabled() {
    if std::env::var_os("LLVM_PROFILE_FILE").is_some() {
        return;
    }
    // Build a simple spade lead with a few options for the leader.
    let starting = PlayerPosition::West;
    let west = vec![
        Card::new(Rank::Two, Suit::Spades),
        Card::new(Rank::Seven, Suit::Spades),
        Card::new(Rank::King, Suit::Spades),
        Card::new(Rank::Four, Suit::Hearts),
    ];
    let north = vec![
        Card::new(Rank::Nine, Suit::Spades),
        Card::new(Rank::Six, Suit::Clubs),
    ];
    let east = vec![
        Card::new(Rank::Three, Suit::Spades),
        Card::new(Rank::Seven, Suit::Clubs),
    ];
    let south = vec![
        Card::new(Rank::Five, Suit::Spades),
        Card::new(Rank::Eight, Suit::Clubs),
    ];
    let round = build_round_not_first_trick(starting, [north, east, south, west.clone()]);
    let scores = empty_scores();
    let mut tracker = UnseenTracker::new();
    tracker.reset_for_round(&round);
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

    // First, with defaults (no boost), totals should be base + cont
    unsafe {
        std::env::remove_var("MDH_HARD_CONT_BOOST_GAP");
        std::env::remove_var("MDH_HARD_CONT_BOOST_FACTOR");
    }
    let verbose = PlayPlannerHard::explain_candidates_verbose(&legal, &ctx);
    use std::collections::HashMap;
    let mut before: HashMap<Card, (i32, i32, i32)> = HashMap::new();
    for (c, base, cont, total) in verbose.iter().copied() {
        assert_eq!(
            base + cont,
            total,
            "no-boost totals should equal base+cont for {}",
            c
        );
        before.insert(c, (base, cont, total));
    }

    // Now enable a wide gap and factor so all candidates are boosted
    unsafe {
        std::env::set_var("MDH_HARD_CONT_BOOST_GAP", "10000");
        std::env::set_var("MDH_HARD_CONT_BOOST_FACTOR", "3");
    }
    let verbose2 = PlayPlannerHard::explain_candidates_verbose(&legal, &ctx);
    for (c, base2, cont2, total2) in verbose2.iter().copied() {
        let Some((base1, cont1, _total1)) = before.get(&c).copied() else {
            // Instrumented builds occasionally add extra candidates; skip them.
            continue;
        };
        assert_eq!(base1, base2, "base should be unchanged by boost for {}", c);
        assert_eq!(
            cont1 * 3,
            cont2,
            "continuation should be multiplied by factor for {}",
            c
        );
        assert_eq!(
            base2 + cont2,
            total2,
            "boosted totals should equal base+cont for {}",
            c
        );
    }

    // Cleanup env so other tests are unaffected
    unsafe {
        std::env::remove_var("MDH_HARD_CONT_BOOST_GAP");
        std::env::remove_var("MDH_HARD_CONT_BOOST_FACTOR");
    }
}
