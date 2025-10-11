# RL Training Pipeline - Implementation Plan

## Overview

This document provides a detailed, step-by-step implementation plan for building the complete PPO-based RL training pipeline for Hearts AI. The plan is organized into 4 sequential phases, each with concrete tasks, deliverables, and acceptance criteria.

**Total Estimated Effort:** 20-28 hours
**Timeline:** 3-4 weeks (part-time) or 1 week (full-time)
**Prerequisites:** Rust, Python, PyTorch installed

---

## Phase 1: Enhanced Data Collection (Rust)

**Goal:** Extend mdhearts to collect PPO-ready experiences with value predictions and log probabilities.

**Estimated Effort:** 6-8 hours

### Task 1.1: Create RLExperience Struct
**File:** `crates/hearts-app/src/rl/experience.rs`
**Effort:** 1 hour

**Implementation:**
```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RLExperience {
    // Existing fields (keep as-is)
    pub observation: Vec<f32>,
    pub action: u8,
    pub reward: f32,
    pub done: bool,
    pub game_id: usize,
    pub step_id: usize,
    pub seat: u8,

    // NEW: PPO-specific fields
    pub value: f32,        // Critic's value estimate V(s)
    pub log_prob: f32,     // Log probability log π(a|s)
}

pub struct RLExperienceCollector {
    writer: BufWriter<File>,
    count: usize,
}

impl RLExperienceCollector {
    pub fn new<P: AsRef<Path>>(path: P) -> Result<Self, String> { /* ... */ }
    pub fn record(&mut self, exp: RLExperience) -> Result<(), String> { /* ... */ }
    pub fn flush(&mut self) -> Result<(), String> { /* ... */ }
    pub fn count(&self) -> usize { /* ... */ }
}
```

**Steps:**
1. Add `value` and `log_prob` fields to `RLExperience`
2. Update serialization tests
3. Create `RLExperienceCollector` (similar to existing `ExperienceCollector`)
4. Add JSONL write/flush methods

**Acceptance Criteria:**
- [ ] `RLExperience` struct compiles
- [ ] Serialization roundtrip test passes
- [ ] JSONL format validated with sample data

---

### Task 1.2: Extend EmbeddedPolicy with Critic
**File:** `crates/hearts-app/src/policy/embedded.rs`
**Effort:** 2-3 hours

**Implementation:**
```rust
impl EmbeddedPolicy {
    /// Forward pass returning action, value, and log probability
    pub fn forward_with_critic(
        &self,
        ctx: &PolicyContext,
    ) -> (Card, f32, f32) {
        let obs = self.obs_builder.build(ctx);
        let obs_array = obs.as_array();

        // Forward pass through network
        let (logits, value) = self.forward_actor_critic(obs_array);

        // Mask illegal actions
        let legal_mask = self.compute_legal_mask(ctx);
        let masked_logits = self.apply_mask(&logits, &legal_mask);

        // Sample action
        let probs = softmax(&masked_logits);
        let action_idx = self.sample_categorical(&probs);
        let log_prob = probs[action_idx].ln();

        // Convert to card
        let card = Card::from_id(action_idx as u8).unwrap();

        (card, value, log_prob)
    }

    fn forward_actor_critic(&self, input: &[f32; 270]) -> ([f32; 52], f32) {
        // Layer 1: 270 -> 256
        let mut hidden1 = [0.0f32; 256];
        self.matmul_add_bias(
            input,
            &self.layer1_weights(),
            &self.layer1_biases(),
            &mut hidden1,
        );
        self.relu(&mut hidden1);

        // Layer 2: 256 -> 128
        let mut hidden2 = [0.0f32; 128];
        self.matmul_add_bias(
            &hidden1,
            &self.layer2_weights(),
            &self.layer2_biases(),
            &mut hidden2,
        );
        self.relu(&mut hidden2);

        // Actor head: 128 -> 52 (logits)
        let mut logits = [0.0f32; 52];
        self.matmul_add_bias(
            &hidden2,
            &self.layer3_weights(),
            &self.layer3_biases(),
            &mut logits,
        );

        // Critic head: 128 -> 1 (value)
        // For now, use simple heuristic until we have trained critic
        let value = self.estimate_value_heuristic(input);

        (logits, value)
    }

    fn estimate_value_heuristic(&self, _obs: &[f32; 270]) -> f32 {
        // Placeholder: return 0 until we have trained critic weights
        // In practice, this will be replaced by learned critic head
        0.0
    }

    fn sample_categorical(&self, probs: &[f32]) -> usize {
        use rand::Rng;
        let mut rng = rand::thread_rng();
        let sample: f32 = rng.gen();

        let mut cumsum = 0.0;
        for (i, &p) in probs.iter().enumerate() {
            cumsum += p;
            if sample < cumsum {
                return i;
            }
        }
        probs.len() - 1
    }
}
```

**Steps:**
1. Add `forward_with_critic()` method
2. Implement softmax and categorical sampling
3. Add critic head placeholder (returns 0.0 initially)
4. Update tests to verify stochastic sampling

**Acceptance Criteria:**
- [ ] `forward_with_critic()` returns (Card, f32, f32)
- [ ] Sampling respects legal move masking
- [ ] Log probabilities are valid (finite, non-NaN)
- [ ] Tests pass

---

### Task 1.3: Implement Step-Wise Rewards
**File:** `crates/hearts-app/src/rl/rewards.rs` (new file)
**Effort:** 2 hours

**Implementation:**
```rust
use hearts_core::game::match_state::MatchState;
use hearts_core::model::player::PlayerPosition;
use hearts_core::model::suit::Suit;
use hearts_core::model::card::Card;

pub enum RewardMode {
    Terminal,    // Only at episode end
    PerTrick,    // After each trick
    Shaped,      // Shaped intermediate rewards
}

pub struct RewardComputer {
    mode: RewardMode,
}

impl RewardComputer {
    pub fn new(mode: RewardMode) -> Self {
        Self { mode }
    }

    pub fn compute_step_reward(
        &self,
        match_state: &MatchState,
        seat: PlayerPosition,
        prev_hand_size: usize,
        prev_tricks_completed: usize,
    ) -> f32 {
        match self.mode {
            RewardMode::Terminal => 0.0,
            RewardMode::PerTrick => self.per_trick_reward(match_state, seat, prev_tricks_completed),
            RewardMode::Shaped => self.shaped_reward(match_state, seat, prev_hand_size),
        }
    }

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
            let points_before = if prev_tricks_completed > 0 {
                // Would need to track historical penalties
                // For now, approximate
                0
            } else {
                0
            };

            let new_points = penalty_totals[seat.index()] - points_before;
            return -(new_points as f32) / 26.0;
        }

        0.0
    }

    fn shaped_reward(
        &self,
        match_state: &MatchState,
        seat: PlayerPosition,
        prev_hand_size: usize,
    ) -> f32 {
        let round = match_state.round();
        let current_hand_size = round.hand(seat).card_count();

        // Played a card this step?
        if current_hand_size < prev_hand_size {
            let trick = round.current_trick();

            // Trick just completed?
            if trick.is_complete() {
                if let Some(winner) = trick.winner() {
                    if winner == seat {
                        let points = trick.penalty_total();
                        return -(points as f32) / 26.0;
                    }
                }
            }
        }

        0.0
    }

    pub fn compute_terminal_reward(
        &self,
        match_state: &MatchState,
        seat: PlayerPosition,
    ) -> f32 {
        let penalty_totals = match_state.round().penalty_totals();
        let our_points = penalty_totals[seat.index()] as f32;

        // Normalize to [-1, 0]
        -our_points / 26.0
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn terminal_reward_zero_points_is_zero() {
        // Test that perfect game (0 points) gives 0 reward
    }

    #[test]
    fn terminal_reward_all_points_is_negative_one() {
        // Test that taking all 26 points gives -1 reward
    }
}
```

