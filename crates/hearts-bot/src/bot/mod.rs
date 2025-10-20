mod belief;
mod params;
mod pass;
mod play;
mod tracker;

pub use belief::BeliefView;
pub use hearts_core::moon::MoonObjective as Objective;
pub use params::BotParams;
pub use pass::PassPlanner;
pub use play::PlayPlanner;
pub use tracker::UnseenTracker;

use crate::policy::TelemetryContext;
use hearts_core::model::card::Card;
use hearts_core::model::hand::Hand;
use hearts_core::model::passing::PassingDirection;
use hearts_core::model::player::PlayerPosition;
use hearts_core::model::rank::Rank;
use hearts_core::model::round::RoundState;
use hearts_core::model::score::ScoreBoard;
use hearts_core::model::suit::Suit;
use hearts_core::moon::{MoonEstimate, MoonEstimator, MoonFeatures};
use std::sync::OnceLock;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BotDifficulty {
    EasyLegacy,
    NormalHeuristic,
    FutureHard,
}

impl Default for BotDifficulty {
    fn default() -> Self {
        Self::NormalHeuristic
    }
}

impl BotDifficulty {
    pub fn from_env() -> Self {
        static CACHED: OnceLock<BotDifficulty> = OnceLock::new();
        *CACHED.get_or_init(|| match std::env::var("MDH_BOT_DIFFICULTY") {
            Ok(raw) => match raw.trim().to_ascii_lowercase().as_str() {
                "easy" => BotDifficulty::EasyLegacy,
                "legacy" => BotDifficulty::EasyLegacy,
                "normal" => BotDifficulty::NormalHeuristic,
                "default" => BotDifficulty::NormalHeuristic,
                "hard" => BotDifficulty::FutureHard,
                "future" => BotDifficulty::FutureHard,
                _ => BotDifficulty::default(),
            },
            Err(_) => BotDifficulty::default(),
        })
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BotStyle {
    Cautious,
    AggressiveMoon,
    HuntLeader,
}

#[derive(Debug, Clone, Copy)]
pub struct ScoreSnapshot {
    pub min_score: u32,
    pub max_score: u32,
    pub min_player: PlayerPosition,
    pub max_player: PlayerPosition,
}

#[derive(Debug, Clone, Copy)]
pub struct BotFeatures {
    belief_v1: bool,
    void_threshold: f32,
    pass_v2: bool,
}

impl BotFeatures {
    pub const fn new(belief_v1: bool, void_threshold: f32) -> Self {
        Self {
            belief_v1,
            void_threshold,
            pass_v2: false,
        }
    }

    pub fn from_env() -> Self {
        Self::from_reader(|key| std::env::var(key).ok())
    }

    pub const fn belief_enabled(self) -> bool {
        self.belief_v1
    }

    pub const fn void_threshold(self) -> f32 {
        self.void_threshold
    }

    pub const fn pass_v2_enabled(self) -> bool {
        self.pass_v2
    }

    pub fn with_pass_v2(mut self, enabled: bool) -> Self {
        self.pass_v2 = enabled;
        self
    }

    fn from_reader<F>(mut read: F) -> Self
    where
        F: FnMut(&str) -> Option<String>,
    {
        let belief_v1 = read("MDH_ENABLE_BELIEF")
            .map(|raw| matches!(raw.trim(), "1" | "true" | "TRUE" | "on" | "ON"))
            .unwrap_or(false);

        let void_threshold = read("MDH_BELIEF_VOID_THRESHOLD")
            .and_then(|raw| raw.parse::<f32>().ok())
            .filter(|value| value.is_finite() && *value >= 0.0 && *value <= 1.0)
            .unwrap_or(0.15);

        let pass_v2 = read("MDH_PASS_V2")
            .map(|raw| matches!(raw.trim(), "1" | "true" | "TRUE" | "on" | "ON"))
            .unwrap_or(false);

        Self {
            belief_v1,
            void_threshold,
            pass_v2,
        }
    }
}

impl Default for BotFeatures {
    fn default() -> Self {
        Self {
            belief_v1: false,
            void_threshold: 0.15,
            pass_v2: false,
        }
    }
}

#[derive(Debug, Clone, Copy)]
pub struct BotContext<'a> {
    pub seat: PlayerPosition,
    pub round: &'a RoundState,
    pub scores: &'a ScoreBoard,
    pub passing_direction: PassingDirection,
    pub tracker: &'a UnseenTracker,
    pub difficulty: BotDifficulty,
    #[allow(dead_code)]
    pub params: &'a BotParams,
    pub features: BotFeatures,
    belief: Option<BeliefView<'a>>,
    moon_estimate: MoonEstimate,
    telemetry: Option<TelemetryContext<'a>>,
}

impl<'a> BotContext<'a> {
    pub fn new(
        seat: PlayerPosition,
        round: &'a RoundState,
        scores: &'a ScoreBoard,
        passing_direction: PassingDirection,
        tracker: &'a UnseenTracker,
        belief: Option<BeliefView<'a>>,
        features: BotFeatures,
        difficulty: BotDifficulty,
        params: &'a BotParams,
        telemetry: Option<TelemetryContext<'a>>,
    ) -> Self {
        let moon_estimator = MoonEstimator::default();
        let moon_features = MoonFeatures::from_state(seat, round, scores, passing_direction);
        let moon_estimate = moon_estimator.estimate(moon_features);
        Self {
            seat,
            round,
            scores,
            passing_direction,
            tracker,
            belief,
            features,
            difficulty,
            params,
            moon_estimate,
            telemetry,
        }
    }

