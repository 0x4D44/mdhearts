//! Reward computation for reinforcement learning.
//!
//! This module defines different reward modes for training RL agents,
//! including terminal rewards, per-trick rewards, and shaped rewards
//! with intermediate feedback.

use hearts_core::game::match_state::MatchState;
use hearts_core::model::player::PlayerPosition;

/// Step reward computation mode for RL training
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StepRewardMode {
    /// Only provide reward at episode termination
    Terminal,
    /// Provide reward after each completed trick
    PerTrick,
    /// Shaped rewards with intermediate feedback
    Shaped,
}

impl StepRewardMode {
    /// Parse reward mode from string
    pub fn from_str(s: &str) -> Result<Self, String> {
        match s.to_ascii_lowercase().as_str() {
            "terminal" => Ok(StepRewardMode::Terminal),
            "per_trick" | "pertrick" => Ok(StepRewardMode::PerTrick),
            "shaped" => Ok(StepRewardMode::Shaped),
            _ => Err(format!("Unknown step reward mode: {}", s)),
        }
    }

    /// Convert to string representation
    #[allow(dead_code)]
    pub fn as_str(&self) -> &str {
        match self {
            StepRewardMode::Terminal => "terminal",
            StepRewardMode::PerTrick => "per_trick",
            StepRewardMode::Shaped => "shaped",
        }
    }
}

/// Computes rewards for RL training
pub struct RewardComputer {
    mode: StepRewardMode,
}

impl RewardComputer {
    /// Create a new reward computer with the specified mode
    pub fn new(mode: StepRewardMode) -> Self {
        Self { mode }
    }

    /// Compute step reward for a player
    ///
    /// # Arguments
    /// * `match_state` - Current match state
    /// * `seat` - Player position
    /// * `prev_hand_size` - Hand size before the action
    /// * `prev_tricks_completed` - Number of tricks completed before the action
    ///
    /// # Returns
    /// Reward value (typically in [-1, 0] range)
    pub fn compute_step_reward(
        &self,
        match_state: &MatchState,
        seat: PlayerPosition,
        prev_hand_size: usize,
        prev_tricks_completed: usize,
        prev_penalties: [u8; 4],
    ) -> f32 {
        match self.mode {
            StepRewardMode::Terminal => 0.0,
            StepRewardMode::PerTrick => {
                self.per_trick_reward(match_state, seat, prev_tricks_completed, prev_penalties)
            }
            StepRewardMode::Shaped => {
                self.shaped_reward(match_state, seat, prev_hand_size, prev_penalties)
            }
        }
    }

    /// Compute terminal reward at end of episode
    ///
    /// # Arguments
    /// * `match_state` - Final match state
    /// * `seat` - Player position
    ///
    /// # Returns
    /// Terminal reward normalized to [-1, 0]
    pub fn compute_terminal_reward(&self, match_state: &MatchState, seat: PlayerPosition) -> f32 {
        let penalty_totals = match_state.round().penalty_totals();
        let our_points = penalty_totals[seat.index()] as f32;

        // Normalize to [-1, 0] range
        // 0 points = 0 reward (best)
        // 26 points = -1 reward (worst)
        -our_points / 26.0
    }

    /// Per-trick reward: penalize when taking points
    fn per_trick_reward(
        &self,
        match_state: &MatchState,
        seat: PlayerPosition,
        prev_tricks_completed: usize,
        prev_penalties: [u8; 4],
    ) -> f32 {
        let round = match_state.round();
        let current_tricks = round.tricks_completed();
        let penalties = round.penalty_totals();
        let delta = penalties[seat.index()] as i32 - prev_penalties[seat.index()] as i32;

        // Just completed a trick?
        if current_tricks > prev_tricks_completed && delta > 0 {
            -(delta as f32) / 26.0
        } else {
            0.0
        }
    }

