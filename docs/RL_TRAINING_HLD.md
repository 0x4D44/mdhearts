# RL Training Pipeline - High-Level Design

## 1. System Overview

The RL training pipeline implements **Proximal Policy Optimization (PPO)** for training a Hearts AI through self-play. The system bridges Rust (game simulation) and Python (neural network training) components.

### 1.1 Design Goals

1. **Self-Contained:** Single command to run complete training
2. **Efficient:** 4 experiences per game (all seats)
3. **Stable:** PPO with proven hyperparameters
4. **Observable:** Comprehensive metrics and logging
5. **Reproducible:** Seeded training with checkpointing

## 2. Architecture Diagram

```
┌────────────────────────────────────────────────────────────────────┐
│                     Training Orchestrator                          │
│                   (train_ppo_pipeline.py)                          │
│                                                                    │
│  Main Loop:                                                        │
│  for iteration in 1..100:                                          │
│    1. Export policy → current_policy.json                          │
│    2. Collect experiences → subprocess(mdhearts eval)              │
│    3. Load experiences → RLDataset                                 │
│    4. Compute advantages → GAE                                     │
│    5. Train PPO → update network                                   │
│    6. Evaluate → compare vs baseline                               │
│    7. Checkpoint → save if improved                                │
└─────────────┬──────────────────────────────────┬───────────────────┘
              │                                  │
              │ Export JSON                      │ Subprocess call
              v                                  v
    ┌──────────────────┐              ┌─────────────────────────┐
    │  Policy Exporter │              │   Experience Collector  │
    │  (Python)        │              │   (Rust: mdhearts)      │
    │                  │              │                         │
    │  .npz → .json    │              │  --self-play mode       │
    │  Validate schema │              │  All 4 seats play       │
    └──────────────────┘              │  Step-wise rewards      │
                                      │  Value predictions      │
                                      │  Log probabilities      │
                                      └────────┬────────────────┘
                                               │
                                               v
                                      ┌─────────────────────┐
                                      │  data_iterN.jsonl   │
                                      │                     │
                                      │  Per experience:    │
                                      │  - obs (270)        │
                                      │  - action (1)       │
                                      │  - reward (1)       │
                                      │  - value (1)        │
                                      │  - log_prob (1)     │
                                      │  - done (bool)      │
                                      │  - game_id, step_id │
                                      │  - seat             │
                                      └────────┬────────────┘
                                               │
                                               v
    ┌──────────────────────────────────────────────────────────────┐
    │                  Advantage Computation                        │
    │                  (advantages.py)                              │
    │                                                               │
    │  Group by episode → Sort by step → Apply GAE formula:        │
    │                                                               │
    │  δₜ = rₜ + γVₜ₊₁ - Vₜ                                        │
    │  Aₜ = δₜ + (γλ)δₜ₊₁ + (γλ)²δₜ₊₂ + ...                       │
    │  Rₜ = Aₜ + Vₜ (returns)                                      │
    │                                                               │
    │  Normalize advantages: A' = (A - μ) / σ                      │
    └────────────┬──────────────────────────────────────────────────┘
                 │
                 v
    ┌──────────────────────────────────────────────────────────────┐
    │                    PPO Training Loop                          │
    │                    (trainer.py)                               │
    │                                                               │
    │  for epoch in 1..4:                                           │
    │    for batch in dataloader:                                   │
    │      1. Forward pass: logits, values = network(obs)           │
    │      2. Compute ratio: r = π_new / π_old                      │
    │      3. Clipped objective: L = min(r*A, clip(r)*A)            │
    │      4. Value loss: L_V = MSE(values, returns)                │
    │      5. Entropy bonus: H = -Σ π log π                         │
    │      6. Total loss: L_total = -L + c₁L_V - c₂H                │
    │      7. Backprop + gradient clip + optimizer step             │
    └────────────┬──────────────────────────────────────────────────┘
                 │
                 v
    ┌──────────────────────────────────────────────────────────────┐
    │                      Evaluation                               │
    │                  (evaluate_policy.py)                         │
    │                                                               │
    │  Run games:                                                   │
    │    mdhearts eval 100 --ai embedded --weights new.json        │
    │    mdhearts eval 100 --ai normal                              │
    │                                                               │
    │  Compare:                                                     │
    │    Δ avg_points = new_policy - baseline                      │
    │    Log to CSV + TensorBoard                                   │
    └───────────────────────────────────────────────────────────────┘
```