    pub fn hand(&self) -> &'a Hand {
        self.round.hand(self.seat)
    }

    pub fn cards_played(&self) -> usize {
        52usize.saturating_sub(self.tracker.unseen_count())
    }

    pub fn void_matrix(&self) -> [[bool; 4]; 4] {
        if self.features.belief_enabled() {
            if let Some(belief) = &self.belief {
                return belief.void_matrix();
            }
        }
        self.tracker.infer_voids(self.seat, self.round)
    }

    pub fn belief(&self) -> Option<&BeliefView<'a>> {
        self.belief.as_ref()
    }

    pub fn features(&self) -> BotFeatures {
        self.features
    }

    pub fn moon_estimate(&self) -> MoonEstimate {
        self.moon_estimate
    }

    pub fn objective_hint(&self) -> Objective {
        self.moon_estimate.objective
    }

    pub fn telemetry(&self) -> Option<TelemetryContext<'a>> {
        self.telemetry
    }
}

pub(crate) fn determine_style(ctx: &BotContext<'_>) -> BotStyle {
    let snapshot = snapshot_scores(ctx.scores);
    let my_score = ctx.scores.score(ctx.seat);
    let hand = ctx.hand();

    if should_try_shoot_moon(hand, my_score, snapshot.min_score, ctx.cards_played()) {
        return BotStyle::AggressiveMoon;
    }

    if matches!(ctx.difficulty, BotDifficulty::FutureHard)
        && snapshot.max_score >= 80
        && snapshot.max_player != ctx.seat
    {
        return BotStyle::HuntLeader;
    }

    if snapshot.max_score >= 90 && snapshot.max_player != ctx.seat {
        return BotStyle::HuntLeader;
    }

    BotStyle::Cautious
}

fn should_try_shoot_moon(
    hand: &Hand,
    my_score: u32,
    leader_score: u32,
    cards_played: usize,
) -> bool {
    if cards_played > 12 {
        return false;
    }
    if my_score >= 70 {
        return false;
    }
    if my_score > leader_score + 15 {
        return false;
    }

    let hearts = count_cards_in_suit(hand, Suit::Hearts);
    if hearts < 7 {
        return false;
    }

    let control_hearts = hand
        .iter()
        .filter(|card| card.suit == Suit::Hearts && card.rank >= Rank::Ten)
        .count();
    let high_spades = hand
        .iter()
        .filter(|card| card.suit == Suit::Spades && card.rank >= Rank::Queen)
        .count();
    let has_ace_spades = hand.contains(Card::new(Rank::Ace, Suit::Spades));
    control_hearts >= 4 && high_spades >= 2 && has_ace_spades
}

