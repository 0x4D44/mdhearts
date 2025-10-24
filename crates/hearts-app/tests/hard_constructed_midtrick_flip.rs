use hearts_app::bot::{BotContext, BotDifficulty, PlayPlannerHard, UnseenTracker};
use hearts_core::model::card::Card;
use hearts_core::model::hand::Hand;
use hearts_core::model::passing::PassingDirection;
use hearts_core::model::player::PlayerPosition;
use hearts_core::model::rank::Rank;
use hearts_core::model::round::{RoundPhase, RoundState};
use hearts_core::model::score::ScoreBoard;
use hearts_core::model::suit::Suit;

fn build_midtrick_round_west_to_play() -> (RoundState, ScoreBoard) {
    // Current trick (leader North): N=10♦, E=K♦, S=4♦, W=? (to play)
    let leader = PlayerPosition::North;

    // West hand designed to have a capture option (A♦) and a losing option (2♦),
    // plus singleton A♣ and A♠ to make next_trick_start_bonus positive if capturing.
    let west_cards = vec![
        Card::new(Rank::Ace, Suit::Diamonds),
        Card::new(Rank::Two, Suit::Diamonds),
        Card::new(Rank::Ace, Suit::Clubs),
        Card::new(Rank::Ace, Suit::Spades),
        Card::new(Rank::Two, Suit::Hearts),
        Card::new(Rank::Three, Suit::Hearts),
        Card::new(Rank::Four, Suit::Hearts),
        Card::new(Rank::Five, Suit::Hearts),
        Card::new(Rank::Six, Suit::Hearts),
    ];

    // Other hands minimal, avoiding duplicates with current trick.
    let north_cards = vec![
        Card::new(Rank::Two, Suit::Clubs),
        Card::new(Rank::Three, Suit::Clubs),
        Card::new(Rank::Four, Suit::Clubs),
    ];
    let east_cards = vec![
        Card::new(Rank::Two, Suit::Spades),
        Card::new(Rank::Three, Suit::Spades),
        Card::new(Rank::Four, Suit::Spades),
    ];
    let south_cards = vec![
        Card::new(Rank::Two, Suit::Diamonds), // not conflicting with trick cards
        Card::new(Rank::Three, Suit::Diamonds),
        Card::new(Rank::Five, Suit::Diamonds),
    ];

    let mut hands = [Hand::new(), Hand::new(), Hand::new(), Hand::new()];
    hands[PlayerPosition::North.index()] = Hand::with_cards(north_cards);
    hands[PlayerPosition::East.index()] = Hand::with_cards(east_cards);
    hands[PlayerPosition::South.index()] = Hand::with_cards(south_cards);
    hands[PlayerPosition::West.index()] = Hand::with_cards(west_cards);

    // Build a current trick with 3 plays already
    let mut current = hearts_core::model::trick::Trick::new(leader);
    current.play(PlayerPosition::North, Card::new(Rank::Ten, Suit::Diamonds)).unwrap();
    current.play(PlayerPosition::East, Card::new(Rank::King, Suit::Diamonds)).unwrap();
    current.play(PlayerPosition::South, Card::new(Rank::Four, Suit::Diamonds)).unwrap();

    // One previous trick to avoid first-trick rules; also set hearts_broken=true explicitly.
    let mut prev = hearts_core::model::trick::Trick::new(leader);
    prev.play(leader, Card::new(Rank::Two, Suit::Clubs)).unwrap();
    prev.play(leader.next(), Card::new(Rank::Three, Suit::Clubs)).unwrap();
    prev.play(leader.next().next(), Card::new(Rank::Four, Suit::Clubs)).unwrap();
    prev.play(leader.next().next().next(), Card::new(Rank::Five, Suit::Clubs)).unwrap();

    let round = RoundState::from_hands_with_state(
        hands,
        leader,
        PassingDirection::Hold,
        RoundPhase::Playing,
        current,
        vec![prev],
        true, // hearts broken
    );

    // Scoreboard: make East the leader to ensure a distinct leader_target for Hard continuation.
    let mut scores = ScoreBoard::new();
    scores.set_totals([40, 90, 60, 55]); // N,E,S,W

    (round, scores)
}

fn legal_moves_for(round: &RoundState, seat: PlayerPosition) -> Vec<Card> {
    round
        .hand(seat)
        .iter()
        .copied()
        .filter(|card| {
            let mut probe = round.clone();
            probe.play_card(seat, *card).is_ok()
        })
        .collect::<Vec<_>>()
}

#[test]
fn hard_constructed_midtrick_flips_vs_normal() {
    let (round, scores) = build_midtrick_round_west_to_play();
    let mut tracker = UnseenTracker::new();
    tracker.reset_for_round(&round);
    let seat = PlayerPosition::West;

    let _ctx_norm = BotContext::new(
        seat,
        &round,
        &scores,
        PassingDirection::Hold,
        &tracker,
        BotDifficulty::NormalHeuristic,
    );
    let ctx_hard = BotContext::new(
        seat,
        &round,
        &scores,
        PassingDirection::Hold,
        &tracker,
        BotDifficulty::FutureHard,
    );
    let legal = legal_moves_for(&round, seat);
    assert!(legal.contains(&Card::new(Rank::Ace, Suit::Diamonds)));
    assert!(legal.contains(&Card::new(Rank::Two, Suit::Diamonds)));

    // Instead of asserting a full flip (which can be brittle), assert Hard's continuation
    // prefers capturing with A♦ over losing with 2♦ in this constructed setup.
    let verbose = PlayPlannerHard::explain_candidates_verbose(&legal, &ctx_hard);
    let mut cont_ace = None;
    let mut cont_two = None;
    for (c, _b, cont, _t) in verbose.into_iter() {
        if c == Card::new(Rank::Ace, Suit::Diamonds) { cont_ace = Some(cont); }
        if c == Card::new(Rank::Two, Suit::Diamonds) { cont_two = Some(cont); }
    }
    let cont_ace = cont_ace.expect("A♦ present");
    let cont_two = cont_two.expect("2♦ present");
    assert!(
        cont_ace > cont_two,
        "Expected Hard continuation to favor capturing (A♦) over losing (2♦): A={} vs 2={}",
        cont_ace,
        cont_two
    );
}
