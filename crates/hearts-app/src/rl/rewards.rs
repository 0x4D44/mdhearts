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
    ) -> f32 {
        match self.mode {
            StepRewardMode::Terminal => 0.0,
            StepRewardMode::PerTrick => {
                self.per_trick_reward(match_state, seat, prev_tricks_completed)
            }
            StepRewardMode::Shaped => self.shaped_reward(match_state, seat, prev_hand_size),
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
    ) -> f32 {
        let round = match_state.round();
        let current_tricks = round.tricks_completed();

        // Just completed a trick?
        if current_tricks > prev_tricks_completed {
            let penalty_totals = round.penalty_totals();
            let current_points = penalty_totals[seat.index()] as f32;

            // For simplicity, we penalize based on total points so far
            // A more sophisticated approach would track delta points
            -current_points / 26.0
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
    ) -> f32 {
        let round = match_state.round();
        let current_hand_size = round.hand(seat).len();

        // Did we just play a card?
        if current_hand_size < prev_hand_size {
            let trick = round.current_trick();

            // Trick just completed?
            if trick.is_complete() {
                if let Some(winner) = trick.winner() {
                    if winner == seat {
                        // We won the trick - penalize for points taken
                        let points = trick.penalty_total() as f32;
                        return -points / 26.0;
                    }
                }
            }
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

        let reward = computer.compute_step_reward(&match_state, PlayerPosition::South, 13, 0);

        assert_eq!(reward, 0.0);
    }

    #[test]
    fn per_trick_mode_gives_zero_when_no_trick_completed() {
        let computer = RewardComputer::new(StepRewardMode::PerTrick);
        let match_state = MatchState::with_seed(PlayerPosition::South, 42);

        // Same trick count, so no reward
        let reward = computer.compute_step_reward(&match_state, PlayerPosition::South, 13, 0);

        assert_eq!(reward, 0.0);
    }

    #[test]
    fn shaped_mode_gives_zero_when_hand_unchanged() {
        let computer = RewardComputer::new(StepRewardMode::Shaped);
        let match_state = MatchState::with_seed(PlayerPosition::South, 42);

        let hand_size = match_state.round().hand(PlayerPosition::South).len();

        // Hand size unchanged, so no reward
        let reward =
            computer.compute_step_reward(&match_state, PlayerPosition::South, hand_size, 0);

        assert_eq!(reward, 0.0);
    }
}
