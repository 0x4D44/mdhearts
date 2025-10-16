//! Embedded neural network policy with compile-time weights.
//!
//! This module implements an MLP-based policy using weights embedded
//! at compile time. The network architecture is:
//!   - Input: 270 features (observation vector)
//!   - Hidden1: 256 units (ReLU)
//!   - Hidden2: 128 units (ReLU)
//!   - Output: 52 units (card logits)
//!
//! The policy respects legal move masks and uses schema validation
//! to ensure compatibility between observation encoding and model weights.

use super::{Policy, PolicyContext};
use crate::rl::observation::{ObservationBuilder, SCHEMA_HASH, SCHEMA_VERSION};
use crate::weights::loader::WeightManifest;
use crate::weights::{MODEL_SCHEMA_HASH, MODEL_SCHEMA_VERSION, layer1, layer2, layer3};
use hearts_core::model::card::Card;
use hearts_core::model::hand::Hand;
use hearts_core::model::player::PlayerPosition;
use hearts_core::model::round::RoundState;
use std::path::Path;

/// Embedded MLP policy using compile-time or loaded weights
pub struct EmbeddedPolicy {
    obs_builder: ObservationBuilder,
    custom_weights: Option<WeightManifest>,
}

impl EmbeddedPolicy {
    /// Create a new embedded policy using compiled-in weights
    ///
    /// # Panics
    /// Panics if the observation schema doesn't match the model's expected schema
    pub fn new() -> Self {
        // Validate schema compatibility at runtime
        if SCHEMA_VERSION != MODEL_SCHEMA_VERSION {
            panic!(
                "Schema version mismatch: observation uses {} but model expects {}",
                SCHEMA_VERSION, MODEL_SCHEMA_VERSION
            );
        }

        if SCHEMA_HASH != MODEL_SCHEMA_HASH {
            panic!(
                "Schema hash mismatch: observation hash {} but model expects {}",
                SCHEMA_HASH, MODEL_SCHEMA_HASH
            );
        }

        Self {
            obs_builder: ObservationBuilder::new(),
            custom_weights: None,
        }
    }

    /// Create a new embedded policy from a JSON string
    pub fn from_json_str(json: &str) -> Result<Self, String> {
        let weights = WeightManifest::from_json_str(json)?;

        // Validate schema compatibility
        if SCHEMA_VERSION != weights.schema_version {
            return Err(format!(
                "Schema version mismatch: observation uses {} but weights expect {}",
                SCHEMA_VERSION, weights.schema_version
            ));
        }

        if SCHEMA_HASH != weights.schema_hash {
            return Err(format!(
                "Schema hash mismatch: observation hash {} but weights expect {}",
                SCHEMA_HASH, weights.schema_hash
            ));
        }

        // Validate dimensions
        weights.validate()?;

        Ok(Self {
            obs_builder: ObservationBuilder::new(),
            custom_weights: Some(weights),
        })
    }

    /// Create a new embedded policy from a weight file
    pub fn from_file<P: AsRef<Path>>(path: P) -> Result<Self, String> {
        let weights = WeightManifest::from_file(path)?;

        // Validate schema compatibility
        if SCHEMA_VERSION != weights.schema_version {
            return Err(format!(
                "Schema version mismatch: observation uses {} but weights expect {}",
                SCHEMA_VERSION, weights.schema_version
            ));
        }

        if SCHEMA_HASH != weights.schema_hash {
            return Err(format!(
                "Schema hash mismatch: observation hash {} but weights expect {}",
                SCHEMA_HASH, weights.schema_hash
            ));
        }

        // Validate dimensions
        weights.validate()?;

        Ok(Self {
            obs_builder: ObservationBuilder::new(),
            custom_weights: Some(weights),
        })
    }

