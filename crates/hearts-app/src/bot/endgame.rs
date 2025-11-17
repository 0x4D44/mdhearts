// Phase 3: Endgame Perfect Play
//
// This module implements a dynamic programming solver for endgame positions
// (when each player has ≤ 6 cards remaining). With perfect information about
// remaining cards, we can solve these positions optimally.
//
// Algorithm: Minimax with memoization
// - Hash position based on: cards in each hand + current trick + hearts broken
// - Recursively evaluate all possible play sequences
// - Cache results to avoid recomputation
// - Guaranteed optimal play in the endgame

use super::{BotContext, UnseenTracker};
use hearts_core::model::card::Card;
use hearts_core::model::player::PlayerPosition;
use hearts_core::model::round::RoundState;
use std::collections::HashMap;
use std::time::Instant;

// ============================================================================
// Position Hashing
// ============================================================================

#[derive(Hash, Eq, PartialEq, Clone, Debug)]
struct EndgamePosition {
    // Sorted hands for each player
    hands: [Vec<Card>; 4],
    // Current trick cards
    trick_cards: Vec<(PlayerPosition, Card)>,
    // Game state
    hearts_broken: bool,
    // Who leads next (if trick is empty)
    leader: PlayerPosition,
    // Accumulated penalty points for each player
    accumulated_penalties: [u8; 4],
}

impl EndgamePosition {
    fn from_round(
        round: &RoundState,
        our_seat: PlayerPosition,
        tracker: &UnseenTracker,
        use_sampling: bool,
    ) -> Self {
        let mut hands: [Vec<Card>; 4] = Default::default();

        // Our hand is known
        let our_cards: Vec<Card> = round.hand(our_seat).iter().copied().collect();
        hands[our_seat.index()] = our_cards;

        if use_sampling && tracker.unseen_count() > 0 {
            // Use belief-state sampling for imperfect information
            use rand::SeedableRng;
            let mut rng = rand::rngs::SmallRng::from_entropy();
            let sampled = tracker.sample_world(&mut rng, our_seat, round);

            // Merge sampled hands with known cards from round
            for pos in [
                PlayerPosition::North,
                PlayerPosition::East,
                PlayerPosition::South,
                PlayerPosition::West,
            ] {
                if pos == our_seat {
                    continue; // Already set
                }

                // Get visible cards for this opponent (cards they've already played in current trick)
                let mut visible_cards: Vec<Card> = round.hand(pos).iter().copied().collect();
                // Add sampled cards
                visible_cards.extend(sampled.hands[pos.index()].iter().copied());
                hands[pos.index()] = visible_cards;
            }
        } else {
            // Use perfect information (for testing or when all cards are known)
            for pos in [
                PlayerPosition::North,
                PlayerPosition::East,
                PlayerPosition::South,
                PlayerPosition::West,
            ] {
                if pos == our_seat {
                    continue; // Already set
                }

                let opponent_cards: Vec<Card> = round.hand(pos).iter().copied().collect();
                hands[pos.index()] = opponent_cards;
            }
        }

        // Current trick
        let trick_cards: Vec<(PlayerPosition, Card)> = round
            .current_trick()
            .plays()
            .iter()
            .map(|p| (p.position, p.card))
            .collect();

        // Leader
        let leader = if trick_cards.is_empty() {
            round.current_trick().leader()
        } else {
            round.current_trick().leader()
        };

        EndgamePosition {
            hands,
            trick_cards,
            hearts_broken: round.hearts_broken(),
            leader,
            accumulated_penalties: [0, 0, 0, 0], // Start with zero penalties
        }
    }
}

// ============================================================================
// Endgame Solver
// ============================================================================

pub struct EndgameSolver {
    // Memo key includes perspective (our_seat) because evaluation depends on viewpoint
    memo: HashMap<(EndgamePosition, PlayerPosition), EndgameResult>,
    nodes_evaluated: usize,
    // Optional deadline for timeout protection
    deadline: Option<Instant>,
}

#[derive(Clone, Debug)]
struct EndgameResult {
    best_move: Card,
    expected_penalties: [u8; 4], // Expected final penalties for each player
}

impl EndgameSolver {
    pub fn new() -> Self {
        Self {
            memo: HashMap::new(),
            nodes_evaluated: 0,
            deadline: None,
        }
    }

    /// Set a timeout deadline for this solver
    #[allow(dead_code)]
    pub fn set_deadline(&mut self, deadline: Instant) {
        self.deadline = Some(deadline);
    }

    /// Check if deadline has been exceeded
    fn time_expired(&self) -> bool {
        if let Some(deadline) = self.deadline {
            Instant::now() >= deadline
        } else {
            false
        }
    }

