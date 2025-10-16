# Hearts AI Research Summary

**Date**: 2025-10-15
**Author**: Claude
**Purpose**: Research findings on Hearts AI strategies, algorithms, and implementations to inform heuristic improvements

---

## Executive Summary

**Key Finding**: As of 2021, there is still no superhuman Hearts AI. The best known AI (Moving AI Lab, 2013) beat Hearts Deluxe but hasn't been updated in over 10 years. This represents a significant opportunity for our implementation.

**Primary Algorithms**:
1. **Monte Carlo Tree Search (MCTS)** with UCT - Most successful approach
2. **Perfect Information Monte Carlo (PIMC)** - Sample opponent hands, use maxn/paranoid search
3. **Reinforcement Learning** with feature construction - Sub-human performance
4. **Rule-Based Heuristics** - Baseline approaches

---

## 1. State of the Art

### Moving AI Lab (University of Alberta, 2013)
**Reference**: https://www.movingai.com/hearts.html

**Approach**:
- Monte-Carlo sampling of opponent hands
- UCT (Upper Confidence Bounds for Trees) algorithm
- Custom node ordering with speculative pruning
- Objective: Minimize expected score

**Performance**:
- 90-game tournament vs Hearts Deluxe
- Score: 55.8 (their AI) vs 75.1 (Hearts Deluxe) - **Won by ~25%**
- Improved "shooting the moon" handling in later versions

**Limitations** (acknowledged by authors):
- Imperfect opponent hand generation
- Need for improved opponent modeling

**Status**: Free program released at ns-software.com/Hearts/, last updated 2013

**Key Insight**: This is the benchmark to beat. 12-year-old AI is still state-of-the-art.

---

## 2. Core Algorithms

### 2.1 Monte Carlo Tree Search (MCTS)

**Concept**:
- Sample possible opponent hands from unseen cards
- For each sampled configuration, run game tree search
- Aggregate results across all samples to select best move

**Variants Used in Hearts**:

1. **Information Set MCTS (ISMCTS)**
   - Handles imperfect information by treating information sets
   - "Good results" reported for trick-taking games
   - Works with just knowledge of game rules

2. **Perfect Information Monte Carlo (PIMC)**
   - Sample opponent hands (treat as perfect information)
   - Run maxn or paranoid search on each sample
   - Average results
   - **Breakthrough in Bridge (GIB player) - on par with human experts**

**Implementation Details**:
- Lookahead depth: 1-4 cards ahead (based on trick state)
- Search algorithm: maxn (multiplayer minimax) or paranoid
- UCT for tree exploration/exploitation balance

**Pros**:
- Handles imperfect information
- Anytime algorithm (can be interrupted)
- Proven track record in trick-taking games

**Cons**:
- Computationally expensive (needs many rollouts)
- Opponent modeling limited to current hand distribution
- No learning across games

### 2.2 Maxn and Paranoid Search

**Maxn Algorithm**:
- Generalization of minimax to >2 players
- Each node stores tuple of values (one per player)
- Player i selects move maximizing value[i]
- Used at leaf nodes in MCTS rollouts

**Paranoid Algorithm**:
- Assumes all opponents cooperate against you
- Simplifies to 2-player minimax (you vs coalition)
- More pessimistic but computationally cheaper

**Usage in Hearts**:
- Depth-limited search (1-4 plies)
- Heuristic evaluation at leaf nodes (expected score)

### 2.3 Policy-Based Inference (PI)

**Reference**: "Policy Based Inference in Trick-Taking Card Games" (Skat paper)

**Concept**:
- Track probability distributions over opponent cards
- Update probabilities based on observed actions
- Opponents' plays reveal information about their hands

**Example**:
- If opponent doesn't follow suit → they are void in that suit
- If opponent plays high card → likely trying to take trick
- If opponent plays low card → likely trying to avoid points

**Benefits**:
- Improves move selection in determinized search
- Better opponent hand sampling (weight by probability)
- Enables opponent modeling

**Implementation**:
- "Assumptions matrix": P(card C held by player P)
- Update matrix after each play using Bayesian inference

---

## 3. Machine Learning Approaches

### 3.1 Reinforcement Learning (RL)

