use hearts_app::bot::{BotContext, BotDifficulty, DecisionLimit, PlayPlannerHard, UnseenTracker};
use hearts_app::controller::{GameController, TimeoutFallback};
use hearts_core::model::card::Card;
use hearts_core::model::hand::Hand;
use hearts_core::model::passing::PassingDirection;
use hearts_core::model::player::PlayerPosition;
use hearts_core::model::rank::Rank;
use hearts_core::model::round::{RoundPhase, RoundState};
use hearts_core::model::score::ScoreBoard;
use hearts_core::model::suit::Suit;
use std::sync::Mutex;
use std::time::{Duration, Instant};

// Helper to serialize environment changes
static ENV_LOCK: Mutex<()> = Mutex::new(());

struct EnvGuard {
    key: &'static str,
    original: Option<String>,
}

impl EnvGuard {
    fn new(key: &'static str, value: &str) -> Self {
        let _lock = ENV_LOCK.lock().unwrap();
        let original = std::env::var(key).ok();
        unsafe { std::env::set_var(key, value) };
        Self { key, original }
    }
}

impl Drop for EnvGuard {
    fn drop(&mut self) {
        let _lock = ENV_LOCK.lock().unwrap();
        if let Some(ref val) = self.original {
            unsafe { std::env::set_var(self.key, val) };
        } else {
            unsafe { std::env::remove_var(self.key) };
        }
    }
}

fn make_context<'a>(
    seat: PlayerPosition,
    round: &'a RoundState,
    scores: ScoreBoard,
    tracker: &'a UnseenTracker,
) -> BotContext<'a> {
    BotContext::new(
        seat,
        round,
        scores,
        PassingDirection::Left,
        tracker,
        BotDifficulty::SearchLookahead,
    )
}

#[test]
fn test_hard_planner_respects_branch_limit_env() {
    let _guard = EnvGuard::new("MDH_HARD_BRANCH_LIMIT", "1");

    // Setup a simple round with cards for everyone to avoid simulation panics
    let hand = vec![
        Card::new(Rank::Two, Suit::Clubs),
        Card::new(Rank::Three, Suit::Clubs),
    ];
    let other_hand = vec![Card::new(Rank::Four, Suit::Clubs)];
    let hands = [
        Hand::with_cards(hand.clone()),
        Hand::with_cards(other_hand.clone()),
        Hand::with_cards(other_hand.clone()),
        Hand::with_cards(other_hand.clone()),
    ];
    let round = RoundState::from_hands(
        hands,
        PlayerPosition::North,
        PassingDirection::Left,
        RoundPhase::Playing,
    );
    let mut tracker = UnseenTracker::new();
    tracker.reset_for_round(&round);
    let scores = ScoreBoard::new();
    let ctx = make_context(PlayerPosition::North, &round, scores, &tracker);

    let candidates = PlayPlannerHard::explain_candidates_verbose(&hand, &ctx);
    assert_eq!(candidates.len(), 1, "Should respect branch limit of 1");
}

#[test]
fn test_hard_planner_respects_time_limit_expired() {
    let hand = vec![Card::new(Rank::Two, Suit::Clubs)];
    let other_hand = vec![Card::new(Rank::Four, Suit::Clubs)];
    let hands = [
        Hand::with_cards(hand.clone()),
        Hand::with_cards(other_hand.clone()),
        Hand::with_cards(other_hand.clone()),
        Hand::with_cards(other_hand.clone()),
    ];
    let round = RoundState::from_hands(
        hands,
        PlayerPosition::North,
        PassingDirection::Left,
        RoundPhase::Playing,
    );
    let mut tracker = UnseenTracker::new();
    tracker.reset_for_round(&round);
    let scores = ScoreBoard::new();
    let ctx = make_context(PlayerPosition::North, &round, scores, &tracker);

    let past = Instant::now() - Duration::from_secs(1);
    let limit = DecisionLimit {
        deadline: Some(past),
        cancel: None,
    };

    let choice = PlayPlannerHard::choose_with_limit(&hand, &ctx, Some(&limit));
    assert!(
        choice.is_none(),
        "Should return None when time limit expired"
    );
}

