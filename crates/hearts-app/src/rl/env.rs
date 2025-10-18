use crate::bot::{BotFeatures, UnseenTracker};
use crate::policy::PolicyContext;
use crate::rl::observation::{Observation, ObservationBuilder};
use hearts_core::belief::Belief;
use hearts_core::game::match_state::MatchState;
use hearts_core::model::card::Card;
use hearts_core::model::passing::PassingDirection;
use hearts_core::model::player::PlayerPosition;
use hearts_core::model::round::RoundPhase;

/// RL environment configuration
#[derive(Debug, Clone, Copy)]
#[allow(dead_code)]
pub struct EnvConfig {
    pub reward_mode: RewardMode,
}

impl Default for EnvConfig {
    fn default() -> Self {
        Self {
            reward_mode: RewardMode::Relative,
        }
    }
}

/// Reward computation mode
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[allow(dead_code)]
pub enum RewardMode {
    /// Relative to average opponent score (recommended)
    Relative,
    /// Rank-based: 1st=+3, 2nd=+1, 3rd=-1, 4th=-3
    Rank,
}

/// RL environment wrapping Hearts game state
#[allow(dead_code)]
pub struct HeartsEnv {
    match_state: MatchState,
    pub(crate) current_seat: PlayerPosition, // pub(crate) for testing
    step_count: u32,
    config: EnvConfig,
    obs_builder: ObservationBuilder,
    tracker: UnseenTracker,
    bot_features: BotFeatures,
}

/// Single step result
#[derive(Debug, Clone)]
#[allow(dead_code)]
pub struct Step {
    pub obs: Observation,
    pub reward: f32,
    pub done: bool,
    pub info: StepInfo,
}

/// Additional step information
#[derive(Debug, Clone)]
#[allow(dead_code)]
pub struct StepInfo {
    pub phase: PhaseInfo,
    pub round_complete: bool,
    pub final_points: Option<[u32; 4]>,
}

/// Current phase information
#[derive(Debug, Clone)]
#[allow(dead_code)]
pub enum PhaseInfo {
    Passing {
        direction: PassingDirection,
        submitted: usize,
    },
    Playing {
        trick_leader: PlayerPosition,
        cards_in_trick: usize,
    },
}

impl HeartsEnv {
    /// Create new environment with seed
    #[allow(dead_code)]
    pub fn new(seed: u64, config: EnvConfig) -> Self {
        let match_state = MatchState::with_seed(PlayerPosition::South, seed);
        let mut tracker = UnseenTracker::new();
        tracker.reset_for_round(match_state.round());
        let bot_features = BotFeatures::from_env();

        Self {
            match_state,
            current_seat: PlayerPosition::South,
            step_count: 0,
            config,
            obs_builder: ObservationBuilder::new(),
            tracker,
            bot_features,
        }
    }

    /// Reset environment for new round
    #[allow(dead_code)]
    pub fn reset(&mut self) -> Observation {
        self.step_count = 0;
        self.current_seat = PlayerPosition::South;
        self.tracker.reset_for_round(self.match_state.round());
        self.build_observation()
    }

    /// Get current phase information
    #[allow(dead_code)]
    pub fn phase(&self) -> PhaseInfo {
        match self.match_state.round().phase() {
            RoundPhase::Passing(state) => {
                let submitted = if state.is_complete() { 4 } else { 0 };
                PhaseInfo::Passing {
                    direction: self.match_state.passing_direction(),
                    submitted,
                }
            }
            RoundPhase::Playing => {
                let trick = self.match_state.round().current_trick();
                PhaseInfo::Playing {
                    trick_leader: trick.leader(),
                    cards_in_trick: trick.plays().len(),
                }
            }
        }
    }

    /// Get legal moves for current seat
    #[allow(dead_code)]
    pub fn legal_moves(&self) -> Vec<Card> {
        let round = self.match_state.round();
        let hand = round.hand(self.current_seat);

        hand.iter()
            .copied()
            .filter(|&card| {
                let mut probe = round.clone();
                probe.play_card(self.current_seat, card).is_ok()
            })
            .collect()
    }