    /// Shaped reward with intermediate feedback
    fn shaped_reward(
        &self,
        match_state: &MatchState,
        seat: PlayerPosition,
        prev_hand_size: usize,
        prev_penalties: [u8; 4],
    ) -> f32 {
        let round = match_state.round();
        let current_hand_size = round.hand(seat).len();
        let penalties = round.penalty_totals();
        let delta = penalties[seat.index()] as i32 - prev_penalties[seat.index()] as i32;

        // Did we just play a card?
        if current_hand_size < prev_hand_size && delta > 0 {
            return -(delta as f32) / 26.0;
        }

        0.0
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use hearts_core::game::match_state::MatchState;
    use hearts_core::model::player::PlayerPosition;

    #[test]
    fn reward_mode_from_str() {
        assert_eq!(
            StepRewardMode::from_str("terminal").unwrap(),
            StepRewardMode::Terminal
        );
        assert_eq!(
            StepRewardMode::from_str("per_trick").unwrap(),
            StepRewardMode::PerTrick
        );
        assert_eq!(
            StepRewardMode::from_str("shaped").unwrap(),
            StepRewardMode::Shaped
        );
        assert!(StepRewardMode::from_str("invalid").is_err());
    }

    #[test]
    fn terminal_reward_zero_points_is_zero() {
        let computer = RewardComputer::new(StepRewardMode::Terminal);
        let match_state = MatchState::with_seed(PlayerPosition::South, 42);

        // At start, no points taken yet
        let reward = computer.compute_terminal_reward(&match_state, PlayerPosition::South);

        // Should be 0 since no points
        assert_eq!(reward, 0.0);
    }

    #[test]
    fn terminal_mode_gives_zero_step_reward() {
        let computer = RewardComputer::new(StepRewardMode::Terminal);
        let match_state = MatchState::with_seed(PlayerPosition::South, 42);
        let prev_penalties = match_state.round().penalty_totals();

        let reward = computer.compute_step_reward(
            &match_state,
            PlayerPosition::South,
            13,
            0,
            prev_penalties,
        );

        assert_eq!(reward, 0.0);
    }

    #[test]
    fn per_trick_mode_gives_zero_when_no_trick_completed() {
        let computer = RewardComputer::new(StepRewardMode::PerTrick);
        let match_state = MatchState::with_seed(PlayerPosition::South, 42);
        let prev_penalties = match_state.round().penalty_totals();

        // Same trick count, so no reward
        let reward = computer.compute_step_reward(
            &match_state,
            PlayerPosition::South,
            13,
            0,
            prev_penalties,
        );

        assert_eq!(reward, 0.0);
    }

    #[test]
    fn shaped_mode_gives_zero_when_hand_unchanged() {
        let computer = RewardComputer::new(StepRewardMode::Shaped);
        let match_state = MatchState::with_seed(PlayerPosition::South, 42);
        let prev_penalties = match_state.round().penalty_totals();

        let hand_size = match_state.round().hand(PlayerPosition::South).len();

        // Hand size unchanged, so no reward
        let reward = computer.compute_step_reward(
            &match_state,
            PlayerPosition::South,
            hand_size,
            0,
            prev_penalties,
        );

        assert_eq!(reward, 0.0);
    }

    #[test]
    fn shaped_reward_penalizes_points() {
        use hearts_core::model::card::Card;
        use hearts_core::model::hand::Hand;
        use hearts_core::model::passing::PassingDirection;
        use hearts_core::model::rank::Rank;
        use hearts_core::model::round::{RoundPhase, RoundState};
        use hearts_core::model::suit::Suit;
        use hearts_core::model::trick::Trick;

        let mut trick = Trick::new(PlayerPosition::South);
        trick
            .play(PlayerPosition::South, Card::new(Rank::Ace, Suit::Hearts))
            .unwrap();
        trick
            .play(PlayerPosition::West, Card::new(Rank::Queen, Suit::Spades))
            .unwrap();
        trick
            .play(PlayerPosition::North, Card::new(Rank::Two, Suit::Hearts))
            .unwrap();
        trick
            .play(PlayerPosition::East, Card::new(Rank::Three, Suit::Hearts))
            .unwrap();
        assert_eq!(trick.winner(), Some(PlayerPosition::South));
        assert_eq!(trick.penalty_total(), 16);

        let hands = [
            Hand::with_cards(Vec::<Card>::new()),
            Hand::with_cards(Vec::<Card>::new()),
            Hand::with_cards(Vec::<Card>::new()),
            Hand::with_cards(Vec::<Card>::new()),
        ];
        let round = RoundState::from_hands_with_state(
            hands,
            PlayerPosition::South,
            PassingDirection::Hold,
            RoundPhase::Playing,
            Trick::new(PlayerPosition::South),
            vec![trick],
            true,
        );

        let mut match_state = MatchState::with_seed(PlayerPosition::South, 0);
        *match_state.round_mut() = round;

        let computer = RewardComputer::new(StepRewardMode::Shaped);
        let prev_penalties = [0, 0, 0, 0];
        let reward =
            computer.compute_step_reward(&match_state, PlayerPosition::South, 1, 0, prev_penalties);
        assert!((reward - (-16.0 / 26.0)).abs() < f32::EPSILON);
    }

    #[test]
    fn per_trick_reward_penalizes_delta() {
        use hearts_core::model::card::Card;
        use hearts_core::model::hand::Hand;
        use hearts_core::model::passing::PassingDirection;
        use hearts_core::model::rank::Rank;
        use hearts_core::model::round::{RoundPhase, RoundState};
        use hearts_core::model::suit::Suit;
        use hearts_core::model::trick::Trick;

        let mut trick = Trick::new(PlayerPosition::South);
        trick
            .play(PlayerPosition::South, Card::new(Rank::King, Suit::Hearts))
            .unwrap();
        trick
            .play(PlayerPosition::West, Card::new(Rank::Queen, Suit::Spades))
            .unwrap();
        trick
            .play(PlayerPosition::North, Card::new(Rank::Two, Suit::Hearts))
            .unwrap();
        trick
            .play(PlayerPosition::East, Card::new(Rank::Three, Suit::Hearts))
            .unwrap();
        assert_eq!(trick.winner(), Some(PlayerPosition::South));
        let hands = [
            Hand::with_cards(Vec::<Card>::new()),
            Hand::with_cards(Vec::<Card>::new()),
            Hand::with_cards(Vec::<Card>::new()),
            Hand::with_cards(Vec::<Card>::new()),
        ];
        let round = RoundState::from_hands_with_state(
            hands,
            PlayerPosition::South,
            PassingDirection::Hold,
            RoundPhase::Playing,
            Trick::new(PlayerPosition::South),
            vec![trick],
            true,
        );

        let mut match_state = MatchState::with_seed(PlayerPosition::South, 0);
        *match_state.round_mut() = round;

        let computer = RewardComputer::new(StepRewardMode::PerTrick);
        let prev_penalties = [0, 0, 0, 0];
        let reward =
            computer.compute_step_reward(&match_state, PlayerPosition::South, 1, 0, prev_penalties);
        assert!(reward < 0.0);
    }
}