**Reference**: Sturtevant et al., "Feature Construction for Reinforcement Learning in Hearts"

**Approach**:
- Stochastic Linear Regression
- Temporal Difference Learning (TDL)
- Handcrafted features (e.g., "will I take Queen of Spades?")

**Result**: **Sub-human performance**

**Why It Failed**:
- Feature engineering difficult for complex game
- Sparse rewards (only at end of round)
- Multi-player credit assignment problem
- Non-stationary opponents (if using self-play)

**Takeaway**: Pure RL struggled without domain knowledge or sophisticated opponent modeling

### 3.2 Neural Networks

**Reference**: "Teaching a Neural Network to Play Cards" (Medium article)

**Approach**:
- Train neural network via self-play
- Iterative: Start with random data, then model plays itself
- No human expert data required

**Result**: "Reasonably good" play without human input

**Challenges**:
- Requires massive amounts of self-play data
- Prone to catastrophic forgetting (as we experienced!)
- Difficult to debug/understand failures

**Takeaway**: NNs can work but need careful training methodology (which we discovered the hard way in Gen3/Gen4)

---

## 4. Open Source Implementations

### 4.1 Devking/HeartsAI (Java)
**URL**: https://github.com/Devking/HeartsAI

**AI Players Implemented**:
1. RandomPlayAI - Random valid card
2. LowPlayAI - Always play lowest card
3. LookAheadPlayer - Minimal lookahead search
4. MCTSPlayer - Monte Carlo Tree Search

**Architecture**:
- Abstract `Player` class
- `State` class for game state tracking
- Supports shooting the moon
- Designed for easy extension/comparison

**Value**: Framework for testing different approaches

### 4.2 Rohoe/CS4700_HeartsAI (Python)
**URL**: https://github.com/Rohoe/CS4700_HeartsAI

**Implementation**: Monte Carlo Tree Search