    /// Step with passing action
    #[allow(dead_code)]
    pub fn step_pass(&mut self, cards: [Card; 3]) -> Result<Step, String> {
        let round = self.match_state.round_mut();

        // Submit pass for current seat
        round
            .submit_pass(self.current_seat, cards)
            .map_err(|e| format!("Invalid pass: {:?}", e))?;

        // Track passed cards
        self.tracker.note_pass_selection(self.current_seat, &cards);

        // Check if all players have submitted
        if let RoundPhase::Passing(state) = round.phase() {
            if state.is_complete() {
                // All passes submitted, resolve them
                round
                    .resolve_passes()
                    .map_err(|e| format!("Pass resolution failed: {:?}", e))?;

                // Phase transitions to Playing
                self.current_seat = round.current_trick().leader();
                self.step_count += 1;

                Ok(Step {
                    obs: self.build_observation(),
                    reward: 0.0,
                    done: false,
                    info: StepInfo {
                        phase: self.phase(),
                        round_complete: false,
                        final_points: None,
                    },
                })
            } else {
                // More passes needed
                self.current_seat = self.current_seat.next();
                self.step_count += 1;

                Ok(Step {
                    obs: self.build_observation(),
                    reward: 0.0,
                    done: false,
                    info: StepInfo {
                        phase: self.phase(),
                        round_complete: false,
                        final_points: None,
                    },
                })
            }
        } else {
            Err("Not in passing phase".to_string())
        }
    }

    /// Step with playing action
    #[allow(dead_code)]
    pub fn step_play(&mut self, card: Card) -> Result<Step, String> {
        let actor = self.current_seat;
        let round = self.match_state.round_mut();

        // Play the card
        let outcome = round
            .play_card(actor, card)
            .map_err(|e| format!("Invalid play: {:?}", e))?;

        self.tracker.note_card_played(actor, card);
        self.step_count += 1;

        // Determine next seat and check if round is complete
        use hearts_core::model::round::PlayOutcome;

        match outcome {
            PlayOutcome::Played => {
                // Continue in same trick
                self.current_seat = actor.next();
                Ok(Step {
                    obs: self.build_observation(),
                    reward: 0.0,
                    done: false,
                    info: StepInfo {
                        phase: self.phase(),
                        round_complete: false,
                        final_points: None,
                    },
                })
            }
            PlayOutcome::TrickCompleted { winner, .. } => {
                // Trick completed, winner leads next
                self.current_seat = winner;

                // Check if round is complete (all 13 tricks played)
                if round.trick_history().len() == 13 {
                    // Round complete, compute rewards
                    let final_points = self.compute_round_points();
                    let reward = self.compute_reward_for(actor, &final_points);

                    Ok(Step {
                        obs: self.build_observation(),
                        reward,
                        done: true,
                        info: StepInfo {
                            phase: self.phase(),
                            round_complete: true,
                            final_points: Some(final_points),
                        },
                    })
                } else {
                    Ok(Step {
                        obs: self.build_observation(),
                        reward: 0.0,
                        done: false,
                        info: StepInfo {
                            phase: self.phase(),
                            round_complete: false,
                            final_points: None,
                        },
                    })
                }
            }
        }
    }

    fn build_observation(&self) -> Observation {
        let round = self.match_state.round();
        let hand = round.hand(self.current_seat);
        let mut belief_holder: Option<Belief> = None;
        let belief_ref = if self.bot_features.belief_enabled() {
            belief_holder = Some(Belief::from_state(round, self.current_seat));
            belief_holder.as_ref()
        } else {
            None
        };

        let ctx = PolicyContext {
            seat: self.current_seat,
            hand,
            round,
            scores: self.match_state.scores(),
            passing_direction: self.match_state.passing_direction(),
            tracker: &self.tracker,
            belief: belief_ref,
            features: self.bot_features,
        };

        self.obs_builder.build(&ctx)
    }

