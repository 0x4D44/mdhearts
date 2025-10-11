# RL Training Pipeline Specification

## 1. Overview

### 1.1 Objective
Build a complete Reinforcement Learning training pipeline using Proximal Policy Optimization (PPO) to train a Hearts AI that surpasses heuristic-based opponents.

### 1.2 Success Criteria
- Trained policy achieves lower average points than Normal AI across 1000+ games
- Training converges within 50k-100k game episodes
- Supports self-play for continuous improvement
- Full pipeline automatable via single command

### 1.3 Scope
**In Scope:**
- PPO training algorithm with actor-critic architecture
- Experience collection with advantage estimation
- Self-play iteration framework
- Performance tracking and visualization
- Checkpoint management

**Out of Scope:**
- Multi-agent PPO (all opponents use same policy for simplicity)
- Distributed training (single-machine only)
- Neural architecture search
- Model compression/optimization

## 2. High-Level Design

### 2.1 System Architecture

```
┌─────────────────────────────────────────────────────────────┐
│                   Training Orchestrator                      │
│  (Python: train_ppo.py)                                      │
│  - Iteration loop                                            │
│  - Checkpoint management                                     │
│  - Metrics tracking                                          │
└──────────────┬──────────────────────────────┬────────────────┘
               │                              │
               v                              v
┌──────────────────────────┐    ┌────────────────────────────┐
│   Experience Generator   │    │     PPO Trainer           │
│   (Rust: mdhearts)       │    │     (Python: PyTorch)     │
│                          │    │                           │
│  - Self-play games       │    │  - Actor-Critic network   │
│  - Observation building  │    │  - Advantage estimation   │
│  - Reward computation    │    │  - Policy optimization    │
│  - JSONL output          │    │  - Value function fitting │
└──────────────┬───────────┘    └────────────┬───────────────┘
               │                              │
               v                              v
        ┌─────────────────────────────────────────┐
        │       Experience Buffer (JSONL)         │
        │  - Observations (270 features)          │
        │  - Actions (card IDs)                   │
        │  - Rewards (per step)                   │
        │  - Values (critic estimates)            │
        │  - Log probabilities                    │
        │  - Advantages (computed)                │
        └─────────────────────────────────────────┘
```

### 2.2 Data Flow

```
1. Collection Phase:
   mdhearts eval --self-play --policy current.json --collect-rl data.jsonl
   ↓
   [obs₁, action₁, reward₁, done₁, value₁, log_prob₁]
   [obs₂, action₂, reward₂, done₂, value₂, log_prob₂]
   ...

2. Processing Phase:
   compute_advantages(data.jsonl) → data_with_advantages.jsonl
   ↓
   [obs₁, action₁, advantage₁, return₁, old_log_prob₁]
   ...

3. Training Phase:
   train_ppo(data_with_advantages.jsonl, current.npz) → improved.npz
   ↓
   Update actor and critic networks using PPO loss

4. Evaluation Phase:
   mdhearts eval --ai embedded --weights improved.json --baseline normal
   ↓
   Performance metrics vs baseline

5. Iteration:
   current.json ← improved.json
   Go to step 1
```

## 3. Component Specifications

### 3.1 Rust Components (mdhearts)

#### 3.1.1 Enhanced Experience Collection
**File:** `crates/hearts-app/src/rl/experience.rs`

**New Fields:**
```rust
pub struct RLExperience {
    pub observation: Vec<f32>,  // Existing
    pub action: u8,             // Existing
    pub reward: f32,            // Existing (per-step, not just terminal)
    pub done: bool,             // Existing
    pub game_id: usize,         // Existing
    pub step_id: usize,         // Existing
    pub seat: u8,               // Existing

    // NEW for PPO:
    pub value: f32,             // Value estimate from critic
    pub log_prob: f32,          // Log probability of action
    pub next_observation: Option<Vec<f32>>,  // For bootstrapping
}
```

**Requirements:**
- During collection, run forward pass through current policy
- Store value predictions and action log probabilities
- Support step-wise rewards (not just terminal)
- Track episode boundaries for GAE computation

#### 3.1.2 Self-Play Mode
**File:** `crates/hearts-app/src/cli.rs`

**New CLI Arguments:**
```bash
mdhearts eval <games> \
  --self-play \                    # All 4 players use same policy
  --collect-rl <path> \            # Enhanced RL data collection
  --policy <path> \                # Policy for all players
  --step-rewards                   # Compute per-step rewards
```

**Requirements:**
- All 4 seats use identical policy for training
- Compute intermediate rewards, not just terminal
- Collect experiences from all 4 perspectives (4x data per game)

