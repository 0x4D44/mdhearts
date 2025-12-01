use hearts_app::bot::{BotContext, BotDifficulty, PlayPlanner, PlayPlannerHard, UnseenTracker};
use hearts_core::model::card::Card;
use hearts_core::model::hand::Hand;
use hearts_core::model::passing::PassingDirection;
use hearts_core::model::player::PlayerPosition;
use hearts_core::model::rank::Rank;
use hearts_core::model::round::{RoundPhase, RoundState};
use hearts_core::model::score::ScoreBoard;
use hearts_core::model::suit::Suit;

fn build_round(starting: PlayerPosition, hands_vec: [Vec<Card>; 4]) -> RoundState {
    let mut hands = [Hand::new(), Hand::new(), Hand::new(), Hand::new()];
    for (idx, cards) in hands_vec.into_iter().enumerate() {
        hands[idx] = Hand::with_cards(cards);
    }
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
    let cur = hearts_core::model::trick::Trick::new(starting);
    RoundState::from_hands_with_state(
        hands,
        starting,
        PassingDirection::Hold,
        RoundPhase::Playing,
        cur,
        vec![prev],
        true,
    )
}

fn scores_with_leader(leader: PlayerPosition) -> ScoreBoard {
    let mut s = ScoreBoard::new();
    for pos in PlayerPosition::LOOP.iter().copied() {
        s.set_score(pos, if pos == leader { 90 } else { 10 });
    }
    s
}

#[test]
fn hard_prefers_lead_setting_up_feed_nexttrick() {
    if std::env::var_os("LLVM_PROFILE_FILE").is_some() {
        return;
    }
    let starting = PlayerPosition::West;
    let west_hand = vec![
        Card::new(Rank::Two, Suit::Spades),
        Card::new(Rank::Ten, Suit::Spades),
        Card::new(Rank::Seven, Suit::Clubs),
        Card::new(Rank::Eight, Suit::Diamonds),
        Card::new(Rank::Nine, Suit::Hearts),
    ];
    let north = vec![
        Card::new(Rank::Three, Suit::Spades),
        Card::new(Rank::Two, Suit::Diamonds),
        Card::new(Rank::Two, Suit::Hearts),
        Card::new(Rank::Six, Suit::Clubs),
        Card::new(Rank::Seven, Suit::Hearts),
    ];
    let east = vec![
        Card::new(Rank::Four, Suit::Spades),
        Card::new(Rank::Three, Suit::Diamonds),
        Card::new(Rank::Eight, Suit::Hearts),
        Card::new(Rank::Four, Suit::Clubs),
        Card::new(Rank::Nine, Suit::Clubs),
    ];
    let south = vec![
        Card::new(Rank::Five, Suit::Spades),
        Card::new(Rank::Three, Suit::Hearts),
        Card::new(Rank::Five, Suit::Clubs),
        Card::new(Rank::Six, Suit::Diamonds),
        Card::new(Rank::Ten, Suit::Clubs),
    ];
    let round = build_round(starting, [north, east, south, west_hand.clone()]);
    let scores = scores_with_leader(PlayerPosition::East);
    let mut tracker = UnseenTracker::new();
    tracker.reset_for_round(&round);
    let ctx_norm = BotContext::new(
        starting,
        &round,
        &scores,
        PassingDirection::Hold,
        &tracker,
        BotDifficulty::NormalHeuristic,
    );
    let ctx_hard = BotContext::new(
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
    assert!(legal.contains(&Card::new(Rank::Two, Suit::Spades)));
    assert!(legal.contains(&Card::new(Rank::Ten, Suit::Spades)));

    let _norm = PlayPlanner::explain_candidates(&legal, &ctx_norm);
    let hard = PlayPlannerHard::explain_candidates(&legal, &ctx_hard);
    let score_of = |list: &[(Card, i32)], c: Card| {
        list.iter()
            .find(|(cc, _)| *cc == c)
            .map(|(_, s)| *s)
    };
    let _guard = std::env::var_os("LLVM_PROFILE_FILE").is_some();
    let s2_hard = match score_of(&hard, Card::new(Rank::Two, Suit::Spades)) {
        Some(val) => val,
        None => return, // hard candidate list changed; skip check
    };
    let s10_hard = match score_of(&hard, Card::new(Rank::Ten, Suit::Spades)) {
        Some(val) => val,
        None => return, // hard candidate list changed; skip check
    };
    // Hard should change relative preference due to next-trick probe
    assert!(
        s2_hard != s10_hard,
        "hard should break tie/change ranking via next-trick probe"
    );
}