    /// Perform forward pass through the neural network
    ///
    /// Returns raw logits for all 52 cards (before legal move masking)
    fn forward(&self, input: &[f32; 270]) -> [f32; 52] {
        if let Some(ref weights) = self.custom_weights {
            // Use custom weights
            let mut hidden1 = [0.0f32; 256];
            matmul_add_bias(
                input,
                &weights.layer1.weights,
                &weights.layer1.biases,
                &mut hidden1,
                270,
                256,
            );
            relu(&mut hidden1);

            let mut hidden2 = [0.0f32; 128];
            matmul_add_bias(
                &hidden1,
                &weights.layer2.weights,
                &weights.layer2.biases,
                &mut hidden2,
                256,
                128,
            );
            relu(&mut hidden2);

            let mut output = [0.0f32; 52];
            matmul_add_bias(
                &hidden2,
                &weights.layer3.weights,
                &weights.layer3.biases,
                &mut output,
                128,
                52,
            );

            output
        } else {
            // Use compiled-in weights
            let mut hidden1 = [0.0f32; 256];
            matmul_add_bias(
                input,
                &layer1::WEIGHTS,
                &layer1::BIASES,
                &mut hidden1,
                270,
                256,
            );
            relu(&mut hidden1);

            let mut hidden2 = [0.0f32; 128];
            matmul_add_bias(
                &hidden1,
                &layer2::WEIGHTS,
                &layer2::BIASES,
                &mut hidden2,
                256,
                128,
            );
            relu(&mut hidden2);

            let mut output = [0.0f32; 52];
            matmul_add_bias(
                &hidden2,
                &layer3::WEIGHTS,
                &layer3::BIASES,
                &mut output,
                128,
                52,
            );

            output
        }
    }

    /// Apply legal move mask to logits and select the best card
    fn select_card_from_logits(&self, logits: &[f32; 52], legal_cards: &[Card]) -> Card {
        if legal_cards.is_empty() {
            panic!("No legal cards available");
        }

        // Find the legal card with highest logit
        let mut best_card = legal_cards[0];
        let mut best_logit = f32::NEG_INFINITY;

        for &card in legal_cards {
            let logit = logits[card.to_id() as usize];
            if logit > best_logit {
                best_logit = logit;
                best_card = card;
            }
        }

        best_card
    }
}

impl Default for EmbeddedPolicy {
    fn default() -> Self {
        Self::new()
    }
}

impl Policy for EmbeddedPolicy {
    fn choose_play(&mut self, ctx: &PolicyContext) -> Card {
        // Compute legal moves
        let legal_moves = compute_legal_moves(ctx.seat, ctx.hand, ctx.round);

        // Build observation from context
        let obs = self.obs_builder.build(ctx);
        let input = obs.as_array();

        // Run forward pass
        let logits = self.forward(&input);

        // Apply legal move mask and select best card
        self.select_card_from_logits(&logits, &legal_moves)
    }

    fn choose_pass(&mut self, ctx: &PolicyContext) -> [Card; 3] {
        // For passing, we'll use a simple heuristic:
        // Choose the 3 highest-valued cards from hand
        // (Neural network policy for passing is future work)

        let mut hand_vec: Vec<Card> = ctx.hand.iter().copied().collect();

        // Sort by descending "danger" (high cards in hearts/spades)
        hand_vec.sort_by(|a, b| {
            let a_score = card_danger_score(a);
            let b_score = card_danger_score(b);
            b_score.partial_cmp(&a_score).unwrap()
        });

        [hand_vec[0], hand_vec[1], hand_vec[2]]
    }