#### 3.1.3 Step-Wise Reward Shaping
**File:** `crates/hearts-app/src/rl/env.rs`

**Reward Function:**
```rust
pub enum RewardMode {
    Terminal,           // Existing: only at episode end
    PerTrick,          // NEW: after each trick
    Shaped,            // NEW: intermediate + terminal
}

impl RewardMode {
    fn compute_step_reward(&self, ctx: &StepContext) -> f32 {
        match self {
            Terminal => 0.0,  // Wait for episode end
            PerTrick => {
                if trick_complete {
                    -(points_taken as f32) / 26.0
                } else {
                    0.0
                }
            }
            Shaped => {
                let mut reward = 0.0;

                // Negative reward for taking points
                if trick_complete && won_trick {
                    reward -= points_in_trick as f32 / 26.0;
                }

                // Small penalty for playing high cards early
                if hearts_broken && played_heart && trick_position == 0 {
                    reward -= 0.01;
                }

                // Small bonus for voiding suits
                if created_void {
                    reward += 0.02;
                }

                reward
            }
        }
    }
}
```

### 3.2 Python Components

#### 3.2.1 Actor-Critic Network
**File:** `tools/ppo/network.py`

**Architecture:**
```python
class ActorCriticNetwork(nn.Module):
    """Shared trunk with separate actor and critic heads."""

    def __init__(self):
        super().__init__()
        # Shared trunk
        self.shared = nn.Sequential(
            nn.Linear(270, 256),
            nn.ReLU(),
            nn.Linear(256, 128),
            nn.ReLU(),
        )

        # Actor head (policy)
        self.actor = nn.Linear(128, 52)  # Action logits

        # Critic head (value function)
        self.critic = nn.Linear(128, 1)   # State value

    def forward(self, obs, legal_mask=None):
        features = self.shared(obs)
        logits = self.actor(features)

        # Mask illegal actions
        if legal_mask is not None:
            logits = logits.masked_fill(~legal_mask, -1e9)

        value = self.critic(features)
        return logits, value
```

#### 3.2.2 PPO Trainer
**File:** `tools/ppo/trainer.py`

**Core Algorithm:**
```python
class PPOTrainer:
    def __init__(self, network, lr=3e-4, gamma=0.99, gae_lambda=0.95,
                 clip_epsilon=0.2, value_coef=0.5, entropy_coef=0.01):
        self.network = network
        self.optimizer = optim.Adam(network.parameters(), lr=lr)
        self.gamma = gamma
        self.gae_lambda = gae_lambda
        self.clip_epsilon = clip_epsilon
        self.value_coef = value_coef
        self.entropy_coef = entropy_coef

    def compute_gae(self, rewards, values, dones):
        """Generalized Advantage Estimation."""
        advantages = []
        gae = 0

        for t in reversed(range(len(rewards))):
            if t == len(rewards) - 1:
                next_value = 0 if dones[t] else values[t]
            else:
                next_value = values[t + 1]

            delta = rewards[t] + self.gamma * next_value - values[t]
            gae = delta + self.gamma * self.gae_lambda * (1 - dones[t]) * gae
            advantages.insert(0, gae)

        returns = [adv + val for adv, val in zip(advantages, values)]
        return advantages, returns

    def update(self, batch, epochs=4):
        """PPO update with clipped objective."""
        obs, actions, old_log_probs, advantages, returns = batch

        # Normalize advantages
        advantages = (advantages - advantages.mean()) / (advantages.std() + 1e-8)

        for _ in range(epochs):
            # Forward pass
            logits, values = self.network(obs)
            dist = Categorical(logits=logits)

            new_log_probs = dist.log_prob(actions)
            entropy = dist.entropy().mean()

            # PPO clipped objective
            ratio = torch.exp(new_log_probs - old_log_probs)
            surr1 = ratio * advantages
            surr2 = torch.clamp(ratio, 1 - self.clip_epsilon,
                                       1 + self.clip_epsilon) * advantages
            actor_loss = -torch.min(surr1, surr2).mean()

            # Value loss (MSE)
            value_loss = F.mse_loss(values.squeeze(), returns)

            # Total loss
            loss = actor_loss + self.value_coef * value_loss - self.entropy_coef * entropy

            # Optimize
            self.optimizer.zero_grad()
            loss.backward()
            nn.utils.clip_grad_norm_(self.network.parameters(), 0.5)
            self.optimizer.step()
```