**Steps:**
1. Create `rewards.rs` module
2. Implement `RewardMode` enum
3. Add `RewardComputer` with three reward strategies
4. Write tests for reward computation
5. Export from `rl/mod.rs`

**Acceptance Criteria:**
- [ ] `RewardComputer::compute_step_reward()` works
- [ ] Terminal rewards normalized to [-1, 0]
- [ ] Per-trick rewards only fire on trick completion
- [ ] Tests pass

---

### Task 1.4: Implement Self-Play CLI Mode
**File:** `crates/hearts-app/src/cli.rs`
**Effort:** 2-3 hours

**Implementation:**
```rust
pub fn run_self_play_eval(
    num_games: usize,
    policy_path: Option<PathBuf>,
    collect_rl_path: Option<PathBuf>,
    reward_mode: RewardMode,
) -> Result<(), CliError> {
    use crate::rl::{RLExperience, RLExperienceCollector, RewardComputer};

    println!("Running {} games in self-play mode", num_games);

    // Load policy for all 4 players
    let policy: Box<dyn Policy> = if let Some(path) = policy_path {
        Box::new(EmbeddedPolicy::from_file(path).map_err(|e| {
            CliError::Io(std::io::Error::new(std::io::ErrorKind::InvalidData, e))
        })?)
    } else {
        Box::new(EmbeddedPolicy::new())
    };

    // Create collector if requested
    let mut collector = if let Some(ref path) = collect_rl_path {
        Some(RLExperienceCollector::new(path).map_err(|e| {
            CliError::Io(std::io::Error::new(std::io::ErrorKind::Other, e))
        })?)
    } else {
        None
    };

    let obs_builder = if collector.is_some() {
        Some(ObservationBuilder::new())
    } else {
        None
    };

    let reward_computer = RewardComputer::new(reward_mode);

    for game_id in 0..num_games {
        let seed = game_id as u64;
        let mut match_state = MatchState::with_seed(PlayerPosition::South, seed);
        let mut tracker = UnseenTracker::new();
        tracker.reset_for_round(match_state.round());

        // Track experiences for all 4 seats
        let mut seat_experiences: [Vec<(usize, Card, Vec<f32>, f32, f32)>; 4] = Default::default();
        let mut step_ids = [0usize; 4];

        // Play complete round
        loop {
            // Handle passing phase
            if matches!(match_state.round().phase(), RoundPhase::Passing(_)) {
                for seat in PlayerPosition::LOOP {
                    let hand = match_state.round().hand(seat);
                    let scores = match_state.scores();
                    let passing_dir = match_state.passing_direction();

                    let ctx = PolicyContext {
                        hand,
                        round: match_state.round(),
                        scores,
                        seat,
                        tracker: &tracker,
                        passing_direction: passing_dir,
                    };

                    let pass_cards = policy.choose_pass(&ctx);
                    let _ = match_state.round_mut().submit_pass(seat, pass_cards);
                }
                let _ = match_state.round_mut().resolve_passes();
                continue;
            }

            // Handle playing phase
            if matches!(match_state.round().phase(), RoundPhase::Playing) {
                let current_player = {
                    let trick = match_state.round().current_trick();
                    trick
                        .plays()
                        .last()
                        .map(|p| p.position.next())
                        .unwrap_or(trick.leader())
                };

                let hand = match_state.round().hand(current_player);
                let scores = match_state.scores();
                let passing_dir = match_state.passing_direction();

                let ctx = PolicyContext {
                    hand,
                    round: match_state.round(),
                    scores,
                    seat: current_player,
                    tracker: &tracker,
                    passing_direction: passing_dir,
                };

                // Track state before action
                let prev_hand_size = hand.card_count();
                let prev_tricks = match_state.round().tricks_completed();

                // Get action with critic values
                let (card, value, log_prob) = if collector.is_some() {
                    policy.forward_with_critic(&ctx)
                } else {
                    let card = policy.choose_play(&ctx);
                    (card, 0.0, 0.0)
                };

                // Execute action
                tracker.note_card_played(current_player, card);
                match match_state.round_mut().play_card(current_player, card) {
                    Ok(_) => {}
                    Err(e) => {
                        eprintln!("Error playing card: {:?}", e);
                        break;
                    }
                }

                // Compute step reward
                let step_reward = reward_computer.compute_step_reward(
                    &match_state,
                    current_player,
                    prev_hand_size,
                    prev_tricks,
                );

                // Store experience data (will add terminal reward later)
                if let Some(ref builder) = obs_builder {
                    let obs = builder.build(&ctx);
                    let obs_vec = obs.as_array().to_vec();

                    let seat_idx = current_player.index();
                    seat_experiences[seat_idx].push((
                        step_ids[seat_idx],
                        card,
                        obs_vec,
                        value,
                        log_prob,
                    ));
                    step_ids[seat_idx] += 1;
                }

                // Check if round complete
                if match_state.round().tricks_completed() >= 13 {
                    break;
                }
            } else {
                break;
            }
        }

        // Compute terminal rewards and record all experiences
        if let Some(ref mut coll) = collector {
            let penalty_totals = match_state.round().penalty_totals();

            for seat in PlayerPosition::LOOP {
                let seat_idx = seat.index();
                let terminal_reward = reward_computer.compute_terminal_reward(&match_state, seat);

                for (i, (step_id, card, obs_vec, value, log_prob)) in
                    seat_experiences[seat_idx].iter().enumerate()
                {
                    let is_last = i == seat_experiences[seat_idx].len() - 1;

                    let exp = RLExperience {
                        observation: obs_vec.clone(),
                        action: card.to_id(),
                        reward: if is_last { terminal_reward } else { 0.0 },
                        done: is_last,
                        game_id,
                        step_id: *step_id,
                        seat: seat_idx as u8,
                        value: *value,
                        log_prob: *log_prob,
                    };

                    if let Err(e) = coll.record(exp) {
                        eprintln!("Warning: Failed to record experience: {}", e);
                    }
                }
            }
        }

        // Progress output
        if (game_id + 1) % 10 == 0 {
            println!("Completed {}/{} games", game_id + 1, num_games);
        }
    }

    if let Some(ref mut coll) = collector {
        coll.flush()?;
        println!("Collected {} experiences", coll.count());
    }

    Ok(())
}
```

**Steps:**
1. Add `--self-play` flag parsing in CLI
2. Add `--collect-rl <path>` flag
3. Add `--reward-mode <terminal|per-trick|shaped>` flag
4. Implement self-play game loop collecting from all 4 seats
5. Store experiences with value/log_prob from policy
6. Add terminal reward at episode end

**Acceptance Criteria:**
- [ ] `mdhearts eval 10 --self-play --collect-rl data.jsonl` works
- [ ] Collects experiences from all 4 seats (4x per game)
- [ ] JSONL output has all required fields
- [ ] Value and log_prob are non-zero
- [ ] Tests pass