## 3. Component Details

### 3.1 Rust Components

#### 3.1.1 Enhanced Experience Structure

```rust
// crates/hearts-app/src/rl/experience.rs

pub struct RLExperience {
    // Existing fields
    pub observation: Vec<f32>,      // 270 features
    pub action: u8,                 // Card ID [0, 51]
    pub reward: f32,                // Immediate reward
    pub done: bool,                 // Episode terminal
    pub game_id: usize,
    pub step_id: usize,
    pub seat: u8,

    // NEW: PPO-specific fields
    pub value: f32,                 // V(s) from critic
    pub log_prob: f32,              // log π(a|s)
}
```

**Key Changes:**
- During collection, policy performs forward pass
- Store critic's value estimate: `value = network.critic(obs)`
- Store action log probability: `log_prob = log(π(action|obs))`

#### 3.1.2 Self-Play Game Loop

```rust
// crates/hearts-app/src/cli.rs

fn run_self_play_eval(
    num_games: usize,
    policy_path: &Path,
    output_path: &Path,
) -> Result<(), CliError> {
    // Load policy for all 4 seats
    let policy = EmbeddedPolicy::from_file(policy_path)?;
    let mut collector = RLExperienceCollector::new(output_path)?;

    for game_id in 0..num_games {
        let mut match_state = MatchState::with_seed(game_id as u64);

        // Each seat uses same policy
        for seat in ALL_SEATS {
            loop {
                if round_complete { break; }

                // Build observation
                let obs = obs_builder.build(&ctx);

                // Forward pass through policy
                let (action, value, log_prob) = policy.forward_with_critic(&obs);

                // Compute reward (step-wise)
                let reward = compute_step_reward(&match_state, seat);

                // Collect experience
                collector.record(RLExperience {
                    observation: obs.as_array().to_vec(),
                    action: action.to_id(),
                    reward,
                    done: is_terminal,
                    game_id,
                    step_id,
                    seat: seat.index() as u8,
                    value,
                    log_prob,
                })?;

                // Execute action
                match_state.round_mut().play_card(seat, action)?;
                step_id += 1;
            }
        }
    }

    Ok(())
}
```

**Data Volume:**
- 1 game = 52 plays per seat × 4 seats = 208 experiences
- 100 games = 20,800 experiences per iteration

#### 3.1.3 Step-Wise Reward Function

```rust
// crates/hearts-app/src/rl/rewards.rs

pub fn compute_step_reward(match_state: &MatchState, seat: PlayerPosition) -> f32 {
    let round = match_state.round();
    let current_trick = round.current_trick();

    let mut reward = 0.0;

    // Just completed a trick?
    if current_trick.is_complete() {
        let winner = current_trick.winner().unwrap();

        if winner == seat {
            // Took points - penalize
            let points = current_trick.penalty_total();
            reward -= points as f32 / 26.0;  // Normalize to [-1, 0]
        }
    }

    // Bonus: Successfully voided a suit
    if just_voided_suit(round.hand(seat)) {
        reward += 0.02;
    }

    // Penalty: Led with hearts too early
    if current_trick.plays().is_empty() && !round.hearts_broken() {
        if let Some(played) = get_played_card(seat) {
            if played.suit == Suit::Hearts {
                reward -= 0.05;
            }
        }
    }

    reward
}
```

**Reward Shaping:**
- Immediate feedback when taking points
- Small bonuses for strategic plays
- Normalized to [-1, 1] range

### 3.2 Python Components

#### 3.2.1 Actor-Critic Network Architecture