**Reference**: Used Jeff Bradberry's MCTS tutorial
(https://jeffbradberry.com/posts/2015/09/intro-to-monte-carlo-tree-search/)

**Value**: Python MCTS implementation to study

### 4.3 zmcx16/OpenAI-Gym-Hearts (Python)
**URL**: https://github.com/zmcx16/OpenAI-Gym-Hearts

**Purpose**: OpenAI Gym environment for Hearts

**Value**:
- Standardized RL environment
- Easy data collection for ML/RL experiments
- Pre-built for experimentation

---

## 5. Strategic Insights from Research

### 5.1 The Two Core Problems

All imperfect information games face:

1. **Move Selection**: What's the best move given my information?
2. **Inference**: What can I infer about opponent hands from their actions?

**Hearts-specific**: 4-player makes opponent modeling 3x harder than 2-player games

### 5.2 Key Strategies (from competitive AI)

**Passing**:
- Not just "dump high hearts"
- Consider passing direction (left/right/across)
- Create voids strategically
- Set up or prevent moon shots

**Leading**:
- Opening: Start safe, gather information
- Mid-game: Lead from safe suits, exploit voids
- End-game: Minimize expected score

**Following**:
- Position matters: last to play has most control
- Slough (dump high cards) when safe
- Take tricks strategically to control flow

**Moon Shot**:
- Early detection crucial (analyze passed cards)
- Coordinated defense (force early points)
- Opportunistic shooting (recognize when you can succeed)

**Card Counting**:
- Track played cards
- Infer voids from failed follows
- Probability distributions for unseen cards

**Opponent Modeling**:
- Identify aggressive vs cautious players
- Track moon shot tendencies
- Adapt strategy per opponent

### 5.3 What Makes Hearts Hard for AI?

From literature and forum discussions:

1. **Large game tree**: 52! permutations, practical depth limited to 1-4 plies
2. **Imperfect information**: Can't see 39 opponent cards (3 hands of 13)
3. **Multi-player**: 4 players with conflicting goals
4. **Non-cooperative**: Unlike Bridge, no formal partnerships
5. **Shooting the moon**: Sudden payoff inversion (all points → no points)
6. **Long-term strategy**: 4 rounds of passing affect entire hand

---

## 6. Gap Analysis: Our Bot vs State-of-the-Art

### What We Have ✓
- Card tracking (`unseen_tracker.rs`)
- Basic heuristic planning (`play_planner.rs`, `pass_planner.rs`)
- Difficulty levels
- Bot personalities (Cautious, AggressiveMoon, HuntLeader)

### What We're Missing ✗

1. **Monte Carlo sampling**
   - No opponent hand sampling
   - No rollout-based evaluation
   - Limited lookahead (heuristics only)

2. **Probabilistic inference**
   - Track what's played, but not probability distributions
   - No void inference from failed follows
   - No Bayesian updates

3. **Opponent modeling**
   - No tracking of opponent tendencies
   - No adaptation to opponent strategies
   - Each opponent treated identically

4. **Advanced passing strategy**
   - Simple "dump high cards"
   - Doesn't consider direction
   - Doesn't create voids intentionally

5. **Coordinated moon defense**
   - Reactive only
   - No early detection from passed cards
   - No proactive blocking

6. **Endgame optimization**
   - No special endgame strategy
   - Could improve lead selection in final tricks

---

## 7. Recommended Improvements (Prioritized)

### Tier 1: High Impact, Medium Complexity (Quick Wins)

**1. Probabilistic Card Tracking**
- Upgrade `unseen_tracker.rs` to track probabilities
- Void detection from failed follows
- Expected: 10-15% win rate improvement

**2. Passing Strategy Overhaul**
- Direction-aware passing
- Void creation
- Moon shot setup/prevention
- Expected: 8-12% improvement

**3. Moon Shot Detection**
- Analyze passed cards for moon intent
- Early detection system
- Proactive defense triggers
- Expected: 10-15% improvement

### Tier 2: High Impact, High Complexity (Major Features)

**4. MCTS Integration** (Optional/Future)
- Opponent hand sampling
- Limited depth rollouts (depth 2-3)
- UCT-based move selection
- Expected: 15-25% improvement (if implemented well)

**5. Opponent Modeling**
- Track player tendencies
- Bayesian strategy inference
- Adaptive play
- Expected: 5-10% improvement

### Tier 3: Lower Priority Refinements

**6. Lead/Follow Optimization**
- Position-aware following
- Better lead selection (especially endgame)
- Expected: 5-8% improvement

**7. Parameter Tuning**
- Grid search for heuristic weights
- Risk threshold optimization
- Expected: 3-5% improvement

---

## 8. Implementation Strategy

Based on research, our best path forward:

**Phase 1**: Upgrade heuristic bot with Tier 1 improvements
- Probabilistic tracking
- Better passing
- Moon detection

**Expected Result**: 30-40% win rate improvement, close to Moving AI Lab level

**Phase 2** (if needed): Add MCTS with limited depth
- Light-weight rollouts (depth 2-3 only)
- Hybrid: MCTS for move selection, heuristics for evaluation
- Balance accuracy vs. speed (<100ms per move)

**Expected Result**: Potential superhuman performance (if implemented well)

**Phase 3**: BC training on improved bot
- Collect 50k+ games from Phase 1 or Phase 2 bot
- Train neural network
- Deploy for fast inference

---

## 9. Key Takeaways

1. **MCTS is king** - Most successful approach for Hearts AI
2. **Opponent modeling matters** - Even simple probability tracking helps
3. **No superhuman AI exists yet** - Opportunity to create one
4. **Heuristics can be strong** - Moving AI Lab beat commercial software
5. **RL struggles without help** - Pure RL failed, hybrid approaches better
6. **Domain knowledge is crucial** - Can't learn Hearts from scratch easily

---

## 10. References

### Academic Papers
- Feature Construction for Reinforcement Learning in Hearts (Sturtevant et al.)
  https://sites.ualberta.ca/~amw8/hearts.pdf

- Learning To Play Hearts (University of Alberta)
  https://webdocs.cs.ualberta.ca/~nathanst/papers/heartslearning.pdf

- Policy Based Inference in Trick-Taking Card Games
  https://www.semanticscholar.org/paper/Policy-Based-Inference-in-Trick-Taking-Card-Games-Rebstock-Solinas/c8e58b8a019dbebad13d108ba49b77ac349cfe69

- Real-Time Opponent Modelling in Trick-Taking Card Games (IJCAI 2011)
  https://www.ijcai.org/Proceedings/11/Papers/110.pdf

- Determinization with Monte Carlo Tree Search for the card game Hearts
  https://studenttheses.uu.nl/bitstream/handle/20.500.12932/37736/Thesis_draft.pdf

### Online Resources
- Moving AI Lab Hearts Project
  https://www.movingai.com/hearts.html

- Jeff Bradberry: Intro to Monte Carlo Tree Search
  https://jeffbradberry.com/posts/2015/09/intro-to-monte-carlo-tree-search/

- Teaching a Neural Network to Play Cards (Medium)
  https://medium.com/data-science/teaching-a-neural-network-to-play-cards-bb6a42c09e20

### GitHub Repositories
- HeartsAI Framework (Devking)
  https://github.com/Devking/HeartsAI

- CS4700 Hearts AI with MCTS (Rohoe)
  https://github.com/Rohoe/CS4700_HeartsAI

- OpenAI Gym Hearts Environment (zmcx16)
  https://github.com/zmcx16/OpenAI-Gym-Hearts

- Open Hearts (Node.js) (snollygolly)
  https://github.com/snollygolly/open-hearts

### Stack Exchange Discussions
- Best techniques for an AI of a card game
  https://softwareengineering.stackexchange.com/questions/213870/best-techniques-for-an-ai-of-a-card-game

- Why multiplayer, imperfect information, trick-taking card games are hard for AI
  https://ai.stackexchange.com/questions/25439/why-multiplayer-imperfect-information-trick-taking-card-games-are-hard-for-ai

- What is the current state of the art in hearts AI?
  https://boardgames.stackexchange.com/questions/31718/what-is-the-current-state-of-the-art-in-hearts-ai

---

## Appendix: Specific Algorithm Pseudocode

### A.1 Perfect Information Monte Carlo (PIMC)

```
function select_move(my_hand, visible_cards, game_state):
    scores = {}  # move -> list of scores

    for sample in range(NUM_SAMPLES):
        # Sample opponent hands from unseen cards
        opponent_hands = sample_consistent_with_observations(
            unseen_cards,
            voids=known_voids,
            constraints=play_history
        )

        # Evaluate each possible move
        for move in legal_moves(my_hand):
            # Run deterministic search with this sample
            final_score = maxn_search(
                move,
                my_hand,
                opponent_hands,
                depth=LOOKAHEAD_DEPTH
            )
            scores[move].append(final_score)

    # Select move with best average score
    best_move = min(scores, key=lambda m: mean(scores[m]))
    return best_move
```

### A.2 Void Inference

```
function update_probabilities_after_play(player, card_played, suit_led):
    if card_played.suit != suit_led:
        # Player is void in led suit
        for card in unseen_cards:
            if card.suit == suit_led:
                P[player][card] = 0.0  # Can't have these cards

        # Renormalize probabilities
        normalize_probabilities(P[player])

    # Remove played card from possibilities
    for p in all_players:
        P[p][card_played] = 0.0
```

### A.3 Simple MCTS for Hearts

```
class MCTSNode:
    def __init__(self, game_state, parent=None):
        self.state = game_state
        self.parent = parent
        self.children = []
        self.visits = 0
        self.total_score = 0.0

    def uct_value(self, exploration=sqrt(2)):
        if self.visits == 0:
            return float('inf')
        exploit = self.total_score / self.visits
        explore = exploration * sqrt(log(self.parent.visits) / self.visits)
        return exploit + explore

function mcts_search(root_state, iterations):
    root = MCTSNode(root_state)

    for _ in range(iterations):
        # 1. Selection
        node = root
        while node.children and not node.is_terminal():
            node = max(node.children, key=lambda n: n.uct_value())

        # 2. Expansion
        if not node.is_terminal():
            moves = node.legal_moves()
            for move in moves:
                child_state = node.state.apply_move(move)
                child = MCTSNode(child_state, parent=node)
                node.children.append(child)

        # 3. Simulation (rollout)
        score = random_rollout(node.state)

        # 4. Backpropagation
        while node:
            node.visits += 1
            node.total_score += score
            node = node.parent

    # Return best move
    best_child = min(root.children, key=lambda n: n.total_score / n.visits)
    return best_child.state.last_move
```

---

**End of Research Summary**