---

### Task 1.5: Update Policy Trait
**File:** `crates/hearts-app/src/policy/mod.rs`
**Effort:** 30 minutes

**Implementation:**
```rust
pub trait Policy {
    fn choose_pass(&mut self, ctx: &PolicyContext) -> [Card; 3];
    fn choose_play(&mut self, ctx: &PolicyContext) -> Card;

    // NEW: Optional method for RL training
    fn forward_with_critic(&mut self, ctx: &PolicyContext) -> (Card, f32, f32) {
        // Default implementation: deterministic play with no critic
        let card = self.choose_play(ctx);
        (card, 0.0, 0.0)
    }
}
```

**Steps:**
1. Add `forward_with_critic()` default method to trait
2. Implement for `HeuristicPolicy` (returns 0.0 for value/log_prob)
3. Implement for `EmbeddedPolicy` (uses actual critic)

**Acceptance Criteria:**
- [ ] Trait compiles with new method
- [ ] All policy types implement the trait
- [ ] Tests pass

---

### Phase 1 Deliverables

**Code Files:**
- `crates/hearts-app/src/rl/experience.rs` - RLExperience + collector
- `crates/hearts-app/src/rl/rewards.rs` - Reward computation
- `crates/hearts-app/src/policy/embedded.rs` - Critic support
- `crates/hearts-app/src/policy/mod.rs` - Updated trait
- `crates/hearts-app/src/cli.rs` - Self-play mode

**Tests:**
- Unit tests for RLExperience serialization
- Unit tests for reward computation
- Integration test for self-play collection

**Validation Command:**
```bash
cargo test --workspace
cargo build --release
./target/release/mdhearts.exe eval 10 --self-play --collect-rl test_data.jsonl --reward-mode shaped
# Should create test_data.jsonl with ~2080 experiences
```

---

## Phase 2: Python PPO Core

**Goal:** Implement PPO training algorithm with actor-critic network.

**Estimated Effort:** 6-8 hours

### Task 2.1: Create Project Structure
**Effort:** 30 minutes

**Directory Structure:**
```
tools/
├── ppo/
│   ├── __init__.py
│   ├── config.py          # Configuration dataclasses
│   ├── network.py         # ActorCriticNetwork
│   ├── trainer.py         # PPOTrainer
│   ├── dataset.py         # RLDataset
│   ├── advantages.py      # GAE computation
│   └── utils.py           # Helper functions
├── train_ppo_pipeline.py  # Main orchestrator
├── evaluate_policy.py     # Evaluation harness
└── requirements.txt       # Python dependencies
```

**requirements.txt:**
```
torch>=2.0.0
numpy>=1.24.0
tensorboard>=2.12.0
pyyaml>=6.0
tqdm>=4.65.0
```

**Steps:**
1. Create directory structure
2. Add `__init__.py` files
3. Create `requirements.txt`
4. Add `.gitignore` for Python artifacts

**Acceptance Criteria:**
- [ ] Directory structure created
- [ ] `pip install -r requirements.txt` works
- [ ] Imports work: `from ppo import config`

---

### Task 2.2: Implement Configuration
**File:** `tools/ppo/config.py`
**Effort:** 30 minutes

**Implementation:**
```python
from dataclasses import dataclass, field
from typing import Literal
import yaml

@dataclass
class PPOConfig:
    # Learning
    learning_rate: float = 3e-4
    gamma: float = 0.99
    gae_lambda: float = 0.95
    clip_epsilon: float = 0.2
    value_coef: float = 0.5
    entropy_coef: float = 0.01
    max_grad_norm: float = 0.5

    # Training
    ppo_epochs: int = 4
    batch_size: int = 256
    normalize_advantages: bool = True

    # Network
    obs_dim: int = 270
    hidden_dim: int = 256
    action_dim: int = 52

    # Collection
    games_per_iteration: int = 100
    num_iterations: int = 100
    reward_mode: Literal["terminal", "per_trick", "shaped"] = "shaped"

    # Evaluation
    eval_games: int = 100
    eval_frequency: int = 1

    # Checkpointing
    checkpoint_dir: str = "checkpoints"
    save_frequency: int = 5

    # Logging
    log_dir: str = "logs"
    use_tensorboard: bool = True

    @classmethod
    def from_yaml(cls, path: str) -> "PPOConfig":
        with open(path, 'r') as f:
            data = yaml.safe_load(f)
        return cls(**data)

    def to_yaml(self, path: str):
        with open(path, 'w') as f:
            yaml.dump(self.__dict__, f, default_flow_style=False)
```

**Steps:**
1. Create `PPOConfig` dataclass
2. Add `from_yaml()` and `to_yaml()` methods
3. Create default config file `config/ppo_default.yaml`
4. Add validation

**Acceptance Criteria:**
- [ ] Config loads from YAML
- [ ] Config saves to YAML
- [ ] All parameters have sensible defaults

---

### Task 2.3: Implement Actor-Critic Network
**File:** `tools/ppo/network.py`
**Effort:** 2 hours

**Implementation:**
```python
import torch
import torch.nn as nn
import numpy as np
from typing import Tuple, Optional

class ActorCriticNetwork(nn.Module):
    """MLP policy with shared trunk and separate actor/critic heads."""

    def __init__(self, obs_dim: int = 270, hidden_dim: int = 256, action_dim: int = 52):
        super().__init__()

        # Shared feature extractor
        self.shared = nn.Sequential(
            nn.Linear(obs_dim, hidden_dim),
            nn.ReLU(),
            nn.Linear(hidden_dim, hidden_dim // 2),
            nn.ReLU(),
        )

        # Actor head (policy)
        self.actor = nn.Linear(hidden_dim // 2, action_dim)

        # Critic head (value function)
        self.critic = nn.Linear(hidden_dim // 2, 1)

        # Initialize weights with orthogonal
        self.apply(self._init_weights)

    def _init_weights(self, module):
        if isinstance(module, nn.Linear):
            nn.init.orthogonal_(module.weight, gain=np.sqrt(2))
            nn.init.constant_(module.bias, 0.0)

    def forward(
        self,
        obs: torch.Tensor,
        legal_mask: Optional[torch.Tensor] = None
    ) -> Tuple[torch.Tensor, torch.Tensor]:
        """
        Forward pass.

        Args:
            obs: [batch_size, 270] observations
            legal_mask: [batch_size, 52] boolean mask (True = legal)

        Returns:
            logits: [batch_size, 52] action logits
            values: [batch_size] state values
        """
        features = self.shared(obs)

        # Actor logits
        logits = self.actor(features)
        if legal_mask is not None:
            logits = logits.masked_fill(~legal_mask, -1e9)

        # Critic value
        values = self.critic(features).squeeze(-1)

        return logits, values

    def get_action_and_value(
        self,
        obs: torch.Tensor,
        legal_mask: Optional[torch.Tensor] = None,
        action: Optional[torch.Tensor] = None
    ) -> Tuple[torch.Tensor, torch.Tensor, torch.Tensor, torch.Tensor]:
        """
        Sample action and compute value (used during collection).

        Returns:
            action: [batch_size] sampled actions
            log_prob: [batch_size] log probabilities
            entropy: [batch_size] policy entropy
            value: [batch_size] state values
        """
        logits, value = self.forward(obs, legal_mask)
        probs = torch.distributions.Categorical(logits=logits)

        if action is None:
            action = probs.sample()

        return action, probs.log_prob(action), probs.entropy(), value


# Unit tests
if __name__ == '__main__':
    # Test forward pass
    net = ActorCriticNetwork()
    obs = torch.randn(32, 270)
    legal_mask = torch.ones(32, 52, dtype=torch.bool)

    logits, values = net(obs, legal_mask)
    assert logits.shape == (32, 52)
    assert values.shape == (32,)

    # Test action sampling
    action, log_prob, entropy, value = net.get_action_and_value(obs, legal_mask)
    assert action.shape == (32,)
    assert log_prob.shape == (32,)
    assert entropy.shape == (32,)
    assert value.shape == (32,)

    print("✓ Network tests passed")
```