    fn compute_round_points(&self) -> [u32; 4] {
        let round = self.match_state.round();
        let penalties = round.penalty_totals();

        [
            penalties[0] as u32,
            penalties[1] as u32,
            penalties[2] as u32,
            penalties[3] as u32,
        ]
    }

    fn compute_reward_for(&self, seat: PlayerPosition, final_points: &[u32; 4]) -> f32 {
        // Detect successful moon shooting: one player has 26, others have 0
        let moon_shooter = final_points.iter().position(|&p| p == 26);

        if let Some(shooter_idx) = moon_shooter {
            // Verify it's a real moon (others have 0)
            let others_total: u32 = final_points
                .iter()
                .enumerate()
                .filter(|(i, _)| *i != shooter_idx)
                .map(|(_, &p)| p)
                .sum();

            if others_total == 0 {
                // Successful moon! Shooter gets huge reward, victims get penalty
                let mut rewards = [-26.0f32; 4];
                rewards[shooter_idx] = 78.0; // 3×26 advantage
                return rewards[seat.index()];
            }
        }

        // Normal scoring: relative rewards
        let my_points = final_points[seat.index()] as f32;

        match self.config.reward_mode {
            RewardMode::Relative => {
                let opponent_avg: f32 = final_points
                    .iter()
                    .enumerate()
                    .filter(|(i, _)| *i != seat.index())
                    .map(|(_, &p)| p as f32)
                    .sum::<f32>()
                    / 3.0;

                opponent_avg - my_points // Positive when winning
            }

            RewardMode::Rank => {
                let rank = self.compute_rank(final_points, seat);
                match rank {
                    1 => 3.0,
                    2 => 1.0,
                    3 => -1.0,
                    4 => -3.0,
                    _ => 0.0,
                }
            }
        }
    }