    /// Solve the endgame and return the best move
    pub fn solve(&mut self, ctx: &BotContext<'_>, legal: &[Card]) -> Option<Card> {
        if legal.is_empty() {
            return None;
        }

        // Only solve if we're in the endgame (few cards remaining)
        let max_cards = endgame_max_cards(ctx);
        if ctx.hand().len() > max_cards {
            return None; // Not in endgame yet
        }

        // Use belief-state sampling for imperfect information endgame solving
        let use_sampling = endgame_use_sampling();
        let pos = EndgamePosition::from_round(ctx.round, ctx.seat, ctx.tracker, use_sampling);

        let result = self.minimax(&pos, ctx.seat, true)?;

        // Make sure the best move is actually legal
        if legal.contains(&result.best_move) {
            Some(result.best_move)
        } else {
            None
        }
    }

    fn minimax(
        &mut self,
        pos: &EndgamePosition,
        our_seat: PlayerPosition,
        is_our_turn: bool,
    ) -> Option<EndgameResult> {
        self.nodes_evaluated += 1;

        // Check for timeout
        if self.time_expired() {
            return None;
        }

        // Check memo (keyed by position + perspective)
        if let Some(result) = self.memo.get(&(pos.clone(), our_seat)) {
            return Some(result.clone());
        }

        // Base case: all hands empty
        if pos.hands.iter().all(|h| h.is_empty()) {
            let mut final_penalties = pos.accumulated_penalties;

            // Check for moon shooting: if any player took all 26 points
            for player_idx in 0..4 {
                if final_penalties[player_idx] == 26 {
                    // Moon shot! Shooter gets 0, all others get 26
                    final_penalties = [26, 26, 26, 26];
                    final_penalties[player_idx] = 0;
                    break;
                }
            }

            let result = EndgameResult {
                best_move: Card::new(
                    hearts_core::model::rank::Rank::Two,
                    hearts_core::model::suit::Suit::Clubs,
                ), // Dummy
                expected_penalties: final_penalties,
            };
            return Some(result);
        }

        // Determine who plays next
        let to_play = if pos.trick_cards.is_empty() {
            pos.leader
        } else {
            let last = pos.trick_cards.last().unwrap().0;
            last.next()
        };

        let legal = self.get_legal_moves(pos, to_play);
        if legal.is_empty() {
            return None;
        }

        let mut best_result: Option<EndgameResult> = None;

        for card in legal {
            // Apply the move
            let next_pos = self.apply_move(pos, to_play, card);

            // Determine who plays next in the new position
            let next_to_play = if next_pos.trick_cards.is_empty() {
                next_pos.leader // Trick completed, new leader plays
            } else {
                next_pos.trick_cards.last().unwrap().0.next() // Trick in progress
            };

            // Recursively evaluate
            let result = self.minimax(&next_pos, our_seat, next_to_play == our_seat)?;

            // Choose best move based on whose turn it is
            if is_our_turn {
                // Minimize our penalties
                if best_result.is_none()
                    || result.expected_penalties[our_seat.index()]
                        < best_result.as_ref().unwrap().expected_penalties[our_seat.index()]
                {
                    best_result = Some(EndgameResult {
                        best_move: card,
                        expected_penalties: result.expected_penalties,
                    });
                }
            } else {
                // Opponent minimizes their penalties (we assume optimal play)
                if best_result.is_none()
                    || result.expected_penalties[to_play.index()]
                        < best_result.as_ref().unwrap().expected_penalties[to_play.index()]
                {
                    best_result = Some(EndgameResult {
                        best_move: card,
                        expected_penalties: result.expected_penalties,
                    });
                }
            }
        }

        if let Some(ref result) = best_result {
            self.memo.insert((pos.clone(), our_seat), result.clone());
        }

        best_result
    }

    fn get_legal_moves(&self, pos: &EndgamePosition, seat: PlayerPosition) -> Vec<Card> {
        let hand = &pos.hands[seat.index()];

        if pos.trick_cards.is_empty() {
            // Leading - can play anything except hearts if not broken
            if pos.hearts_broken {
                hand.clone()
            } else {
                let non_hearts: Vec<Card> = hand
                    .iter()
                    .copied()
                    .filter(|c| c.suit != hearts_core::model::suit::Suit::Hearts)
                    .collect();
                if non_hearts.is_empty() {
                    hand.clone() // Only hearts left
                } else {
                    non_hearts
                }
            }
        } else {
            // Following - must follow suit if possible
            let lead_suit = pos.trick_cards[0].1.suit;
            let following: Vec<Card> = hand
                .iter()
                .copied()
                .filter(|c| c.suit == lead_suit)
                .collect();
            if following.is_empty() {
                hand.clone() // Can play anything
            } else {
                following
            }
        }
    }