**Steps:**
1. Implement `ActorCriticNetwork` class
2. Add orthogonal weight initialization
3. Implement `forward()` and `get_action_and_value()`
4. Add unit tests
5. Test gradient flow

**Acceptance Criteria:**
- [ ] Network forward pass works
- [ ] Legal masking works correctly
- [ ] Action sampling is stochastic
- [ ] Gradients flow through both heads
- [ ] Unit tests pass

---

### Task 2.4: Implement GAE Computation
**File:** `tools/ppo/advantages.py`
**Effort:** 1-2 hours

**Implementation:**
```python
import torch
import numpy as np
from typing import List, Dict, Tuple

def compute_gae(
    rewards: torch.Tensor,
    values: torch.Tensor,
    dones: torch.Tensor,
    next_values: torch.Tensor,
    gamma: float = 0.99,
    gae_lambda: float = 0.95,
) -> Tuple[torch.Tensor, torch.Tensor]:
    """
    Compute Generalized Advantage Estimation.

    Args:
        rewards: [T] rewards
        values: [T] value estimates V(s_t)
        dones: [T] episode terminals
        next_values: [T] next value estimates V(s_{t+1})
        gamma: discount factor
        gae_lambda: GAE smoothing parameter

    Returns:
        advantages: [T] advantage estimates
        returns: [T] value targets (A_t + V_t)
    """
    T = len(rewards)
    advantages = torch.zeros_like(rewards)
    gae = 0

    for t in reversed(range(T)):
        if dones[t]:
            next_value = 0
        else:
            next_value = next_values[t]

        # TD error: δ_t = r_t + γV(s_{t+1}) - V(s_t)
        delta = rewards[t] + gamma * next_value - values[t]

        # GAE: A_t = δ_t + (γλ)δ_{t+1} + (γλ)²δ_{t+2} + ...
        gae = delta + gamma * gae_lambda * (1 - dones[t]) * gae
        advantages[t] = gae

    returns = advantages + values
    return advantages, returns


def process_episodes(
    experiences: List[Dict],
    gamma: float = 0.99,
    gae_lambda: float = 0.95,
) -> List[Dict]:
    """
    Group experiences by episode and compute advantages.

    Args:
        experiences: List of experience dicts from JSONL
        gamma: discount factor
        gae_lambda: GAE parameter

    Returns:
        Processed experiences with 'advantage' and 'return' fields
    """
    from collections import defaultdict

    # Group by (game_id, seat)
    episodes = defaultdict(list)
    for exp in experiences:
        key = (exp['game_id'], exp['seat'])
        episodes[key].append(exp)

    processed = []

    for episode in episodes.values():
        # Sort by step_id
        episode = sorted(episode, key=lambda x: x['step_id'])

        # Extract tensors
        rewards = torch.tensor([e['reward'] for e in episode], dtype=torch.float32)
        values = torch.tensor([e['value'] for e in episode], dtype=torch.float32)
        dones = torch.tensor([float(e['done']) for e in episode], dtype=torch.float32)
        next_values = torch.cat([values[1:], torch.zeros(1)])

        # Compute GAE
        advantages, returns = compute_gae(rewards, values, dones, next_values, gamma, gae_lambda)

        # Augment experiences
        for i, exp in enumerate(episode):
            exp['advantage'] = advantages[i].item()
            exp['return'] = returns[i].item()
            processed.append(exp)

    return processed


# Unit tests
if __name__ == '__main__':
    # Test basic GAE
    rewards = torch.tensor([0.0, 0.0, 1.0])
    values = torch.tensor([0.5, 0.6, 0.0])
    dones = torch.tensor([0.0, 0.0, 1.0])
    next_values = torch.tensor([0.6, 0.0, 0.0])

    advantages, returns = compute_gae(rewards, values, dones, next_values, gamma=0.99, gae_lambda=0.95)

    assert advantages.shape == (3,)
    assert returns.shape == (3,)
    assert torch.isfinite(advantages).all()
    assert torch.isfinite(returns).all()

    print("✓ GAE tests passed")
    print(f"  Advantages: {advantages}")
    print(f"  Returns: {returns}")
```

**Steps:**
1. Implement `compute_gae()` function
2. Implement `process_episodes()` for batching
3. Add unit tests with known values
4. Validate against reference implementation

**Acceptance Criteria:**
- [ ] GAE computation is correct
- [ ] Returns = advantages + values
- [ ] Episode boundaries respected
- [ ] Unit tests pass

---

### Task 2.5: Implement PPO Trainer
**File:** `tools/ppo/trainer.py`
**Effort:** 3-4 hours

