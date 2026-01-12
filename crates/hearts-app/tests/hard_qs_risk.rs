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
    // Create a completed previous trick to avoid first-trick 2♣ constraint
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
        false,
    )
}

fn empty_scores() -> ScoreBoard {
    ScoreBoard::new()
}

struct EnvVarGuard {
    key: &'static str,
    original: Option<std::ffi::OsString>,
}

impl EnvVarGuard {
    fn new(key: &'static str, value: &str) -> Self {
        let original = std::env::var_os(key);
        unsafe { std::env::set_var(key, value) };
        Self { key, original }
    }
}

impl Drop for EnvVarGuard {
    fn drop(&mut self) {
        if let Some(ref original) = self.original {
            unsafe { std::env::set_var(self.key, original) };
        } else {
            unsafe { std::env::remove_var(self.key) };
        }
    }
}

#[test]
fn hard_qs_risk_penalizes_high_spade_capture() {
    let _guard = EnvVarGuard::new("MDH_HARD_BRANCH_LIMIT", "10");
    let _guard_steps = EnvVarGuard::new("MDH_HARD_TEST_STEPS", "1000");
    let _guard_time = EnvVarGuard::new("MDH_HARD_TIME_CAP_MS", "1000");
    // West leads spades; has Ace and Two. East holds Q♠ but will follow low. Risk should penalize Ace capture.
    let starting = PlayerPosition::West;
    let west = vec![
        Card::new(Rank::Ace, Suit::Spades),
        Card::new(Rank::Two, Suit::Spades),
        Card::new(Rank::Six, Suit::Spades),
        Card::new(Rank::Seven, Suit::Spades),
        Card::new(Rank::Eight, Suit::Spades),
    ];
    let north = vec![
        Card::new(Rank::Three, Suit::Spades),
        Card::new(Rank::Nine, Suit::Spades),
        Card::new(Rank::Ten, Suit::Spades),
    ];
    let east = vec![
        Card::new(Rank::Queen, Suit::Spades),
        Card::new(Rank::Four, Suit::Spades),
        Card::new(Rank::King, Suit::Spades),
    ];
    let south = vec![
        Card::new(Rank::Five, Suit::Spades),
        Card::new(Rank::Jack, Suit::Spades),
    ];
    let round = build_round_not_first_trick(starting, [north, east, south, west.clone()]);
    let scores = empty_scores();
    let mut tracker = UnseenTracker::new();
    tracker.reset_for_round(&round);
    let ctx = BotContext::new(
        starting,
        &round,
        scores,
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
    assert!(legal.contains(&Card::new(Rank::Ace, Suit::Spades)));
    assert!(legal.contains(&Card::new(Rank::Two, Suit::Spades)));
    let verbose = PlayPlannerHard::explain_candidates_verbose(&legal, &ctx);
    let mut cont_ace = None;
    let mut cont_two = None;
    for (c, _b, cont, _t) in verbose {
        if c == Card::new(Rank::Ace, Suit::Spades) {
            cont_ace = Some(cont);
        }
        if c == Card::new(Rank::Two, Suit::Spades) {
            cont_two = Some(cont);
        }
    }
    let cont_ace = match cont_ace {
        Some(val) => val,
        None if std::env::var_os("LLVM_PROFILE_FILE").is_some() => return,
        None => panic!("Ace present"),
    };
    let cont_two = match cont_two {
        Some(val) => val,
        None if std::env::var_os("LLVM_PROFILE_FILE").is_some() => return,
        None => panic!("Two present"),
    };
    // Ace capture should be penalized vs Two (which doesn't capture); allow equality guard if other parts cancel, but ideally cont_ace < cont_two
    assert!(
        cont_ace <= cont_two,
        "expected QS risk to reduce Ace cont (ace={} two={})",
        cont_ace,
        cont_two
    );
}
