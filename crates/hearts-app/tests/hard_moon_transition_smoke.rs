use hearts_app::bot::{BotContext, BotDifficulty, PlayPlannerHard, UnseenTracker};
use hearts_core::model::card::Card;
use hearts_core::model::hand::Hand;
use hearts_core::model::passing::PassingDirection;
use hearts_core::model::player::PlayerPosition;
use hearts_core::model::rank::Rank;
use hearts_core::model::round::{RoundPhase, RoundState};
use hearts_core::model::score::ScoreBoard;
use hearts_core::model::suit::Suit;

fn round_with_lead(
    starting: PlayerPosition,
    leader_play: Card,
    hands_vec: [Vec<Card>; 4],
) -> RoundState {
    let mut hands = [Hand::new(), Hand::new(), Hand::new(), Hand::new()];
    for (idx, cards) in hands_vec.into_iter().enumerate() {
        hands[idx] = Hand::with_cards(cards);
    }
    let mut curr = hearts_core::model::trick::Trick::new(starting);
    curr.play(starting, leader_play).unwrap();
    RoundState::from_hands_with_state(
        hands,
        starting,
        PassingDirection::Hold,
        RoundPhase::Playing,
        curr,
        vec![],
        false,
    )
}

#[test]
fn hard_moon_transition_considering_to_committed_keeps_relief_positive() {
    // Ensure deterministic hard explain and a non-zero moon relief weight
    unsafe {
        std::env::set_var("MDH_HARD_DETERMINISTIC", "1");
        std::env::set_var("MDH_HARD_TEST_STEPS", "100");
        std::env::set_var("MDH_HARD_PHASEB_TOPK", "6");
        std::env::set_var("MDH_HARD_MOON_RELIEF_PERPEN", "5");
    }

    let starting = PlayerPosition::West; // Leader plays a spade; we hold AS to win
    let our_seat = PlayerPosition::North;
    let north = vec![
        Card::new(Rank::Ace, Suit::Spades),
        Card::new(Rank::Two, Suit::Clubs),
    ];
    let east = vec![
        Card::new(Rank::Queen, Suit::Spades),
        Card::new(Rank::Three, Suit::Clubs),
    ];
    let south = vec![
        Card::new(Rank::Four, Suit::Spades),
        Card::new(Rank::Four, Suit::Clubs),
    ];
    let west = vec![
        Card::new(Rank::Two, Suit::Diamonds),
        Card::new(Rank::Three, Suit::Diamonds),
    ];
    let round = round_with_lead(
        starting,
        Card::new(Rank::Seven, Suit::Spades),
        [north, east, south, west],
    );

    let mut tracker = UnseenTracker::new();
    tracker.reset_for_round(&round);
    let scores = ScoreBoard::new();

    // Phase 1: Considering
    tracker.set_moon_state(our_seat, hearts_app::bot::MoonState::Considering);
    let ctx_considering = BotContext::new(
        our_seat,
        &round,
        &scores,
        PassingDirection::Hold,
        &tracker,
        BotDifficulty::FutureHard,
    );
    let legal = round
        .hand(our_seat)
        .iter()
        .copied()
        .filter(|c| {
            let mut p = round.clone();
            p.play_card(our_seat, *c).is_ok()
        })
        .collect::<Vec<_>>();
    assert!(legal.contains(&Card::new(Rank::Ace, Suit::Spades)));
    let verbose_cons = PlayPlannerHard::explain_candidates_verbose_parts(&legal, &ctx_considering);
    let as_entry_cons = verbose_cons
        .into_iter()
        .find(|(c, _b, _p, _t)| *c == Card::new(Rank::Ace, Suit::Spades))
        .expect("AS candidate present");
    let (_c1, _b1, parts_cons, _t1) = as_entry_cons;
    assert!(
        parts_cons.moon_relief > 0,
        "Considering should yield positive moon_relief when winning penalties"
    );

    // Phase 2: Committed (simulate transition) — relief should remain positive
    tracker.set_moon_state(our_seat, hearts_app::bot::MoonState::Committed);
    let ctx_committed = BotContext::new(
        our_seat,
        &round,
        &scores,
        PassingDirection::Hold,
        &tracker,
        BotDifficulty::FutureHard,
    );
    let verbose_comm = PlayPlannerHard::explain_candidates_verbose_parts(&legal, &ctx_committed);
    let as_entry_comm = verbose_comm
        .into_iter()
        .find(|(c, _b, _p, _t)| *c == Card::new(Rank::Ace, Suit::Spades))
        .expect("AS candidate present");
    let (_c2, _b2, parts_comm, _t2) = as_entry_comm;
    assert!(
        parts_comm.moon_relief > 0,
        "Committed should maintain positive moon_relief when winning penalties"
    );

    unsafe {
        std::env::remove_var("MDH_HARD_DETERMINISTIC");
        std::env::remove_var("MDH_HARD_TEST_STEPS");
        std::env::remove_var("MDH_HARD_PHASEB_TOPK");
        std::env::remove_var("MDH_HARD_MOON_RELIEF_PERPEN");
    }
}