**Implementation:**
```python
import torch
import torch.nn as nn
import torch.optim as optim
import torch.nn.functional as F
from torch.distributions import Categorical
from typing import Dict, Optional
import numpy as np

from .network import ActorCriticNetwork
from .config import PPOConfig

class PPOTrainer:
    """Proximal Policy Optimization trainer."""

    def __init__(self, network: ActorCriticNetwork, config: PPOConfig):
        self.network = network
        self.config = config
        self.optimizer = optim.Adam(network.parameters(), lr=config.learning_rate)

        # Metrics tracking
        self.metrics = {
            'policy_loss': [],
            'value_loss': [],
            'entropy': [],
            'approx_kl': [],
            'clip_fraction': [],
        }

    def update(self, rollout_buffer: Dict[str, torch.Tensor]) -> Dict[str, float]:
        """
        Perform PPO update.

        Args:
            rollout_buffer: Dict with keys:
                - obs: [N, 270]
                - actions: [N]
                - old_log_probs: [N]
                - advantages: [N]
                - returns: [N]
                - old_values: [N]

        Returns:
            Metrics dict
        """
        # Normalize advantages
        advantages = rollout_buffer['advantages']
        if self.config.normalize_advantages:
            advantages = (advantages - advantages.mean()) / (advantages.std() + 1e-8)

        # Multiple epochs over same data
        for epoch in range(self.config.ppo_epochs):
            # Shuffle indices
            indices = torch.randperm(len(advantages))

            # Mini-batch updates
            for start in range(0, len(indices), self.config.batch_size):
                end = start + self.config.batch_size
                batch_idx = indices[start:end]

                # Extract batch
                b_obs = rollout_buffer['obs'][batch_idx]
                b_actions = rollout_buffer['actions'][batch_idx]
                b_old_log_probs = rollout_buffer['old_log_probs'][batch_idx]
                b_advantages = advantages[batch_idx]
                b_returns = rollout_buffer['returns'][batch_idx]
                b_old_values = rollout_buffer['old_values'][batch_idx]

                # Forward pass
                logits, values = self.network(b_obs)
                dist = Categorical(logits=logits)
                new_log_probs = dist.log_prob(b_actions)
                entropy = dist.entropy()

                # Policy loss (PPO clipped objective)
                ratio = torch.exp(new_log_probs - b_old_log_probs)
                surr1 = ratio * b_advantages
                surr2 = torch.clamp(
                    ratio,
                    1 - self.config.clip_epsilon,
                    1 + self.config.clip_epsilon
                ) * b_advantages
                policy_loss = -torch.min(surr1, surr2).mean()

                # Value loss (clipped)
                values_clipped = b_old_values + torch.clamp(
                    values - b_old_values,
                    -self.config.clip_epsilon,
                    self.config.clip_epsilon
                )
                value_loss_unclipped = F.mse_loss(values, b_returns)
                value_loss_clipped = F.mse_loss(values_clipped, b_returns)
                value_loss = torch.max(value_loss_unclipped, value_loss_clipped)

                # Total loss
                loss = (
                    policy_loss
                    + self.config.value_coef * value_loss
                    - self.config.entropy_coef * entropy.mean()
                )

                # Optimize
                self.optimizer.zero_grad()
                loss.backward()
                nn.utils.clip_grad_norm_(self.network.parameters(), self.config.max_grad_norm)
                self.optimizer.step()

                # Track metrics
                with torch.no_grad():
                    approx_kl = ((ratio - 1) - ratio.log()).mean()
                    clip_fraction = ((ratio - 1).abs() > self.config.clip_epsilon).float().mean()

                self.metrics['policy_loss'].append(policy_loss.item())
                self.metrics['value_loss'].append(value_loss.item())
                self.metrics['entropy'].append(entropy.mean().item())
                self.metrics['approx_kl'].append(approx_kl.item())
                self.metrics['clip_fraction'].append(clip_fraction.item())

        # Return average metrics
        return {k: np.mean(v[-10:]) for k, v in self.metrics.items()}

    def get_metrics(self) -> Dict[str, float]:
        """Get recent training metrics."""
        return {k: np.mean(v[-10:]) if v else 0.0 for k, v in self.metrics.items()}


# Unit tests
if __name__ == '__main__':
    from .config import PPOConfig

    config = PPOConfig()
    network = ActorCriticNetwork()
    trainer = PPOTrainer(network, config)

    # Create fake rollout
    N = 256
    rollout = {
        'obs': torch.randn(N, 270),
        'actions': torch.randint(0, 52, (N,)),
        'old_log_probs': torch.randn(N),
        'advantages': torch.randn(N),
        'returns': torch.randn(N),
        'old_values': torch.randn(N),
    }

    # Update
    metrics = trainer.update(rollout)

    assert 'policy_loss' in metrics
    assert 'value_loss' in metrics
    assert np.isfinite(metrics['policy_loss'])

    print("✓ PPO trainer tests passed")
    print(f"  Metrics: {metrics}")
```

**Steps:**
1. Implement `PPOTrainer` class
2. Add clipped policy objective
3. Add clipped value loss
4. Implement mini-batch training
5. Add metrics tracking
6. Write unit tests

**Acceptance Criteria:**
- [ ] PPO update runs without errors
- [ ] Losses decrease on dummy data
- [ ] Clip fraction is non-zero
- [ ] KL divergence is tracked
- [ ] Unit tests pass

---

### Task 2.6: Implement Dataset Loader
**File:** `tools/ppo/dataset.py`
**Effort:** 1 hour

**Implementation:**
```python
import json
import torch
from torch.utils.data import Dataset
from typing import List, Dict
import numpy as np

class RLDataset(Dataset):
    """Dataset for loading RL experiences."""

    def __init__(self, experiences: List[Dict]):
        self.experiences = experiences

        # Pre-convert to tensors for efficiency
        self.obs = torch.tensor(
            [e['observation'] for e in experiences],
            dtype=torch.float32
        )
        self.actions = torch.tensor(
            [e['action'] for e in experiences],
            dtype=torch.long
        )
        self.old_log_probs = torch.tensor(
            [e['log_prob'] for e in experiences],
            dtype=torch.float32
        )
        self.advantages = torch.tensor(
            [e['advantage'] for e in experiences],
            dtype=torch.float32
        )
        self.returns = torch.tensor(
            [e['return'] for e in experiences],
            dtype=torch.float32
        )
        self.old_values = torch.tensor(
            [e['value'] for e in experiences],
            dtype=torch.float32
        )

    def __len__(self):
        return len(self.experiences)

    def __getitem__(self, idx):
        return {
            'obs': self.obs[idx],
            'action': self.actions[idx],
            'old_log_prob': self.old_log_probs[idx],
            'advantage': self.advantages[idx],
            'return': self.returns[idx],
            'old_value': self.old_values[idx],
        }

    def get_rollout_buffer(self) -> Dict[str, torch.Tensor]:
        """Get all data as tensors (for PPO update)."""
        return {
            'obs': self.obs,
            'actions': self.actions,
            'old_log_probs': self.old_log_probs,
            'advantages': self.advantages,
            'returns': self.returns,
            'old_values': self.old_values,
        }


def load_experiences(jsonl_path: str) -> List[Dict]:
    """Load experiences from JSONL file."""
    experiences = []
    with open(jsonl_path, 'r') as f:
        for line in f:
            exp = json.loads(line.strip())
            experiences.append(exp)
    return experiences
```

**Steps:**
1. Implement `RLDataset` class
2. Add `load_experiences()` function
3. Pre-convert to tensors for speed
4. Add batch collation

**Acceptance Criteria:**
- [ ] Loads JSONL files
- [ ] Converts to PyTorch tensors
- [ ] Handles large files efficiently
- [ ] DataLoader works with it

---

### Phase 2 Deliverables

**Code Files:**
- `tools/ppo/config.py` - Configuration
- `tools/ppo/network.py` - Actor-critic network
- `tools/ppo/advantages.py` - GAE computation
- `tools/ppo/trainer.py` - PPO update
- `tools/ppo/dataset.py` - Data loading
- `config/ppo_default.yaml` - Default config

**Tests:**
- Unit tests for network forward pass
- Unit tests for GAE computation
- Unit tests for PPO update
- Integration test with dummy data

**Validation Command:**
```bash
cd tools
python -m ppo.network
python -m ppo.advantages
python -m ppo.trainer
# All tests should pass
```

---

## Phase 3: Training Pipeline

**Goal:** Implement orchestration, checkpointing, and evaluation.

**Estimated Effort:** 4-6 hours

### Task 3.1: Implement Weight Export/Import
**File:** `tools/ppo/utils.py`
**Effort:** 1-2 hours