pub(crate) fn snapshot_scores(scores: &ScoreBoard) -> ScoreSnapshot {
    let mut min_score = u32::MAX;
    let mut max_score = u32::MIN;
    let mut min_player = PlayerPosition::North;
    let mut max_player = PlayerPosition::North;

    for seat in PlayerPosition::LOOP.iter().copied() {
        let value = scores.score(seat);
        if value < min_score {
            min_score = value;
            min_player = seat;
        }
        if value > max_score {
            max_score = value;
            max_player = seat;
        }
    }

    ScoreSnapshot {
        min_score,
        max_score,
        min_player,
        max_player,
    }
}

pub(crate) fn count_cards_in_suit(hand: &Hand, suit: Suit) -> usize {
    hand.iter().filter(|card| card.suit == suit).count()
}

pub(crate) fn card_sort_key(card: Card) -> (u8, u8) {
    (card.suit as u8, card.rank.value())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::bot::BeliefView;
    use hearts_core::belief::Belief;
    use hearts_core::model::round::RoundPhase;
    use std::collections::HashMap;

    fn build_round(seat: PlayerPosition, hand_cards: &[Card]) -> RoundState {
        let mut hands = [Hand::new(), Hand::new(), Hand::new(), Hand::new()];
        hands[seat.index()] = Hand::with_cards(hand_cards.to_vec());
        RoundState::from_hands(hands, seat, PassingDirection::Hold, RoundPhase::Playing)
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

    fn make_tracker(round: &RoundState) -> UnseenTracker {
        let mut tracker = UnseenTracker::new();
        tracker.reset_for_round(round);
        tracker
    }

    #[test]
    fn bot_features_from_env_default_fallbacks() {
        let features = BotFeatures::from_reader(|_| None);
        assert!(!features.pass_v2_enabled());
        assert!(!features.belief_enabled());
        assert!((features.void_threshold() - 0.15).abs() < f32::EPSILON);
    }

    #[test]
    fn bot_features_from_env_respects_flags() {
        let mut vars = HashMap::new();
        vars.insert("MDH_PASS_V2".to_string(), "true".to_string());
        vars.insert("MDH_ENABLE_BELIEF".to_string(), "1".to_string());
        vars.insert("MDH_BELIEF_VOID_THRESHOLD".to_string(), "0.4".to_string());

        let features = BotFeatures::from_reader(|key| vars.get(key).cloned());
        assert!(features.pass_v2_enabled());
        assert!(features.belief_enabled());
        assert!((features.void_threshold() - 0.4).abs() < f32::EPSILON);
    }

    #[test]
    fn style_moon_threshold() {
        let seat = PlayerPosition::South;
        let hand = vec![
            Card::new(Rank::Ace, Suit::Hearts),
            Card::new(Rank::King, Suit::Hearts),
            Card::new(Rank::Queen, Suit::Hearts),
            Card::new(Rank::Jack, Suit::Hearts),
            Card::new(Rank::Ten, Suit::Hearts),
            Card::new(Rank::Nine, Suit::Hearts),
            Card::new(Rank::Eight, Suit::Hearts),
            Card::new(Rank::Ace, Suit::Spades),
            Card::new(Rank::King, Suit::Spades),
            Card::new(Rank::Two, Suit::Clubs),
            Card::new(Rank::Three, Suit::Clubs),
            Card::new(Rank::Four, Suit::Diamonds),
            Card::new(Rank::Five, Suit::Diamonds),
        ];
        let round = build_round(seat, &hand);
        let scores = build_scores([18, 24, 22, 27]);
        let tracker = make_tracker(&round);
        let params = BotParams::default();
        let ctx = BotContext::new(
            seat,
            &round,
            &scores,
            PassingDirection::Hold,
            &tracker,
            None,
            BotFeatures::default(),
            BotDifficulty::NormalHeuristic,
            &params,
            None,
        );
        assert_eq!(determine_style(&ctx), BotStyle::AggressiveMoon);
    }

    #[test]
    fn objective_hint_blocks_for_heavy_hearts() {
        let seat = PlayerPosition::North;
        let hand = vec![
            Card::new(Rank::Ace, Suit::Hearts),
            Card::new(Rank::King, Suit::Hearts),
            Card::new(Rank::Queen, Suit::Hearts),
            Card::new(Rank::Jack, Suit::Hearts),
            Card::new(Rank::Ten, Suit::Hearts),
            Card::new(Rank::Nine, Suit::Hearts),
            Card::new(Rank::Eight, Suit::Hearts),
            Card::new(Rank::Seven, Suit::Hearts),
            Card::new(Rank::Six, Suit::Hearts),
            Card::new(Rank::Five, Suit::Hearts),
            Card::new(Rank::Four, Suit::Clubs),
            Card::new(Rank::Three, Suit::Clubs),
            Card::new(Rank::Two, Suit::Clubs),
        ];
        let round = build_round(seat, &hand);
        let scores = build_scores([20, 18, 22, 16]);
        let tracker = make_tracker(&round);
        let params = BotParams::default();
        let ctx = BotContext::new(
            seat,
            &round,
            &scores,
            PassingDirection::Hold,
            &tracker,
            None,
            BotFeatures::default(),
            BotDifficulty::NormalHeuristic,
            &params,
            None,
        );
        assert_eq!(ctx.objective_hint(), Objective::BlockShooter);
        assert!(ctx.moon_estimate().probability >= 0.32);
    }

    #[test]
    fn objective_hint_prefers_points_for_balanced_hand() {
        let seat = PlayerPosition::East;
        let hand = vec![
            Card::new(Rank::Ace, Suit::Clubs),
            Card::new(Rank::King, Suit::Diamonds),
            Card::new(Rank::Queen, Suit::Spades),
            Card::new(Rank::Jack, Suit::Hearts),
            Card::new(Rank::Ten, Suit::Hearts),
            Card::new(Rank::Nine, Suit::Clubs),
            Card::new(Rank::Eight, Suit::Diamonds),
            Card::new(Rank::Seven, Suit::Spades),
            Card::new(Rank::Six, Suit::Hearts),
            Card::new(Rank::Five, Suit::Clubs),
            Card::new(Rank::Four, Suit::Diamonds),
            Card::new(Rank::Three, Suit::Spades),
            Card::new(Rank::Two, Suit::Hearts),
        ];
        let round = build_round(seat, &hand);
        let scores = build_scores([12, 14, 17, 15]);
        let tracker = make_tracker(&round);
        let params = BotParams::default();
        let ctx = BotContext::new(
            seat,
            &round,
            &scores,
            PassingDirection::Hold,
            &tracker,
            None,
            BotFeatures::default(),
            BotDifficulty::NormalHeuristic,
            &params,
            None,
        );
        assert_eq!(ctx.objective_hint(), Objective::MyPointsPerHand);
        assert!(ctx.moon_estimate().probability < 0.32);
    }

    #[test]
    fn style_hunt_leader_normal() {
        let seat = PlayerPosition::South;
        let hand = vec![
            Card::new(Rank::Ace, Suit::Clubs),
            Card::new(Rank::King, Suit::Diamonds),
            Card::new(Rank::Three, Suit::Spades),
        ];
        let round = build_round(seat, &hand);
        let scores = build_scores([15, 94, 20, 18]);
        let tracker = make_tracker(&round);
        let params = BotParams::default();
        let ctx = BotContext::new(
            seat,
            &round,
            &scores,
            PassingDirection::Hold,
            &tracker,
            None,
            BotFeatures::default(),
            BotDifficulty::NormalHeuristic,
            &params,
            None,
        );
        assert_eq!(determine_style(&ctx), BotStyle::HuntLeader);
    }

    #[test]
    fn style_futurehard_bias() {
        let seat = PlayerPosition::East;
        let hand = vec![
            Card::new(Rank::Ace, Suit::Clubs),
            Card::new(Rank::Five, Suit::Diamonds),
            Card::new(Rank::Seven, Suit::Spades),
        ];
        let round = build_round(seat, &hand);
        let scores = build_scores([40, 50, 82, 60]);
        let tracker = make_tracker(&round);
        let params = BotParams::default();
        let ctx = BotContext::new(
            seat,
            &round,
            &scores,
            PassingDirection::Hold,
            &tracker,
            None,
            BotFeatures::default(),
            BotDifficulty::FutureHard,
            &params,
            None,
        );
        assert_eq!(determine_style(&ctx), BotStyle::HuntLeader);
    }

    #[test]
    fn belief_view_marks_void_when_probability_low() {
        let seat = PlayerPosition::South;
        let mut round = build_round(seat, &[]);

        // Construct a trick where South fails to follow clubs, implying void in clubs.
        let mut trick = hearts_core::model::trick::Trick::new(PlayerPosition::North);
        trick
            .play(PlayerPosition::North, Card::new(Rank::Two, Suit::Clubs))
            .unwrap();
        trick
            .play(PlayerPosition::East, Card::new(Rank::Three, Suit::Clubs))
            .unwrap();
        trick
            .play(PlayerPosition::South, Card::new(Rank::Four, Suit::Hearts))
            .unwrap();
        trick
            .play(PlayerPosition::West, Card::new(Rank::Five, Suit::Clubs))
            .unwrap();
        round.complete_trick(PlayerPosition::North);

        let belief = Belief::from_state(&round, seat);
        let view = BeliefView::new(&belief, 0.2);
        let tracker = make_tracker(&round);
        let params = BotParams::default();
        let scores = build_scores([0, 0, 0, 0]);
        let ctx = BotContext::new(
            seat,
            &round,
            &scores,
            PassingDirection::Hold,
            &tracker,
            Some(view),
            BotFeatures::new(true, 0.2),
            BotDifficulty::NormalHeuristic,
            &params,
            None,
        );

        let matrix = ctx.void_matrix();
        assert!(matrix[seat.index()][Suit::Clubs as usize]);
    }

    #[test]
    fn moon_aborts_after_penalties() {
        let seat = PlayerPosition::South;
        let hand = vec![
            Card::new(Rank::Ace, Suit::Hearts),
            Card::new(Rank::King, Suit::Hearts),
            Card::new(Rank::Queen, Suit::Hearts),
            Card::new(Rank::Jack, Suit::Hearts),
            Card::new(Rank::Ten, Suit::Hearts),
            Card::new(Rank::Nine, Suit::Hearts),
            Card::new(Rank::Eight, Suit::Hearts),
            Card::new(Rank::Ace, Suit::Spades),
            Card::new(Rank::King, Suit::Spades),
            Card::new(Rank::Two, Suit::Clubs),
            Card::new(Rank::Three, Suit::Clubs),
            Card::new(Rank::Four, Suit::Diamonds),
            Card::new(Rank::Five, Suit::Diamonds),
        ];
        let round = build_round(seat, &hand);
        let scores = build_scores([20, 18, 76, 42]);
        let mut tracker = make_tracker(&round);
        for rank in Rank::ORDERED.iter().take(10) {
            tracker.note_card_revealed(Card::new(*rank, Suit::Clubs));
        }
        let params = BotParams::default();
        let ctx = BotContext::new(
            seat,
            &round,
            &scores,
            PassingDirection::Hold,
            &tracker,
            None,
            BotFeatures::default(),
            BotDifficulty::NormalHeuristic,
            &params,
            None,
        );
        assert_eq!(determine_style(&ctx), BotStyle::Cautious);
    }

    #[test]
    fn style_threshold_boundary() {
        let seat = PlayerPosition::North;
        let hand = vec![Card::new(Rank::Ace, Suit::Clubs)];
        let round = build_round(seat, &hand);
        let tracker = make_tracker(&round);

        let params = BotParams::default();
        let style_89 = {
            let scores = build_scores([10, 20, 30, 89]);
            let ctx = BotContext::new(
                seat,
                &round,
                &scores,
                PassingDirection::Hold,
                &tracker,
                None,
                BotFeatures::default(),
                BotDifficulty::NormalHeuristic,
                &params,
                None,
            );
            determine_style(&ctx)
        };
        assert_eq!(style_89, BotStyle::Cautious);

        let style_90 = {
            let scores = build_scores([10, 20, 30, 90]);
            let ctx = BotContext::new(
                seat,
                &round,
                &scores,
                PassingDirection::Hold,
                &tracker,
                None,
                BotFeatures::default(),
                BotDifficulty::NormalHeuristic,
                &params,
                None,
            );
            determine_style(&ctx)
        };
        assert_eq!(style_90, BotStyle::HuntLeader);
    }
}