```python
# tools/ppo/network.py

class ActorCriticNetwork(nn.Module):
    def __init__(self, obs_dim=270, hidden_dim=256, action_dim=52):
        super().__init__()

        # Shared feature extractor
        self.shared = nn.Sequential(
            nn.Linear(obs_dim, hidden_dim),
            nn.ReLU(),
            nn.Linear(hidden_dim, hidden_dim // 2),
            nn.ReLU(),
        )

        # Policy head (actor)
        self.actor = nn.Linear(hidden_dim // 2, action_dim)

        # Value head (critic)
        self.critic = nn.Linear(hidden_dim // 2, 1)

        # Initialize weights
        self.apply(self._init_weights)

    def _init_weights(self, m):
        if isinstance(m, nn.Linear):
            nn.init.orthogonal_(m.weight, gain=np.sqrt(2))
            nn.init.constant_(m.bias, 0.0)

    def forward(self, obs, legal_mask=None):
        features = self.shared(obs)

        # Policy logits
        logits = self.actor(features)
        if legal_mask is not None:
            logits = logits.masked_fill(~legal_mask, -1e9)

        # State value
        value = self.critic(features).squeeze(-1)

        return logits, value

    def get_action_and_value(self, obs, legal_mask=None, action=None):
        """Used during collection for exploration."""
        logits, value = self.forward(obs, legal_mask)
        probs = Categorical(logits=logits)

        if action is None:
            action = probs.sample()

        return action, probs.log_prob(action), probs.entropy(), value
```

**Key Features:**
- Shared trunk reduces parameters
- Orthogonal initialization for stability
- Legal move masking preserves gradient flow
- Categorical distribution for discrete actions

#### 3.2.2 PPO Update Algorithm

```python
# tools/ppo/trainer.py

class PPOTrainer:
    def __init__(self, network, config):
        self.network = network
        self.optimizer = optim.Adam(network.parameters(), lr=config.lr)
        self.config = config

    def compute_gae(self, rewards, values, dones, next_values):
        """
        Generalized Advantage Estimation.

        Args:
            rewards: [T] tensor of rewards
            values: [T] tensor of value estimates
            dones: [T] tensor of episode terminals
            next_values: [T] tensor of next state values

        Returns:
            advantages: [T] GAE estimates
            returns: [T] value targets
        """
        advantages = torch.zeros_like(rewards)
        gae = 0

        for t in reversed(range(len(rewards))):
            if dones[t]:
                next_value = 0
            else:
                next_value = next_values[t]

            # TD error
            delta = rewards[t] + self.config.gamma * next_value - values[t]

            # Accumulate GAE
            gae = delta + self.config.gamma * self.config.gae_lambda * (1 - dones[t]) * gae
            advantages[t] = gae

        returns = advantages + values
        return advantages, returns

    def update(self, rollout_buffer):
        """
        PPO update using clipped objective.

        Args:
            rollout_buffer: Dict containing:
                - obs: [N, 270]
                - actions: [N]
                - old_log_probs: [N]
                - advantages: [N]
                - returns: [N]
                - legal_masks: [N, 52]
        """
        # Normalize advantages
        advantages = rollout_buffer['advantages']
        advantages = (advantages - advantages.mean()) / (advantages.std() + 1e-8)

        # Multiple epochs over same data
        for _ in range(self.config.ppo_epochs):
            # Shuffle and batch
            indices = torch.randperm(len(advantages))

            for start in range(0, len(indices), self.config.batch_size):
                end = start + self.config.batch_size
                batch_idx = indices[start:end]

                # Get batch
                b_obs = rollout_buffer['obs'][batch_idx]
                b_actions = rollout_buffer['actions'][batch_idx]
                b_old_log_probs = rollout_buffer['old_log_probs'][batch_idx]
                b_advantages = advantages[batch_idx]
                b_returns = rollout_buffer['returns'][batch_idx]
                b_legal_masks = rollout_buffer['legal_masks'][batch_idx]

                # Forward pass
                logits, values = self.network(b_obs, b_legal_masks)
                dist = Categorical(logits=logits)
                new_log_probs = dist.log_prob(b_actions)
                entropy = dist.entropy()

                # Policy loss (clipped)
                ratio = torch.exp(new_log_probs - b_old_log_probs)
                surr1 = ratio * b_advantages
                surr2 = torch.clamp(
                    ratio,
                    1 - self.config.clip_epsilon,
                    1 + self.config.clip_epsilon
                ) * b_advantages
                policy_loss = -torch.min(surr1, surr2).mean()

                # Value loss (clipped for stability)
                values_clipped = rollout_buffer['values'][batch_idx] + torch.clamp(
                    values - rollout_buffer['values'][batch_idx],
                    -self.config.clip_epsilon,
                    self.config.clip_epsilon
                )
                value_loss_unclipped = F.mse_loss(values, b_returns)
                value_loss_clipped = F.mse_loss(values_clipped, b_returns)
                value_loss = torch.max(value_loss_unclipped, value_loss_clipped).mean()

                # Entropy bonus (encourage exploration)
                entropy_loss = entropy.mean()

                # Total loss
                loss = (
                    policy_loss
                    + self.config.value_coef * value_loss
                    - self.config.entropy_coef * entropy_loss
                )

                # Optimize
                self.optimizer.zero_grad()
                loss.backward()
                nn.utils.clip_grad_norm_(self.network.parameters(), 0.5)
                self.optimizer.step()

                # Log metrics
                self.log_metrics({
                    'policy_loss': policy_loss.item(),
                    'value_loss': value_loss.item(),
                    'entropy': entropy_loss.item(),
                    'approx_kl': ((ratio - 1) - ratio.log()).mean().item(),
                })
```