**Implementation:**
```python
import torch
import json
import numpy as np
from pathlib import Path
from .network import ActorCriticNetwork

def export_policy_to_json(
    network: ActorCriticNetwork,
    output_path: str,
    schema_version: str = "1.1.0",
    schema_hash: str = "placeholder"
):
    """Export network weights to JSON format compatible with Rust."""

    state_dict = network.state_dict()

    # Extract weights
    manifest = {
        "schema_version": schema_version,
        "schema_hash": schema_hash,
        "layer1": {
            "weights": state_dict['shared.0.weight'].cpu().numpy().flatten().tolist(),
            "biases": state_dict['shared.0.bias'].cpu().numpy().tolist(),
        },
        "layer2": {
            "weights": state_dict['shared.2.weight'].cpu().numpy().flatten().tolist(),
            "biases": state_dict['shared.2.bias'].cpu().numpy().tolist(),
        },
        "layer3": {
            "weights": state_dict['actor.weight'].cpu().numpy().flatten().tolist(),
            "biases": state_dict['actor.bias'].cpu().numpy().tolist(),
        },
    }

    with open(output_path, 'w') as f:
        json.dump(manifest, f, indent=2)

    print(f"Exported policy to {output_path}")


def save_checkpoint(
    network: ActorCriticNetwork,
    optimizer: torch.optim.Optimizer,
    iteration: int,
    metrics: dict,
    checkpoint_dir: str,
):
    """Save training checkpoint."""

    checkpoint_dir = Path(checkpoint_dir)
    checkpoint_dir.mkdir(parents=True, exist_ok=True)

    checkpoint = {
        'iteration': iteration,
        'network_state_dict': network.state_dict(),
        'optimizer_state_dict': optimizer.state_dict(),
        'metrics': metrics,
    }

    checkpoint_path = checkpoint_dir / f"checkpoint_{iteration:04d}.pth"
    torch.save(checkpoint, checkpoint_path)

    # Also save as latest
    latest_path = checkpoint_dir / "latest.pth"
    torch.save(checkpoint, latest_path)

    return checkpoint_path


def load_checkpoint(
    checkpoint_path: str,
    network: ActorCriticNetwork,
    optimizer: torch.optim.Optimizer = None,
) -> dict:
    """Load training checkpoint."""

    checkpoint = torch.load(checkpoint_path)
    network.load_state_dict(checkpoint['network_state_dict'])

    if optimizer is not None:
        optimizer.load_state_dict(checkpoint['optimizer_state_dict'])

    return checkpoint
```

**Steps:**
1. Implement `export_policy_to_json()`
2. Implement `save_checkpoint()` and `load_checkpoint()`
3. Test save/load cycle preserves weights
4. Validate JSON format matches Rust expectations

**Acceptance Criteria:**
- [ ] Exported JSON loads in Rust
- [ ] Checkpoint save/load is lossless
- [ ] Tests pass

---

### Task 3.2: Implement Evaluation Harness
**File:** `tools/evaluate_policy.py`
**Effort:** 1 hour

**Implementation:**
```python
#!/usr/bin/env python3
"""Evaluate a trained policy against baselines."""

import subprocess
import json
import argparse
from pathlib import Path

def evaluate_policy(
    policy_path: str,
    baseline: str = "normal",
    num_games: int = 100,
    mdhearts_binary: str = "mdhearts",
) -> dict:
    """
    Evaluate policy performance.

    Args:
        policy_path: Path to policy JSON
        baseline: Baseline AI type (easy/normal/hard)
        num_games: Number of games to evaluate
        mdhearts_binary: Path to mdhearts executable

    Returns:
        Metrics dict with avg_points, total_points, etc.
    """

    # Run evaluation
    cmd = [
        mdhearts_binary, "eval", str(num_games),
        "--ai", "embedded",
        "--weights", policy_path,
    ]

    result = subprocess.run(
        cmd,
        capture_output=True,
        text=True,
        check=True
    )

    # Parse final summary JSON
    lines = result.stdout.strip().split('\n')
    for line in reversed(lines):
        if line.strip().startswith('{'):
            metrics = json.loads(line)
            break
    else:
        raise ValueError("Could not find metrics JSON in output")

    # Also evaluate baseline
    baseline_cmd = [mdhearts_binary, "eval", str(num_games), "--ai", baseline]
    baseline_result = subprocess.run(
        baseline_cmd,
        capture_output=True,
        text=True,
        check=True
    )

    baseline_lines = baseline_result.stdout.strip().split('\n')
    for line in reversed(baseline_lines):
        if line.strip().startswith('{'):
            baseline_metrics = json.loads(line)
            break

    # Compute delta
    policy_avg = metrics['avg_points'][0]  # South player
    baseline_avg = baseline_metrics['avg_points'][0]
    delta = policy_avg - baseline_avg

    return {
        'policy_avg_points': policy_avg,
        'baseline_avg_points': baseline_avg,
        'delta': delta,
        'improved': delta < 0,
        'full_metrics': metrics,
    }


if __name__ == '__main__':
    parser = argparse.ArgumentParser()
    parser.add_argument('policy', type=str, help='Path to policy JSON')
    parser.add_argument('--baseline', type=str, default='normal')
    parser.add_argument('--games', type=int, default=100)
    parser.add_argument('--mdhearts', type=str, default='mdhearts')

    args = parser.parse_args()

    results = evaluate_policy(
        args.policy,
        args.baseline,
        args.games,
        args.mdhearts
    )

    print(f"\nEvaluation Results:")
    print(f"  Policy:   {results['policy_avg_points']:.2f} avg points")
    print(f"  Baseline: {results['baseline_avg_points']:.2f} avg points")
    print(f"  Delta:    {results['delta']:+.2f} ({'improved' if results['improved'] else 'worse'})")
```

**Steps:**
1. Implement `evaluate_policy()` function
2. Add baseline comparison
3. Parse JSON metrics from mdhearts output
4. Add CLI interface

**Acceptance Criteria:**
- [ ] Runs mdhearts eval successfully
- [ ] Parses metrics correctly
- [ ] Computes improvement delta
- [ ] CLI works

---

### Task 3.3: Implement Training Orchestrator
**File:** `tools/train_ppo_pipeline.py`
**Effort:** 2-3 hours