#### 3.2.3 Training Orchestrator
**File:** `tools/train_ppo_pipeline.py`

**Main Loop:**
```python
def train_ppo_pipeline(
    num_iterations=100,
    games_per_iter=100,
    ppo_epochs=4,
    batch_size=256,
):
    network = ActorCriticNetwork()
    trainer = PPOTrainer(network)

    best_avg_points = float('inf')

    for iteration in range(num_iterations):
        print(f"\n=== Iteration {iteration + 1}/{num_iterations} ===")

        # 1. Export current policy
        export_policy(network, 'current_policy.json')

        # 2. Collect experiences via self-play
        run_subprocess([
            'mdhearts', 'eval', str(games_per_iter),
            '--self-play',
            '--policy', 'current_policy.json',
            '--collect-rl', f'data_iter_{iteration}.jsonl',
            '--step-rewards'
        ])

        # 3. Load and process data
        dataset = load_rl_experiences(f'data_iter_{iteration}.jsonl')

        # 4. Compute advantages
        dataset = compute_advantages(dataset, trainer)

        # 5. Train PPO
        for epoch in range(ppo_epochs):
            for batch in DataLoader(dataset, batch_size=batch_size, shuffle=True):
                trainer.update(batch)

        # 6. Evaluate against baseline
        export_policy(network, 'eval_policy.json')
        metrics = evaluate_policy('eval_policy.json', baseline='normal')

        # 7. Track progress
        avg_points = metrics['avg_points'][0]  # South player
        print(f"Avg Points: {avg_points:.2f} (Best: {best_avg_points:.2f})")

        if avg_points < best_avg_points:
            best_avg_points = avg_points
            export_policy(network, 'best_policy.json')
            print("✓ New best policy!")

        # 8. Log metrics
        log_metrics(iteration, metrics, 'training_log.csv')
```

#### 3.2.4 Advantage Computation
**File:** `tools/ppo/advantages.py`

```python
def compute_advantages_for_episodes(experiences, gamma=0.99, lam=0.95):
    """Group by episode and compute GAE for each."""

    # Group by game_id and seat
    episodes = defaultdict(list)
    for exp in experiences:
        key = (exp['game_id'], exp['seat'])
        episodes[key].append(exp)

    processed = []

    for (game_id, seat), episode in episodes.items():
        # Sort by step_id
        episode = sorted(episode, key=lambda x: x['step_id'])

        rewards = [e['reward'] for e in episode]
        values = [e['value'] for e in episode]
        dones = [e['done'] for e in episode]

        # Compute GAE
        advantages, returns = compute_gae(rewards, values, dones, gamma, lam)

        for i, exp in enumerate(episode):
            exp['advantage'] = advantages[i]
            exp['return'] = returns[i]
            processed.append(exp)

    return processed
```

## 4. Implementation Phases

### Phase 1: Enhanced Data Collection (Rust)
**Deliverables:**
- `RLExperience` struct with value/log_prob fields
- `--self-play` mode in CLI
- Step-wise reward computation
- Enhanced `ExperienceCollector`

**Estimated Effort:** 4-6 hours

### Phase 2: PPO Core (Python)
**Deliverables:**
- `ActorCriticNetwork` class
- `PPOTrainer` with GAE
- Advantage computation utilities
- Unit tests for PPO math

**Estimated Effort:** 6-8 hours

### Phase 3: Training Pipeline (Python)
**Deliverables:**
- `train_ppo_pipeline.py` orchestrator
- Checkpoint management
- Metrics logging (CSV + TensorBoard)
- Baseline evaluation

**Estimated Effort:** 4-6 hours

### Phase 4: Integration & Tuning
**Deliverables:**
- End-to-end pipeline test
- Hyperparameter tuning
- Documentation
- Training guide

**Estimated Effort:** 4-6 hours

**Total:** 18-26 hours

## 5. Hyperparameters

### 5.1 Default Configuration
```yaml
# Data collection
games_per_iteration: 100
collect_all_seats: true  # 400 episodes per iteration

# PPO parameters
learning_rate: 3e-4
gamma: 0.99              # Discount factor
gae_lambda: 0.95         # GAE smoothing
clip_epsilon: 0.2        # PPO clip range
value_coef: 0.5          # Value loss coefficient
entropy_coef: 0.01       # Exploration bonus
ppo_epochs: 4            # Updates per batch
batch_size: 256

# Training
num_iterations: 100
max_episodes: 100000
early_stopping: true
patience: 10             # Iterations without improvement

# Reward shaping
reward_mode: "shaped"
normalize_rewards: true
```