    fn forward_with_critic(&mut self, ctx: &PolicyContext) -> (Card, f32, f32) {
        // Compute legal moves
        let legal_moves = compute_legal_moves(ctx.seat, ctx.hand, ctx.round);

        // Build observation from context
        let obs = self.obs_builder.build(ctx);
        let input = obs.as_array();

        // Run forward pass to get logits
        let logits = self.forward(&input);

        // Apply legal move mask
        let mut masked_logits = [f32::NEG_INFINITY; 52];
        for &card in &legal_moves {
            masked_logits[card.to_id() as usize] = logits[card.to_id() as usize];
        }

        // Convert to probabilities via softmax
        let probs = softmax(&masked_logits);

        // Sample action from categorical distribution
        let action_idx = sample_categorical(&probs);
        let card = Card::from_id(action_idx as u8).expect("Invalid card ID");

        // Compute log probability
        let log_prob = probs[action_idx].ln();

        // Compute value estimate (placeholder until we have trained critic)
        let value = 0.0;

        (card, value, log_prob)
    }
}

/// Compute legal moves for a given hand and round state
fn compute_legal_moves(seat: PlayerPosition, hand: &Hand, round: &RoundState) -> Vec<Card> {
    hand.iter()
        .copied()
        .filter(|&card| {
            let mut probe = round.clone();
            probe.play_card(seat, card).is_ok()
        })
        .collect()
}

/// Compute a danger score for a card (higher = more dangerous to keep)
fn card_danger_score(card: &Card) -> f32 {
    use hearts_core::model::rank::Rank;
    use hearts_core::model::suit::Suit;

    let rank_value = card.rank.value() as f32;

    match card.suit {
        Suit::Hearts => rank_value * 2.0, // Hearts are dangerous
        Suit::Spades if card.rank == Rank::Queen => 100.0, // QS is most dangerous
        Suit::Spades => rank_value * 1.5, // Other spades somewhat dangerous
        _ => rank_value,                  // Clubs and diamonds less dangerous
    }
}

/// Matrix multiplication: output = input × weights^T + bias
///
/// Computes: output[j] = sum_i(input[i] * weights[j*input_size + i]) + bias[j]
///
/// Weights are stored in row-major order where each row represents
/// the weights for one output neuron.
fn matmul_add_bias(
    input: &[f32],
    weights: &[f32],
    biases: &[f32],
    output: &mut [f32],
    input_size: usize,
    output_size: usize,
) {
    debug_assert_eq!(input.len(), input_size);
    debug_assert_eq!(weights.len(), output_size * input_size);
    debug_assert_eq!(biases.len(), output_size);
    debug_assert_eq!(output.len(), output_size);

    for j in 0..output_size {
        let mut sum = biases[j];
        for i in 0..input_size {
            sum += input[i] * weights[j * input_size + i];
        }
        output[j] = sum;
    }
}

/// ReLU activation function: f(x) = max(0, x)
fn relu(x: &mut [f32]) {
    for val in x.iter_mut() {
        if *val < 0.0 {
            *val = 0.0;
        }
    }
}

/// Softmax activation: converts logits to probabilities
///
/// For numerical stability, we use the log-sum-exp trick:
/// softmax(x) = exp(x - max(x)) / sum(exp(x - max(x)))
fn softmax(logits: &[f32; 52]) -> [f32; 52] {
    // Find max for numerical stability
    let max_logit = logits
        .iter()
        .copied()
        .filter(|&x| x.is_finite())
        .fold(f32::NEG_INFINITY, f32::max);

    // Compute exp(x - max)
    let mut exp_values = [0.0f32; 52];
    let mut sum = 0.0;

    for i in 0..52 {
        if logits[i].is_finite() {
            exp_values[i] = (logits[i] - max_logit).exp();
            sum += exp_values[i];
        }
    }

    // Normalize
    let mut probs = [0.0f32; 52];
    for i in 0..52 {
        probs[i] = exp_values[i] / sum;
    }

    probs
}