**Implementation:**
```python
#!/usr/bin/env python3
"""PPO training pipeline orchestrator."""

import subprocess
import json
import argparse
from pathlib import Path
from collections import defaultdict
import torch
from torch.utils.tensorboard import SummaryWriter

from ppo.config import PPOConfig
from ppo.network import ActorCriticNetwork
from ppo.trainer import PPOTrainer
from ppo.dataset import RLDataset, load_experiences
from ppo.advantages import process_episodes
from ppo.utils import export_policy_to_json, save_checkpoint, load_checkpoint
from evaluate_policy import evaluate_policy


class TrainingPipeline:
    """Main training orchestrator."""

    def __init__(self, config: PPOConfig, mdhearts_binary: str = "mdhearts"):
        self.config = config
        self.mdhearts_binary = mdhearts_binary

        # Setup directories
        self.checkpoint_dir = Path(config.checkpoint_dir)
        self.checkpoint_dir.mkdir(parents=True, exist_ok=True)

        self.log_dir = Path(config.log_dir)
        self.log_dir.mkdir(parents=True, exist_ok=True)

        # Create network and trainer
        self.network = ActorCriticNetwork(
            obs_dim=config.obs_dim,
            hidden_dim=config.hidden_dim,
            action_dim=config.action_dim,
        )
        self.trainer = PPOTrainer(self.network, config)

        # TensorBoard
        if config.use_tensorboard:
            self.writer = SummaryWriter(log_dir=str(self.log_dir))
        else:
            self.writer = None

        # Tracking
        self.iteration = 0
        self.best_performance = float('inf')
        self.training_log = []

    def run(self):
        """Main training loop."""

        print("="*60)
        print("PPO Training Pipeline")
        print("="*60)
        print(f"Games per iteration: {self.config.games_per_iteration}")
        print(f"Total iterations: {self.config.num_iterations}")
        print(f"Reward mode: {self.config.reward_mode}")
        print("="*60)

        for self.iteration in range(self.config.num_iterations):
            print(f"\n{'='*60}")
            print(f"Iteration {self.iteration + 1}/{self.config.num_iterations}")
            print(f"{'='*60}")

            # Phase 1: Collect experiences
            print("\n[1/5] Collecting experiences...")
            experiences = self.collect_experiences()
            print(f"  ✓ Collected {len(experiences)} experiences")

            # Phase 2: Process with GAE
            print("\n[2/5] Computing advantages...")
            processed = process_episodes(
                experiences,
                gamma=self.config.gamma,
                gae_lambda=self.config.gae_lambda,
            )
            print(f"  ✓ Processed {len(processed)} experiences")

            # Phase 3: Train PPO
            print("\n[3/5] Training PPO...")
            dataset = RLDataset(processed)
            rollout_buffer = dataset.get_rollout_buffer()
            metrics = self.trainer.update(rollout_buffer)
            self.log_metrics("train", metrics)
            print(f"  ✓ Policy loss: {metrics['policy_loss']:.4f}")
            print(f"  ✓ Value loss: {metrics['value_loss']:.4f}")
            print(f"  ✓ Entropy: {metrics['entropy']:.4f}")

            # Phase 4: Evaluate
            if self.iteration % self.config.eval_frequency == 0:
                print("\n[4/5] Evaluating policy...")
                eval_results = self.evaluate()
                self.log_metrics("eval", eval_results)
                print(f"  ✓ Avg points: {eval_results['policy_avg_points']:.2f}")
                print(f"  ✓ Delta vs baseline: {eval_results['delta']:+.2f}")
            else:
                eval_results = None

            # Phase 5: Checkpoint
            if self.iteration % self.config.save_frequency == 0 or \
               (eval_results and eval_results['policy_avg_points'] < self.best_performance):
                print("\n[5/5] Saving checkpoint...")
                self.save_checkpoint(eval_results)

                if eval_results and eval_results['policy_avg_points'] < self.best_performance:
                    self.best_performance = eval_results['policy_avg_points']
                    print(f"  ✓ New best policy! ({self.best_performance:.2f} avg points)")

            # Log to CSV
            self.log_iteration(metrics, eval_results)

        print("\n" + "="*60)
        print("Training complete!")
        print(f"Best performance: {self.best_performance:.2f} avg points")
        print("="*60)

    def collect_experiences(self) -> list:
        """Run self-play to collect experiences."""

        # Export current policy
        iter_dir = self.checkpoint_dir / f"iter_{self.iteration:04d}"
        iter_dir.mkdir(exist_ok=True)

        policy_path = iter_dir / "policy.json"
        export_policy_to_json(self.network, str(policy_path))

        # Run mdhearts
        data_path = iter_dir / "experiences.jsonl"
        cmd = [
            self.mdhearts_binary, "eval", str(self.config.games_per_iteration),
            "--self-play",
            "--policy", str(policy_path),
            "--collect-rl", str(data_path),
            "--reward-mode", self.config.reward_mode,
        ]

        subprocess.run(cmd, check=True)

        # Load experiences
        return load_experiences(str(data_path))

    def evaluate(self) -> dict:
        """Evaluate current policy."""

        # Export for evaluation
        eval_path = self.checkpoint_dir / f"iter_{self.iteration:04d}" / "eval_policy.json"
        export_policy_to_json(self.network, str(eval_path))

        # Run evaluation
        results = evaluate_policy(
            str(eval_path),
            baseline="normal",
            num_games=self.config.eval_games,
            mdhearts_binary=self.mdhearts_binary,
        )

        return results

    def save_checkpoint(self, eval_results=None):
        """Save training checkpoint."""

        metrics = {
            'iteration': self.iteration,
            'best_performance': self.best_performance,
            'eval_results': eval_results,
        }

        checkpoint_path = save_checkpoint(
            self.network,
            self.trainer.optimizer,
            self.iteration,
            metrics,
            str(self.checkpoint_dir),
        )

        # Also export best policy JSON
        best_policy_path = self.checkpoint_dir / "best_policy.json"
        export_policy_to_json(self.network, str(best_policy_path))

    def log_metrics(self, prefix: str, metrics: dict):
        """Log metrics to TensorBoard."""

        if self.writer is None:
            return

        for key, value in metrics.items():
            if isinstance(value, (int, float)):
                self.writer.add_scalar(f"{prefix}/{key}", value, self.iteration)

    def log_iteration(self, train_metrics: dict, eval_results: dict):
        """Log iteration to CSV."""

        log_entry = {
            'iteration': self.iteration,
            'policy_loss': train_metrics['policy_loss'],
            'value_loss': train_metrics['value_loss'],
            'entropy': train_metrics['entropy'],
            'approx_kl': train_metrics['approx_kl'],
        }

        if eval_results:
            log_entry['eval_avg_points'] = eval_results['policy_avg_points']
            log_entry['eval_delta'] = eval_results['delta']

        self.training_log.append(log_entry)

        # Write CSV
        import csv
        csv_path = self.log_dir / "training_log.csv"

        with open(csv_path, 'w', newline='') as f:
            if self.training_log:
                writer = csv.DictWriter(f, fieldnames=self.training_log[0].keys())
                writer.writeheader()
                writer.writerows(self.training_log)


def main():
    parser = argparse.ArgumentParser(description='Train PPO policy for Hearts')
    parser.add_argument('--config', type=str, default='config/ppo_default.yaml',
                       help='Path to config YAML')
    parser.add_argument('--mdhearts', type=str, default='mdhearts',
                       help='Path to mdhearts binary')
    parser.add_argument('--resume', type=str, default=None,
                       help='Resume from checkpoint')

    args = parser.parse_args()

    # Load config
    config = PPOConfig.from_yaml(args.config)

    # Create pipeline
    pipeline = TrainingPipeline(config, mdhearts_binary=args.mdhearts)

    # Resume if requested
    if args.resume:
        checkpoint = load_checkpoint(
            args.resume,
            pipeline.network,
            pipeline.trainer.optimizer
        )
        pipeline.iteration = checkpoint['iteration'] + 1
        pipeline.best_performance = checkpoint['metrics'].get('best_performance', float('inf'))
        print(f"Resumed from iteration {checkpoint['iteration']}")

    # Run training
    pipeline.run()


if __name__ == '__main__':
    main()
```

**Steps:**
1. Implement `TrainingPipeline` class
2. Add main training loop
3. Integrate collection, processing, training, eval
4. Add TensorBoard logging
5. Add CSV logging
6. Add checkpoint management
7. Add resume support

**Acceptance Criteria:**
- [ ] Pipeline runs end-to-end
- [ ] Checkpoints are saved
- [ ] TensorBoard shows metrics
- [ ] CSV log is created
- [ ] Resume works correctly

---

### Phase 3 Deliverables

**Code Files:**
- `tools/ppo/utils.py` - Export/checkpoint utilities
- `tools/evaluate_policy.py` - Evaluation harness
- `tools/train_ppo_pipeline.py` - Main orchestrator
- `config/ppo_default.yaml` - Default configuration