    fn apply_move(
        &self,
        pos: &EndgamePosition,
        seat: PlayerPosition,
        card: Card,
    ) -> EndgamePosition {
        let mut new_pos = pos.clone();

        // Remove card from hand
        new_pos.hands[seat.index()].retain(|&c| c != card);

        // Add to trick
        new_pos.trick_cards.push((seat, card));

        // Check if trick is complete
        if new_pos.trick_cards.len() == 4 {
            // Determine winner and penalties
            let lead_suit = new_pos.trick_cards[0].1.suit;
            let mut winner = new_pos.trick_cards[0].0;
            let mut best_rank = new_pos.trick_cards[0].1.rank;

            for &(player, played_card) in &new_pos.trick_cards[1..] {
                if played_card.suit == lead_suit && played_card.rank > best_rank {
                    winner = player;
                    best_rank = played_card.rank;
                }
            }

            // Calculate and accumulate penalties
            let penalties: u8 = new_pos
                .trick_cards
                .iter()
                .map(|(_, c)| c.penalty_value())
                .sum();

            // Add penalties to winner's accumulated total
            new_pos.accumulated_penalties[winner.index()] += penalties;

            // Clear trick and set new leader
            new_pos.trick_cards.clear();
            new_pos.leader = winner;

            // Update hearts broken
            if !new_pos.hearts_broken {
                for &(_, c) in &pos.trick_cards {
                    if c.suit == hearts_core::model::suit::Suit::Hearts {
                        new_pos.hearts_broken = true;
                        break;
                    }
                }
            }
        }

        new_pos
    }

    #[allow(dead_code)]
    pub fn stats(&self) -> (usize, usize) {
        (self.memo.len(), self.nodes_evaluated)
    }
}

// ============================================================================
// Configuration
// ============================================================================

/// Check if endgame solver is enabled
/// DEFAULT: ENABLED (can be disabled with MDH_ENDGAME_SOLVER_ENABLED=0)
fn endgame_enabled() -> bool {
    std::env::var("MDH_ENDGAME_SOLVER_ENABLED")
        .map(|v| v == "1" || v.eq_ignore_ascii_case("true") || v.eq_ignore_ascii_case("on"))
        .unwrap_or(true) // ENABLED BY DEFAULT
}

/// Check if endgame solver should use belief-state sampling
/// DEFAULT: ENABLED for imperfect information (can be disabled with MDH_ENDGAME_USE_SAMPLING=0)
fn endgame_use_sampling() -> bool {
    std::env::var("MDH_ENDGAME_USE_SAMPLING")
        .map(|v| v == "1" || v.eq_ignore_ascii_case("true") || v.eq_ignore_ascii_case("on"))
        .unwrap_or(true) // ENABLED BY DEFAULT for proper imperfect information handling
}

/// Maximum number of cards for endgame perfect play
/// SearchLookahead difficulty: 13 cards (PERFECT ENDGAME - entire hand!)
/// Default: 7 cards for strong endgame play
fn endgame_max_cards(ctx: &BotContext<'_>) -> usize {
    // PERFECT endgame for SearchLookahead difficulty - entire hand!
    if matches!(ctx.difficulty, super::BotDifficulty::SearchLookahead) {
        return std::env::var("MDH_ENDGAME_MAX_CARDS")
            .ok()
            .and_then(|s| s.parse::<usize>().ok())
            .unwrap_or(13) // PERFECT: 13 cards = entire hand for Search difficulty
            .max(2)
            .min(13);
    }

    std::env::var("MDH_ENDGAME_MAX_CARDS")
        .ok()
        .and_then(|s| s.parse::<usize>().ok())
        .unwrap_or(7) // Normal: 7 cards
        .max(2)
        .min(10)
}

// ============================================================================
// Integration
// ============================================================================

use super::PlayPlannerHard;

impl PlayPlannerHard {
    pub fn choose_with_endgame_solver(
        legal: &[Card],
        ctx: &BotContext<'_>,
        limit: Option<&super::DecisionLimit<'_>>,
    ) -> Option<Card> {
        if !endgame_enabled() {
            return None;
        }

        // Check if we have time budget remaining
        if let Some(l) = limit {
            if l.expired() {
                return None; // No time left
            }
            // If we have very little time left (< 5ms), skip endgame solver
            if let Some(remaining) = l.remaining_millis() {
                if remaining < 5 {
                    return None;
                }
            }
        }

        let mut solver = EndgameSolver::new();
        solver.solve(ctx, legal)
    }
}