    fn compute_rank(&self, points: &[u32; 4], seat: PlayerPosition) -> usize {
        let my_points = points[seat.index()];
        let better_count = points.iter().filter(|&&p| p < my_points).count();
        better_count + 1 // Rank is 1-indexed
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn env_creates_with_seed() {
        let env = HeartsEnv::new(42, EnvConfig::default());
        let phase = env.phase();
        match phase {
            PhaseInfo::Passing { submitted, .. } => {
                assert_eq!(submitted, 0);
            }
            PhaseInfo::Playing { .. } => panic!("Should start in Passing phase"),
        }
    }

    #[test]
    fn env_legal_moves_non_empty() {
        let mut env = HeartsEnv::new(42, EnvConfig::default());

        // Complete passing phase to get to playing phase
        match env.phase() {
            PhaseInfo::Passing { .. } => {
                for _ in 0..4 {
                    let hand = env.match_state.round().hand(env.current_seat);
                    if hand.len() >= 3 {
                        let cards = [hand.cards()[0], hand.cards()[1], hand.cards()[2]];
                        let _ = env.step_pass(cards);
                    }
                }
            }
            PhaseInfo::Playing { .. } => {}
        }

        let legal = env.legal_moves();
        assert!(!legal.is_empty()); // Should have legal moves in Playing phase
    }

    #[test]
    fn moon_reward_detection() {
        let config = EnvConfig {
            reward_mode: RewardMode::Relative,
        };
        let mut env = HeartsEnv::new(42, config);

        // Set current_seat explicitly for the test
        env.current_seat = PlayerPosition::South;

        // Simulate moon shoot: South (index 2) gets 26, others get 0
        let points = [0, 0, 26, 0]; // points[South.index()] = 26
        let reward = env.compute_reward_for(PlayerPosition::South, &points);

        // South (current_seat) should get huge positive reward
        assert_eq!(reward, 78.0);
    }

    #[test]
    fn normal_reward_computation() {
        let config = EnvConfig {
            reward_mode: RewardMode::Relative,
        };
        let mut env = HeartsEnv::new(42, config);

        // Set current_seat explicitly
        env.current_seat = PlayerPosition::South;

        // Normal scoring: North=8, East=3, South=5, West=10
        let points = [8, 3, 5, 10]; // South is at index 2
        let reward = env.compute_reward_for(PlayerPosition::South, &points);

        // Opponent average = (8+3+10)/3 = 7.0
        // Reward = 7.0 - 5.0 = 2.0
        assert_eq!(reward, 2.0);
    }

    #[test]
    fn rank_based_reward() {
        let config = EnvConfig {
            reward_mode: RewardMode::Rank,
        };
        let mut env = HeartsEnv::new(42, config);

        // Set current_seat explicitly
        env.current_seat = PlayerPosition::South;

        // North=8, East=5, South=3 (1st), West=10
        let points = [8, 5, 3, 10]; // South at index 2
        let reward = env.compute_reward_for(PlayerPosition::South, &points);
        assert_eq!(reward, 3.0); // 1st place

        // North=5, East=8, South=10 (4th), West=3
        let points = [5, 8, 10, 3]; // South at index 2
        let reward = env.compute_reward_for(PlayerPosition::South, &points);
        assert_eq!(reward, -3.0); // 4th place
    }

    #[test]
    fn terminal_reward_assigned_to_actor() {
        use hearts_core::model::card::Card;
        use hearts_core::model::hand::Hand;
        use hearts_core::model::passing::PassingDirection;
        use hearts_core::model::rank::Rank;
        use hearts_core::model::round::{RoundPhase, RoundState};
        use hearts_core::model::suit::Suit;
        use hearts_core::model::trick::Trick;

        let config = EnvConfig {
            reward_mode: RewardMode::Rank,
        };
        let mut env = HeartsEnv::new(0, config);

        let mut history = Vec::with_capacity(12);
        for _ in 0..12 {
            let mut trick = Trick::new(PlayerPosition::North);
            trick
                .play(PlayerPosition::North, Card::new(Rank::Two, Suit::Clubs))
                .unwrap();
            trick
                .play(PlayerPosition::East, Card::new(Rank::Three, Suit::Clubs))
                .unwrap();
            trick
                .play(PlayerPosition::South, Card::new(Rank::Four, Suit::Clubs))
                .unwrap();
            trick
                .play(PlayerPosition::West, Card::new(Rank::Five, Suit::Clubs))
                .unwrap();
            history.push(trick);
        }

        let mut current_trick = Trick::new(PlayerPosition::South);
        current_trick
            .play(PlayerPosition::South, Card::new(Rank::King, Suit::Hearts))
            .unwrap();
        current_trick
            .play(PlayerPosition::West, Card::new(Rank::Queen, Suit::Spades))
            .unwrap();
        current_trick
            .play(PlayerPosition::North, Card::new(Rank::Ace, Suit::Hearts))
            .unwrap();

        let hands = [
            Hand::with_cards(Vec::<Card>::new()),
            Hand::with_cards(vec![Card::new(Rank::Two, Suit::Hearts)]),
            Hand::with_cards(Vec::<Card>::new()),
            Hand::with_cards(Vec::<Card>::new()),
        ];

        let round = RoundState::from_hands_with_state(
            hands,
            PlayerPosition::South,
            PassingDirection::Hold,
            RoundPhase::Playing,
            current_trick,
            history,
            true,
        );

        env.match_state = MatchState::with_seed(PlayerPosition::South, 0);
        *env.match_state.round_mut() = round;
        env.current_seat = PlayerPosition::East;
        env.tracker.reset_for_round(env.match_state.round());

        let step = env
            .step_play(Card::new(Rank::Two, Suit::Hearts))
            .expect("final card plays successfully");

        assert!(step.done);
        assert_eq!(step.reward, 3.0);
        let final_points = step
            .info
            .final_points
            .expect("final points available at terminal state");
        assert_eq!(final_points, [16, 0, 0, 0]);
        // North (index 0) captured the trick and absorbed penalties; actor East (index 1) still
        // receives the positive reward for finishing with the lowest score.
    }

    #[test]
    fn property_determinism() {
        // Same seed should produce identical sequences
        let env1 = HeartsEnv::new(123, EnvConfig::default());
        let env2 = HeartsEnv::new(123, EnvConfig::default());

        let legal1 = env1.legal_moves();
        let legal2 = env2.legal_moves();

        assert_eq!(legal1, legal2);
    }

    #[test]
    fn property_total_points_is_26() {
        // Simulate a full round and verify total points = 26
        let config = EnvConfig::default();
        let mut env = HeartsEnv::new(999, config); // Different seed to get Hold direction

        // Check if we're in passing phase
        match env.phase() {
            PhaseInfo::Passing { .. } => {
                // Complete passes
                for _ in 0..4 {
                    let legal = env.legal_moves();
                    if legal.len() >= 3 {
                        let cards = [legal[0], legal[1], legal[2]];
                        let _ = env.step_pass(cards);
                    }
                }
            }
            PhaseInfo::Playing { .. } => {}
        }

        // Play through the round
        let mut step_result = None;
        for _ in 0..1000 {
            // Safety limit
            let legal = env.legal_moves();
            if legal.is_empty() {
                break;
            }

            let card = legal[0];
            match env.step_play(card) {
                Ok(step) => {
                    if step.done {
                        step_result = Some(step);
                        break;
                    }
                }
                Err(_) => break,
            }
        }

        if let Some(step) = step_result {
            if let Some(points) = step.info.final_points {
                let total: u32 = points.iter().sum();
                assert_eq!(total, 26, "Total points should be 26");
            }
        }
    }

    #[test]
    fn property_legal_moves_always_valid() {
        let mut env = HeartsEnv::new(777, EnvConfig::default());

        // Skip passing
        match env.phase() {
            PhaseInfo::Passing { .. } => {
                for _ in 0..4 {
                    let legal = env.legal_moves();
                    if legal.len() >= 3 {
                        let cards = [legal[0], legal[1], legal[2]];
                        let _ = env.step_pass(cards);
                    }
                }
            }
            PhaseInfo::Playing { .. } => {}
        }

        // Verify every legal move actually works
        for _ in 0..20 {
            let legal = env.legal_moves();
            if legal.is_empty() {
                break;
            }

            // Try to play the first legal move
            let card = legal[0];
            let result = env.step_play(card);
            assert!(
                result.is_ok(),
                "Legal move should always succeed: {:?}",
                card
            );

            if result.unwrap().done {
                break;
            }
        }
    }

    #[test]
    fn fuzz_random_games_no_panic() {
        // Fuzz test: play 100 random games without panicking

        for seed in 0..100 {
            let mut env = HeartsEnv::new(seed, EnvConfig::default());

            // Handle passing phase
            let mut passing_done = false;
            while !passing_done {
                match env.phase() {
                    PhaseInfo::Passing { submitted, .. } => {
                        if submitted == 4 {
                            passing_done = true;
                        } else {
                            let legal = env.legal_moves();
                            if legal.len() >= 3 {
                                let cards = [legal[0], legal[1], legal[2]];
                                match env.step_pass(cards) {
                                    Ok(step) => {
                                        if matches!(step.info.phase, PhaseInfo::Playing { .. }) {
                                            passing_done = true;
                                        }
                                    }
                                    Err(_) => break,
                                }
                            } else {
                                break;
                            }
                        }
                    }
                    PhaseInfo::Playing { .. } => {
                        passing_done = true;
                    }
                }
            }

            // Play through the round
            for _ in 0..1000 {
                let legal = env.legal_moves();
                if legal.is_empty() {
                    break;
                }

                let card = legal[0];
                match env.step_play(card) {
                    Ok(step) => {
                        if step.done {
                            // Round complete, verify total points
                            if let Some(points) = step.info.final_points {
                                let total: u32 = points.iter().sum();
                                assert_eq!(
                                    total, 26,
                                    "Seed {}: Total points should be 26, got {}",
                                    seed, total
                                );
                            }
                            break;
                        }
                    }
                    Err(_) => break,
                }
            }
        }
    }
}