**PPO Specifics:**
- **Clipped objective:** Prevents large policy updates
- **Value clipping:** Stabilizes critic training
- **Entropy bonus:** Maintains exploration
- **Gradient clipping:** Prevents exploding gradients
- **KL monitoring:** Early stopping if policy changes too much

#### 3.2.3 Training Orchestration

```python
# tools/train_ppo_pipeline.py

class TrainingPipeline:
    def __init__(self, config):
        self.config = config
        self.network = ActorCriticNetwork()
        self.trainer = PPOTrainer(self.network, config)
        self.iteration = 0
        self.best_performance = float('inf')

    def run(self):
        """Main training loop."""
        for self.iteration in range(self.config.num_iterations):
            print(f"\n{'='*60}")
            print(f"Iteration {self.iteration + 1}/{self.config.num_iterations}")
            print(f"{'='*60}")

            # Phase 1: Data collection
            experiences = self.collect_experiences()

            # Phase 2: Advantage computation
            processed = self.process_experiences(experiences)

            # Phase 3: PPO training
            metrics = self.train_ppo(processed)

            # Phase 4: Evaluation
            eval_results = self.evaluate()

            # Phase 5: Checkpointing
            self.checkpoint(eval_results)

            # Phase 6: Logging
            self.log_iteration(metrics, eval_results)

    def collect_experiences(self):
        """Run self-play to collect data."""
        # Export current policy
        policy_path = f'checkpoints/iter_{self.iteration:04d}/policy.json'
        self.export_policy(policy_path)

        # Run mdhearts
        data_path = f'checkpoints/iter_{self.iteration:04d}/experiences.jsonl'
        subprocess.run([
            'mdhearts', 'eval', str(self.config.games_per_iter),
            '--self-play',
            '--policy', policy_path,
            '--collect-rl', data_path,
            '--step-rewards'
        ], check=True)

        # Load experiences
        return self.load_experiences(data_path)

    def process_experiences(self, experiences):
        """Compute advantages using GAE."""
        # Group by episode
        episodes = defaultdict(list)
        for exp in experiences:
            key = (exp['game_id'], exp['seat'])
            episodes[key].append(exp)

        processed = []
        for episode in episodes.values():
            episode = sorted(episode, key=lambda x: x['step_id'])

            rewards = torch.tensor([e['reward'] for e in episode])
            values = torch.tensor([e['value'] for e in episode])
            dones = torch.tensor([e['done'] for e in episode])
            next_values = torch.cat([values[1:], torch.zeros(1)])

            # Compute GAE
            advantages, returns = self.trainer.compute_gae(
                rewards, values, dones, next_values
            )

            # Augment experiences
            for i, exp in enumerate(episode):
                exp['advantage'] = advantages[i].item()
                exp['return'] = returns[i].item()
                processed.append(exp)

        return processed

    def train_ppo(self, experiences):
        """Train network with PPO."""
        dataset = RLDataset(experiences)
        self.trainer.update(dataset)
        return self.trainer.get_metrics()

    def evaluate(self):
        """Evaluate against baselines."""
        policy_path = f'checkpoints/iter_{self.iteration:04d}/eval_policy.json'
        self.export_policy(policy_path)

        # Evaluate vs Normal AI
        result = subprocess.run(
            ['mdhearts', 'eval', '100', '--ai', 'embedded', '--weights', policy_path],
            capture_output=True, text=True
        )
        metrics = json.loads(result.stdout.split('\n')[-2])

        return {
            'avg_points': metrics['avg_points'][0],  # South player
            'total_points': metrics['total_points'][0],
        }
```