**Validation Command:**
```bash
# Full training run (1 iteration for testing)
python tools/train_ppo_pipeline.py --config config/ppo_default.yaml

# Should:
# - Collect experiences
# - Train PPO
# - Evaluate
# - Save checkpoint
# - Create TensorBoard logs
```

---

## Phase 4: Integration & Testing

**Goal:** End-to-end testing, bug fixes, documentation.

**Estimated Effort:** 4-6 hours

### Task 4.1: End-to-End Integration Test
**Effort:** 2 hours

**Test Script:**
```bash
#!/bin/bash
# test_pipeline.sh

set -e

echo "Building Rust binary..."
cargo build --release

echo "Creating test config..."
cat > config/test.yaml <<EOF
learning_rate: 3e-4
games_per_iteration: 5
num_iterations: 3
eval_games: 10
checkpoint_dir: test_checkpoints
log_dir: test_logs
EOF

echo "Running training pipeline..."
python tools/train_ppo_pipeline.py \
  --config config/test.yaml \
  --mdhearts ./target/release/mdhearts.exe

echo "Validating outputs..."
test -f test_checkpoints/best_policy.json || (echo "Missing best policy" && exit 1)
test -f test_logs/training_log.csv || (echo "Missing training log" && exit 1)
test -d test_logs || (echo "Missing TensorBoard logs" && exit 1)

echo "Testing evaluation..."
python tools/evaluate_policy.py \
  test_checkpoints/best_policy.json \
  --games 10

echo "✓ All integration tests passed!"
```

**Steps:**
1. Create integration test script
2. Run on small-scale config
3. Validate all outputs created
4. Check for crashes/errors
5. Verify metrics are sane

**Acceptance Criteria:**
- [ ] Script runs without errors
- [ ] All expected files created
- [ ] Metrics are finite
- [ ] Policy improves over random baseline

---

### Task 4.2: Bug Fixes & Edge Cases
**Effort:** 2-3 hours

**Common Issues to Address:**
1. **Empty episodes:** Handle games with 0 experiences
2. **NaN losses:** Add checks and early stopping
3. **OOM errors:** Batch size tuning
4. **File not found:** Better error messages
5. **Windows paths:** Cross-platform compatibility

**Testing Checklist:**
- [ ] Handle empty JSONL files
- [ ] Handle interrupted training
- [ ] Handle invalid checkpoints
- [ ] Handle missing mdhearts binary
- [ ] Validate all file paths exist

---

### Task 4.3: Documentation
**Effort:** 1-2 hours

**Files to Create:**
1. **`docs/TRAINING_GUIDE.md`** - User guide for running training
2. **`README.md` updates** - Add PPO training section
3. **Code comments** - Docstrings for all public functions

**TRAINING_GUIDE.md Outline:**
```markdown
# Training Guide

## Quick Start
```bash
# 1. Collect initial data
mdhearts eval 100 --ai normal --collect-data warmstart.jsonl

# 2. Train with PPO
python tools/train_ppo_pipeline.py --config config/ppo_default.yaml

# 3. Evaluate trained policy
python tools/evaluate_policy.py checkpoints/best_policy.json
```

## Configuration
...

## Monitoring
...

## Troubleshooting
...
```

**Acceptance Criteria:**
- [ ] New user can follow guide successfully
- [ ] All commands are correct
- [ ] Configuration options explained
- [ ] Common errors documented

---

### Phase 4 Deliverables

**Tests:**
- Integration test script
- Edge case handling
- Error message validation

**Documentation:**
- Training guide
- Updated README
- Code docstrings

**Validation:**
```bash
./test_pipeline.sh
# Should complete successfully
```

---

## Final Deliverables Summary

### Rust Components
- [x] `RLExperience` struct with PPO fields
- [x] `RLExperienceCollector` for JSONL writing
- [x] `EmbeddedPolicy::forward_with_critic()`
- [x] `RewardComputer` with shaped rewards
- [x] Self-play CLI mode (`--self-play`, `--collect-rl`)
- [x] Updated `Policy` trait

### Python Components
- [x] `PPOConfig` configuration management
- [x] `ActorCriticNetwork` with shared trunk
- [x] `PPOTrainer` with clipped objective
- [x] GAE computation (`advantages.py`)
- [x] `RLDataset` for data loading
- [x] Weight export/import utilities
- [x] Evaluation harness
- [x] Training orchestrator

### Infrastructure
- [x] Checkpoint management
- [x] TensorBoard logging
- [x] CSV metric logging
- [x] Resume support
- [x] Configuration files

### Documentation
- [x] Training guide
- [x] API documentation
- [x] Troubleshooting guide

---

## Timeline Estimate

**Sequential (Part-time):**
- Week 1: Phase 1 (Rust data collection)
- Week 2: Phase 2 (Python PPO core)
- Week 3: Phase 3 (Training pipeline)
- Week 4: Phase 4 (Testing & docs)

**Parallel (Full-time):**
- Days 1-2: Phase 1
- Days 3-4: Phase 2
- Days 5-6: Phase 3
- Day 7: Phase 4

---

## Success Criteria

### Minimum Viable Product (MVP)
- [ ] Pipeline runs end-to-end without crashes
- [ ] Collects experiences from self-play
- [ ] Trains PPO for 10 iterations
- [ ] Saves checkpoints
- [ ] Evaluates against baseline
- [ ] Logs metrics to TensorBoard

### Full Success
- [ ] Trained policy beats random baseline
- [ ] Training is stable (no divergence)
- [ ] Metrics track correctly
- [ ] Documentation is complete
- [ ] Integration tests pass
- [ ] Ready for production training runs

### Stretch Goals
- [ ] Policy beats Normal AI (< 6.0 avg points)
- [ ] Converges in < 50 iterations
- [ ] Reproduces with fixed seed
- [ ] Performance visualizations generated

---

## Risk Mitigation

| Risk | Impact | Mitigation |
|------|--------|-----------|
| PPO diverges | High | Gradient clipping, KL monitoring, checkpoint rollback |
| Data collection slow | Medium | Optimize Rust code, use release builds |
| OOM during training | Medium | Reduce batch size, gradient accumulation |
| Python/Rust interface breaks | High | Extensive integration tests, JSON schema validation |
| Can't beat heuristics | High | Hyperparameter tuning, reward shaping, more iterations |

---

## Next Steps

1. **Review this plan** - Confirm approach and estimates
2. **Set up environment** - Install dependencies, verify tools
3. **Start Phase 1** - Begin with Task 1.1 (RLExperience struct)
4. **Iterate** - Complete tasks sequentially, test at each step
5. **Deploy** - Run full training when pipeline is stable

---

## Appendix: Commands Cheat Sheet

```bash
# Development
cargo test --workspace                    # Run Rust tests
python -m pytest tools/tests/             # Run Python tests
cargo clippy --workspace -- -D warnings   # Lint Rust
black tools/ && flake8 tools/             # Lint Python

# Data collection
mdhearts eval 100 --self-play --collect-rl data.jsonl --reward-mode shaped

# Training
python tools/train_ppo_pipeline.py --config config/ppo_default.yaml

# Evaluation
python tools/evaluate_policy.py checkpoints/best_policy.json --games 100

# Monitoring
tensorboard --logdir logs/

# Resume training
python tools/train_ppo_pipeline.py --resume checkpoints/latest.pth
```