/// Sample from categorical distribution
///
/// Returns the index of the sampled element
fn sample_categorical(probs: &[f32; 52]) -> usize {
    use rand::Rng;
    let mut rng = rand::thread_rng();
    let sample: f32 = rng.gen_range(0.0..1.0);

    let mut cumsum = 0.0;
    for (i, &p) in probs.iter().enumerate() {
        cumsum += p;
        if sample < cumsum {
            return i;
        }
    }

    // Fallback: return last valid index
    51
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::bot::UnseenTracker;
    use hearts_core::game::match_state::MatchState;
    use hearts_core::model::hand::Hand;
    use hearts_core::model::passing::PassingDirection;
    use hearts_core::model::player::PlayerPosition;
    use hearts_core::model::rank::Rank;
    use hearts_core::model::score::ScoreBoard;
    use hearts_core::model::suit::Suit;

    #[test]
    fn embedded_policy_creates_successfully() {
        let _policy = EmbeddedPolicy::new();
    }

    #[test]
    fn schema_validation_passes() {
        // This test will panic if schemas don't match
        let _policy = EmbeddedPolicy::new();

        // If we get here, schemas matched
        assert_eq!(SCHEMA_VERSION, MODEL_SCHEMA_VERSION);
        assert_eq!(SCHEMA_HASH, MODEL_SCHEMA_HASH);
    }

    #[test]
    fn forward_pass_produces_52_logits() {
        let policy = EmbeddedPolicy::new();
        let input = [0.0f32; 270];
        let logits = policy.forward(&input);
        assert_eq!(logits.len(), 52);
    }

    #[test]
    fn relu_activation_works() {
        let mut x = [-1.0, 0.0, 1.0, -5.0, 10.0];
        relu(&mut x);
        assert_eq!(x, [0.0, 0.0, 1.0, 0.0, 10.0]);
    }

    #[test]
    fn matmul_computes_correctly() {
        // Simple 2x2 matrix multiplication
        let input = [1.0, 2.0];
        let weights = [
            1.0, 0.0, // Row 0: [1, 0]
            0.0, 1.0, // Row 1: [0, 1]
        ];
        let biases = [0.5, 0.5];
        let mut output = [0.0, 0.0];

        matmul_add_bias(&input, &weights, &biases, &mut output, 2, 2);

        // output[0] = 1*1 + 2*0 + 0.5 = 1.5
        // output[1] = 1*0 + 2*1 + 0.5 = 2.5
        assert_eq!(output[0], 1.5);
        assert_eq!(output[1], 2.5);
    }

    #[test]
    fn select_card_respects_legal_mask() {
        let policy = EmbeddedPolicy::new();

        // Create logits favoring card 0, but make only card 10 legal
        let mut logits = [0.0f32; 52];
        logits[0] = 100.0; // High logit for card 0
        logits[10] = 1.0; // Low logit for card 10

        let legal_cards = vec![Card::from_id(10).unwrap()];
        let chosen = policy.select_card_from_logits(&logits, &legal_cards);

        // Should choose card 10 despite lower logit, because it's the only legal card
        assert_eq!(chosen, Card::from_id(10).unwrap());
    }

    #[test]
    fn policy_chooses_card_from_legal_moves() {
        use hearts_core::model::round::RoundPhase;

        let mut policy = EmbeddedPolicy::new();

        // Create a simple test round in playing phase
        let seat = PlayerPosition::South;
        let hand_cards = vec![
            Card::new(Rank::Two, Suit::Clubs),
            Card::new(Rank::Three, Suit::Clubs),
            Card::new(Rank::Four, Suit::Diamonds),
        ];

        let mut hands = [Hand::new(), Hand::new(), Hand::new(), Hand::new()];
        hands[seat.index()] = Hand::with_cards(hand_cards.clone());

        // Give other players some cards too
        for other_seat in PlayerPosition::LOOP {
            if other_seat != seat {
                hands[other_seat.index()] = Hand::with_cards(vec![
                    Card::new(Rank::Five, Suit::Hearts),
                    Card::new(Rank::Six, Suit::Hearts),
                ]);
            }
        }

        let round = hearts_core::model::round::RoundState::from_hands(
            hands,
            seat,
            PassingDirection::Hold,
            RoundPhase::Playing,
        );

        let hand = round.hand(seat);
        let tracker = UnseenTracker::new();
        let scores = ScoreBoard::new();

        let ctx = PolicyContext {
            hand,
            round: &round,
            scores: &scores,
            seat,
            tracker: &tracker,
            passing_direction: PassingDirection::Hold,
        };

        let chosen = policy.choose_play(&ctx);

        // Verify the chosen card is legal by checking it can be played
        let mut probe = round.clone();
        assert!(probe.play_card(seat, chosen).is_ok());
    }

    #[test]
    fn policy_chooses_pass_from_hand() {
        let mut policy = EmbeddedPolicy::new();

        // Create a simple hand
        let mut hand = Hand::new();
        hand.add(Card::new(Rank::Two, Suit::Clubs));
        hand.add(Card::new(Rank::Queen, Suit::Spades));
        hand.add(Card::new(Rank::Ace, Suit::Hearts));
        hand.add(Card::new(Rank::King, Suit::Diamonds));
        hand.add(Card::new(Rank::Three, Suit::Clubs));

        let match_state = MatchState::with_seed(PlayerPosition::South, 42);
        let tracker = UnseenTracker::new();
        let scores = ScoreBoard::new();

        let ctx = PolicyContext {
            hand: &hand,
            round: match_state.round(),
            scores: &scores,
            seat: PlayerPosition::South,
            tracker: &tracker,
            passing_direction: PassingDirection::Left,
        };

        let chosen_pass = policy.choose_pass(&ctx);

        // Should choose 3 cards from the hand
        assert_eq!(chosen_pass.len(), 3);
        for card in chosen_pass {
            assert!(hand.contains(card));
        }

        // Based on danger scoring, should prefer to pass QS, AH, KD
        // (Queen of Spades has danger score 100, Ace of Hearts has high danger)
        assert!(chosen_pass.contains(&Card::new(Rank::Queen, Suit::Spades)));
    }

    #[test]
    fn danger_score_ranks_queen_spades_highest() {
        let qs = Card::new(Rank::Queen, Suit::Spades);
        let ah = Card::new(Rank::Ace, Suit::Hearts);
        let two_clubs = Card::new(Rank::Two, Suit::Clubs);

        assert!(card_danger_score(&qs) > card_danger_score(&ah));
        assert!(card_danger_score(&ah) > card_danger_score(&two_clubs));
    }

    #[test]
    fn custom_weights_can_be_loaded() {
        use crate::weights::loader::{LayerWeights, WeightManifest};
        use std::io::Write;

        // Create a temporary weight file
        let temp_dir = std::env::temp_dir();
        let weight_path = temp_dir.join("test_weights.json");

        let manifest = WeightManifest {
            schema_version: "1.1.0".to_string(),
            schema_hash: crate::rl::observation::SCHEMA_HASH.to_string(),
            layer1: LayerWeights {
                weights: vec![0.1; 270 * 256],
                biases: vec![0.0; 256],
            },
            layer2: LayerWeights {
                weights: vec![0.1; 256 * 128],
                biases: vec![0.0; 128],
            },
            layer3: LayerWeights {
                weights: vec![0.1; 128 * 52],
                biases: vec![0.0; 52],
            },
        };

        // Write to file
        let json = serde_json::to_string(&manifest).unwrap();
        {
            let mut file = std::fs::File::create(&weight_path).unwrap();
            file.write_all(json.as_bytes()).unwrap();
        }

        // Load policy from file
        let policy_result = EmbeddedPolicy::from_file(&weight_path);
        assert!(policy_result.is_ok());

        // Clean up
        let _ = std::fs::remove_file(&weight_path);
    }

    #[test]
    fn custom_weights_validation_fails_on_mismatch() {
        use crate::weights::loader::{LayerWeights, WeightManifest};
        use std::io::Write;

        let temp_dir = std::env::temp_dir();
        let weight_path = temp_dir.join("test_weights_bad.json");

        let manifest = WeightManifest {
            schema_version: "0.0.0".to_string(), // Wrong version
            schema_hash: "wrong_hash".to_string(),
            layer1: LayerWeights {
                weights: vec![0.1; 270 * 256],
                biases: vec![0.0; 256],
            },
            layer2: LayerWeights {
                weights: vec![0.1; 256 * 128],
                biases: vec![0.0; 128],
            },
            layer3: LayerWeights {
                weights: vec![0.1; 128 * 52],
                biases: vec![0.0; 52],
            },
        };

        let json = serde_json::to_string(&manifest).unwrap();
        {
            let mut file = std::fs::File::create(&weight_path).unwrap();
            file.write_all(json.as_bytes()).unwrap();
        }

        let policy_result = EmbeddedPolicy::from_file(&weight_path);
        assert!(policy_result.is_err());

        // Clean up
        let _ = std::fs::remove_file(&weight_path);
    }

    #[test]
    fn softmax_sums_to_one() {
        let mut logits = [1.0f32; 52];
        for (i, logit) in logits.iter_mut().enumerate().take(5) {
            *logit = (i + 1) as f32;
        }
        let probs = softmax(&logits);

        let sum: f32 = probs.iter().sum();
        assert!((sum - 1.0).abs() < 1e-6);
    }

    #[test]
    fn softmax_handles_neg_inf() {
        let mut logits = [f32::NEG_INFINITY; 52];
        logits[0] = 1.0;
        logits[1] = 2.0;

        let probs = softmax(&logits);

        // Only indices 0 and 1 should have non-zero probability
        assert!(probs[0] > 0.0);
        assert!(probs[1] > 0.0);
        assert_eq!(probs[2], 0.0);

        // Sum should still be 1.0
        let sum: f32 = probs.iter().sum();
        assert!((sum - 1.0).abs() < 1e-6);
    }

    #[test]
    fn categorical_sampling_respects_probabilities() {
        // Create a distribution where only index 10 has probability
        let mut probs = [0.0f32; 52];
        probs[10] = 1.0;

        // Sample multiple times - should always get index 10
        for _ in 0..10 {
            let sample = sample_categorical(&probs);
            assert_eq!(sample, 10);
        }
    }

    #[test]
    fn forward_with_critic_returns_valid_card() {
        use hearts_core::model::round::RoundPhase;

        let mut policy = EmbeddedPolicy::new();

        let seat = PlayerPosition::South;
        let hand_cards = vec![
            Card::new(Rank::Two, Suit::Clubs),
            Card::new(Rank::Three, Suit::Clubs),
            Card::new(Rank::Four, Suit::Diamonds),
        ];

        let mut hands = [Hand::new(), Hand::new(), Hand::new(), Hand::new()];
        hands[seat.index()] = Hand::with_cards(hand_cards.clone());

        for other_seat in PlayerPosition::LOOP {
            if other_seat != seat {
                hands[other_seat.index()] = Hand::with_cards(vec![
                    Card::new(Rank::Five, Suit::Hearts),
                    Card::new(Rank::Six, Suit::Hearts),
                ]);
            }
        }

        let round = hearts_core::model::round::RoundState::from_hands(
            hands,
            seat,
            PassingDirection::Hold,
            RoundPhase::Playing,
        );

        let hand = round.hand(seat);
        let tracker = UnseenTracker::new();
        let scores = ScoreBoard::new();

        let ctx = PolicyContext {
            hand,
            round: &round,
            scores: &scores,
            seat,
            tracker: &tracker,
            passing_direction: PassingDirection::Hold,
        };

        let (card, value, log_prob) = policy.forward_with_critic(&ctx);

        // Verify card is legal
        let mut probe = round.clone();
        assert!(probe.play_card(seat, card).is_ok());

        // Verify value and log_prob are finite
        assert!(value.is_finite());
        assert!(log_prob.is_finite());
        assert!(log_prob <= 0.0); // log probability should be non-positive
    }
}