#[test]
fn test_controller_timeout_fallback() {
    // We use the MDH_TEST_FORCE_AUTOP_TIMEOUT logic in controller to simulate timeout
    let _guard = EnvGuard::new("MDH_TEST_FORCE_AUTOP_TIMEOUT", "1");

    let mut controller = GameController::new_with_seed(Some(123), PlayerPosition::North);
    // Setup passing phase handling
    if controller.in_passing_phase() {
        let p = controller.simple_pass_for(PlayerPosition::South).unwrap();
        controller.submit_pass(PlayerPosition::South, p).unwrap();
        controller
            .submit_auto_passes_for_others(PlayerPosition::South)
            .unwrap();
        controller.resolve_passes().unwrap();
    }

    let mut cfg = controller.think_config();
    cfg.fallback = TimeoutFallback::FirstLegal;
    controller.set_think_config(cfg);

    // Ensure it is South's turn or navigate to it
    while controller.expected_to_play() != PlayerPosition::South {
        let seat = controller.expected_to_play();
        let (s, _) = controller.autoplay_one(PlayerPosition::South).unwrap();
        assert_eq!(s, seat);
    }

    let seat = controller.expected_to_play();
    assert_eq!(seat, PlayerPosition::South);

    // This call should timeout (forced) and fallback to FirstLegal
    let result = controller.autoplay_one_with_status(seat.next());
    match result {
        hearts_app::controller::AutoplayOutcome::Played(s, _) => {
            assert_eq!(s, seat);
        }
        _ => panic!("Expected Played outcome from fallback, got {:?}", result),
    }
}

#[test]
fn test_continuation_schedule_loading() {
    use std::io::Write;
    let mut temp = std::env::temp_dir();
    temp.push("test_schedule.json");
    let json = r#"{
        "snnh": {
            "north": {
                "limits_ms": [1000],
                "continuation_scale": [1200]
            }
        }
    }"#;
    std::fs::File::create(&temp)
        .unwrap()
        .write_all(json.as_bytes())
        .unwrap();

    let _guard = EnvGuard::new("MDH_CONT_SCHEDULE_PATH", temp.to_str().unwrap());

    // Trigger lazy loading by creating a context and calling explain
    // We need a context where seat matches the schedule (North)
    let hand = vec![Card::new(Rank::Two, Suit::Clubs)];
    let other_hand = vec![Card::new(Rank::Four, Suit::Clubs)];
    let hands = [
        Hand::with_cards(hand.clone()),
        Hand::with_cards(other_hand.clone()),
        Hand::with_cards(other_hand.clone()),
        Hand::with_cards(other_hand.clone()),
    ];
    let round = RoundState::from_hands(
        hands,
        PlayerPosition::North,
        PassingDirection::Left,
        RoundPhase::Playing,
    );
    let mut tracker = UnseenTracker::new();
    tracker.reset_for_round(&round);
    let scores = ScoreBoard::new();
    let ctx = make_context(PlayerPosition::North, &round, scores, &tracker);

    // We set mix hint to snnh to match the schedule
    let _guard_mix = EnvGuard::new("MDH_SEARCH_MIX_HINT", "snnh");

    // explain_candidates calls schedule loading
    let _ = PlayPlannerHard::explain_candidates(&hand, &ctx);

    // cleanup
    let _ = std::fs::remove_file(temp);
}

#[test]
fn test_feature_flags_toggle() {
    let _guard1 = EnvGuard::new("MDH_FEATURE_HARD_STAGE1", "1");
    let _guard2 = EnvGuard::new("MDH_FEATURE_HARD_STAGE2", "0");

    // Just run a decision to ensure no panic and code paths are exercised
    let hand = vec![Card::new(Rank::Two, Suit::Clubs)];
    let other_hand = vec![Card::new(Rank::Four, Suit::Clubs)];
    let hands = [
        Hand::with_cards(hand.clone()),
        Hand::with_cards(other_hand.clone()),
        Hand::with_cards(other_hand.clone()),
        Hand::with_cards(other_hand.clone()),
    ];
    let round = RoundState::from_hands(
        hands,
        PlayerPosition::North,
        PassingDirection::Left,
        RoundPhase::Playing,
    );
    let mut tracker = UnseenTracker::new();
    tracker.reset_for_round(&round);
    let scores = ScoreBoard::new();
    let ctx = make_context(PlayerPosition::North, &round, scores, &tracker);

    let _ = PlayPlannerHard::explain_candidates(&hand, &ctx);
}