## 4. Data Flow Sequence

```
1. Initialization
   ├─ Load config.yaml
   ├─ Create ActorCriticNetwork (random weights)
   └─ Initialize PPOTrainer

2. Iteration Loop (100x)
   │
   ├─ [Collection Phase]
   │  ├─ Export policy.json
   │  ├─ Run: mdhearts eval 100 --self-play --collect-rl data.jsonl
   │  │  ├─ For each of 100 games:
   │  │  │  ├─ For each of 4 seats:
   │  │  │  │  ├─ For each of ~52 plays:
   │  │  │  │  │  ├─ obs = build_observation()
   │  │  │  │  │  ├─ (action, value, log_prob) = policy(obs)
   │  │  │  │  │  ├─ reward = compute_step_reward()
   │  │  │  │  │  └─ Write RLExperience to JSONL
   │  │  │  │  └─ (208 experiences per game)
   │  │  │  └─ (20,800 total experiences)
   │  └─ Load data.jsonl → List[RLExperience]
   │
   ├─ [Processing Phase]
   │  ├─ Group by (game_id, seat) → 400 episodes
   │  ├─ For each episode:
   │  │  ├─ Extract rewards, values, dones
   │  │  ├─ Compute GAE: advantages, returns
   │  │  └─ Augment experiences with A, R
   │  └─ Shuffle experiences → RLDataset
   │
   ├─ [Training Phase]
   │  ├─ For epoch in [1..4]:
   │  │  ├─ Shuffle dataset
   │  │  ├─ For batch in DataLoader(batch_size=256):
   │  │  │  ├─ Forward: logits, values = network(obs)
   │  │  │  ├─ Compute PPO loss
   │  │  │  ├─ Backward + clip grads
   │  │  │  └─ Optimizer step
   │  │  └─ Log metrics (policy_loss, value_loss, entropy)
   │  └─ Network weights updated
   │
   ├─ [Evaluation Phase]
   │  ├─ Export eval_policy.json
   │  ├─ Run: mdhearts eval 100 --ai embedded --weights eval_policy.json
   │  ├─ Parse metrics.json
   │  └─ Compare to baseline (Normal AI ≈ 6.5 avg points)
   │
   ├─ [Checkpointing Phase]
   │  ├─ If improved:
   │  │  ├─ Save network.pth
   │  │  ├─ Save policy.json
   │  │  └─ Update best_performance
   │  └─ Always save latest.pth
   │
   └─ [Logging Phase]
      ├─ Write to training_log.csv
      ├─ Write to TensorBoard
      └─ Print summary

3. Completion
   ├─ Export best_policy.json
   ├─ Generate training plots
   └─ Print final report
```

## 5. Configuration Schema

```yaml
# config/ppo_default.yaml

# Environment
env:
  reward_mode: "shaped"          # "terminal" | "per_trick" | "shaped"
  normalize_rewards: true
  collect_all_seats: true        # 4x data per game

# Data collection
collection:
  games_per_iteration: 100       # 20,800 experiences per iteration
  num_iterations: 100
  initial_policy: "behavioral_clone"  # "random" | "behavioral_clone"

# Network architecture
network:
  obs_dim: 270
  hidden_dim: 256
  action_dim: 52
  shared_trunk: true

# PPO hyperparameters
ppo:
  learning_rate: 3e-4
  gamma: 0.99                    # Discount factor
  gae_lambda: 0.95               # GAE parameter
  clip_epsilon: 0.2              # PPO clip range
  value_coef: 0.5                # Value loss coefficient
  entropy_coef: 0.01             # Entropy bonus
  max_grad_norm: 0.5             # Gradient clipping
  ppo_epochs: 4                  # Updates per iteration
  batch_size: 256
  normalize_advantages: true

# Training
training:
  max_episodes: 100000
  early_stopping: true
  patience: 10                   # Iterations without improvement
  checkpoint_frequency: 5
  eval_frequency: 1
  eval_games: 100

# Logging
logging:
  tensorboard: true
  csv_log: "training_log.csv"
  log_level: "INFO"
  print_frequency: 1
```