### 5.2 Expected Performance Trajectory

| Iteration | Avg Points | Notes |
|-----------|------------|-------|
| 0 (Random) | 8.5 | Baseline random policy |
| 1 (BC) | 6.5 | After behavioral cloning warmstart |
| 10 | 6.0 | Learning basic strategy |
| 30 | 5.5 | Improving over heuristic |
| 50 | 5.0 | Strong performance |
| 100 | 4.5-5.0 | Convergence |

## 6. Metrics & Monitoring

### 6.1 Training Metrics (per iteration)
- Average episode reward
- Policy loss
- Value loss
- Entropy (exploration)
- KL divergence (policy change)
- Explained variance (value function quality)

### 6.2 Evaluation Metrics
- Average points per seat (vs Normal AI)
- Win rate (lowest points)
- Moon shooting frequency
- Average game length

### 6.3 Logging Format
**CSV:** `training_log.csv`
```csv
iteration,avg_reward,policy_loss,value_loss,entropy,eval_avg_points,eval_vs_normal
0,−0.31,2.45,1.23,3.21,6.5,+0.1
1,−0.28,2.01,0.98,3.15,6.3,−0.1
...
```

**TensorBoard:** Real-time plots
- Reward curve
- Loss curves
- Evaluation performance
- Gradient norms

## 7. File Structure

```
mdhearts/
├── crates/hearts-app/src/
│   ├── rl/
│   │   ├── experience.rs         # RLExperience struct
│   │   └── env.rs                # Shaped rewards
│   └── cli.rs                    # --self-play mode
│
├── tools/
│   ├── ppo/
│   │   ├── __init__.py
│   │   ├── network.py            # ActorCriticNetwork
│   │   ├── trainer.py            # PPOTrainer
│   │   ├── advantages.py         # GAE computation
│   │   └── dataset.py            # PyTorch Dataset
│   │
│   ├── train_ppo_pipeline.py     # Main orchestrator
│   ├── evaluate_policy.py        # Eval harness
│   └── plot_training.py          # Visualization
│
├── experiments/
│   ├── iter_000/
│   │   ├── data.jsonl
│   │   ├── policy.json
│   │   └── metrics.json
│   └── ...
│
└── docs/
    ├── RL_TRAINING_SPEC.md       # This document
    └── TRAINING_GUIDE.md         # User guide
```

## 8. Risk Mitigation

### 8.1 Training Instability
**Risk:** PPO diverges or gets stuck in local minima

**Mitigation:**
- Gradient clipping (max_norm=0.5)
- Learning rate scheduling
- KL divergence monitoring
- Checkpoint rollback if performance degrades

### 8.2 Sample Inefficiency
**Risk:** Requires too many games to converge

**Mitigation:**
- Start with behavioral cloning warmstart
- Collect from all 4 seats (4x data)
- Experience replay buffer
- Curriculum learning (vs Easy → Normal → Hard)

### 8.3 Overfitting to Self-Play
**Risk:** Policy only good against itself

**Mitigation:**
- Regular evaluation vs heuristic baselines
- Mixed opponent pool during training
- Population-based training (multiple policies)

## 9. Success Metrics

### 9.1 Minimum Viable Product (MVP)
- [ ] Pipeline runs end-to-end without manual intervention
- [ ] Trained policy achieves < 6.0 avg points vs Normal AI
- [ ] Training completes in < 24 hours on consumer hardware

### 9.2 Full Success
- [ ] Policy achieves < 5.5 avg points vs Normal AI (10% improvement)
- [ ] Converges in < 50k episodes
- [ ] Generalizes to different opponent types
- [ ] Reproduces with fixed seed

## 10. Future Extensions

### 10.1 Advanced Techniques
- Multi-agent PPO (different policies per seat)
- Opponent modeling (predict other players' hands)
- Hierarchical RL (passing vs playing as separate policies)
- Curiosity-driven exploration

### 10.2 Infrastructure
- Distributed training (Ray/RLlib)
- Cloud training support
- Model serving API
- Web-based evaluation dashboard

## 11. References

- **PPO Paper:** [Proximal Policy Optimization Algorithms](https://arxiv.org/abs/1707.06347)
- **GAE Paper:** [High-Dimensional Continuous Control Using Generalized Advantage Estimation](https://arxiv.org/abs/1506.02438)
- **Spinning Up:** [OpenAI's PPO Guide](https://spinningup.openai.com/en/latest/algorithms/ppo.html)
- **CleanRL:** [PPO Implementation Reference](https://github.com/vwxyzjn/cleanrl)