## 6. Success Criteria & Milestones

### Milestone 1: Pipeline Infrastructure (Week 1)
- [ ] RLExperience struct with value/log_prob
- [ ] --self-play mode functional
- [ ] ActorCriticNetwork implemented
- [ ] PPOTrainer with GAE
- [ ] Basic orchestrator runs end-to-end

**Success:** Pipeline runs 1 iteration without crashes

### Milestone 2: Training Stability (Week 2)
- [ ] Loss curves converge (not diverging)
- [ ] Entropy decreases smoothly
- [ ] KL divergence < 0.05
- [ ] Checkpointing works correctly

**Success:** 10 iterations complete with stable metrics

### Milestone 3: Performance Improvement (Week 3)
- [ ] Policy improves over behavioral clone baseline
- [ ] Avg points < 6.5 (beats random baseline)
- [ ] Generalizes to different seeds

**Success:** Measurable improvement in evaluation

### Milestone 4: Beat Heuristics (Week 4)
- [ ] Avg points < 6.0 vs Normal AI
- [ ] Consistent across 1000+ eval games
- [ ] Reproduces with fixed random seed

**Success:** Statistically significant improvement (p < 0.05)

## 7. Monitoring Dashboard

### Real-Time Metrics (TensorBoard)

**Scalars:**
- `train/policy_loss` - Clipped PPO objective
- `train/value_loss` - MSE between V(s) and returns
- `train/entropy` - Policy entropy (exploration)
- `train/approx_kl` - KL(π_old || π_new)
- `train/clip_fraction` - % of samples clipped
- `eval/avg_points` - Performance vs baseline
- `eval/win_rate` - % of games with lowest points

**Histograms:**
- `network/actor_logits` - Action distribution
- `network/values` - Value predictions
- `network/advantages` - Advantage estimates
- `gradients/*` - Gradient norms per layer

**Text:**
- `config` - Hyperparameters
- `best_iteration` - Iteration with best eval

### CSV Log Format

```csv
iteration,timestamp,policy_loss,value_loss,entropy,kl_div,eval_avg_points,eval_win_rate,checkpoint
0,2024-10-06T08:00:00,2.45,1.23,3.21,0.012,6.8,0.15,false
1,2024-10-06T08:05:00,2.31,1.15,3.18,0.010,6.6,0.18,false
2,2024-10-06T08:10:00,2.18,1.08,3.15,0.009,6.4,0.22,true
...
```

## 8. Failure Modes & Recovery

| Failure Mode | Symptoms | Recovery |
|--------------|----------|----------|
| Training divergence | Loss → NaN, rewards → -inf | Reduce LR, increase clip_epsilon, rollback checkpoint |
| Overfitting | Train improves, eval degrades | Add entropy bonus, reduce ppo_epochs |
| Sample inefficiency | No improvement after 50 iters | Increase games_per_iter, check reward shaping |
| Policy collapse | All actions → same card | Increase entropy_coef, check legal masking |
| Slow convergence | Loss decreases but plateaus | Warmstart with BC, tune GAE lambda |

## 9. Testing Strategy

### Unit Tests
- `test_gae_computation()` - Verify GAE math
- `test_ppo_loss()` - Verify clipped objective
- `test_advantage_normalization()` - Check μ=0, σ=1
- `test_legal_masking()` - Ensure no illegal actions

### Integration Tests
- `test_end_to_end_iteration()` - One full iteration
- `test_checkpoint_restore()` - Save/load consistency
- `test_determinism()` - Same seed → same results

### Performance Tests
- `test_collection_speed()` - > 100 games/sec
- `test_training_speed()` - One iteration < 5 min
- `test_memory_usage()` - < 4GB RAM

## 10. Deployment Checklist

- [ ] Code review complete
- [ ] All tests pass
- [ ] Documentation written
- [ ] Config validated
- [ ] Initial run successful (10 iterations)
- [ ] TensorBoard accessible
- [ ] Checkpoints saved correctly
- [ ] Evaluation scripts work
- [ ] Reproducible with seed
- [ ] Training guide written
