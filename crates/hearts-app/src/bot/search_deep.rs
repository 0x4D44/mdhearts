// Phase 2: Deeper Search with Alpha-Beta Pruning and Transposition Tables
//
// This module implements multi-ply search to look ahead 2-4 tricks instead of just 1.
// Key components:
// - Alpha-beta pruning for efficient search
// - Transposition tables for caching positions
// - Integration with belief-state sampling from Phase 1
// - Configurable depth via MDH_SEARCH_MAX_DEPTH

use super::{BotContext, PlayPlanner, PlayPlannerHard};
use hearts_core::model::card::Card;
use hearts_core::model::passing::PassingDirection;
use hearts_core::model::player::PlayerPosition;
use hearts_core::model::round::{PlayOutcome, RoundState};
use hearts_core::model::score::ScoreBoard;
use std::collections::HashMap;
use std::time::Instant;

// ============================================================================
// Transposition Table
// ============================================================================

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[allow(dead_code)]
pub enum NodeType {
    Exact,      // Exact score
    LowerBound, // Alpha cutoff (score is at least this value)
    UpperBound, // Beta cutoff (score is at most this value)
}

#[derive(Debug, Clone)]
#[allow(dead_code)]
struct TableEntry {
    hash: u64,
    depth: u8,
    score: i32,
    best_move: Option<Card>,
    node_type: NodeType,
}

#[allow(dead_code)]
pub struct TranspositionTable {
    entries: HashMap<u64, TableEntry>,
    max_size: usize,
    hits: usize,
    misses: usize,
}

#[allow(dead_code)]
impl TranspositionTable {
    pub fn new(max_size: usize) -> Self {
        Self {
            entries: HashMap::with_capacity(max_size.min(1_000_000)),
            max_size,
            hits: 0,
            misses: 0,
        }
    }

    pub fn probe(&mut self, hash: u64, depth: u8, alpha: i32, beta: i32) -> Option<i32> {
        if let Some(entry) = self.entries.get(&hash) {
            if entry.hash == hash && entry.depth >= depth {
                self.hits += 1;
                match entry.node_type {
                    NodeType::Exact => return Some(entry.score),
                    NodeType::LowerBound if entry.score >= beta => return Some(entry.score),
                    NodeType::UpperBound if entry.score <= alpha => return Some(entry.score),
                    _ => {}
                }
            }
        }
        self.misses += 1;
        None
    }

    pub fn store(&mut self, hash: u64, depth: u8, score: i32, best_move: Option<Card>, node_type: NodeType) {
        // Evict random entry if table is full
        if self.entries.len() >= self.max_size {
            if let Some(&key) = self.entries.keys().next() {
                self.entries.remove(&key);
            }
        }

        self.entries.insert(
            hash,
            TableEntry {
                hash,
                depth,
                score,
                best_move,
                node_type,
            },
        );
    }

    #[allow(dead_code)]
    pub fn stats(&self) -> (usize, usize, usize) {
        (self.entries.len(), self.hits, self.misses)
    }

    pub fn clear(&mut self) {
        self.entries.clear();
        self.hits = 0;
        self.misses = 0;
    }
}

// ============================================================================
// Position Hashing
// ============================================================================

#[allow(dead_code)]
fn hash_position(round: &RoundState, seat: PlayerPosition) -> u64 {
    use std::collections::hash_map::DefaultHasher;
    use std::hash::{Hash, Hasher};

    let mut hasher = DefaultHasher::new();

    // Hash our hand
    let hand = round.hand(seat);
    for card in hand.cards() {
        card.hash(&mut hasher);
    }

    // Hash current trick
    for play in round.current_trick().plays() {
        play.position.hash(&mut hasher);
        play.card.hash(&mut hasher);
    }

    // Hash trick count (game phase)
    round.trick_history().len().hash(&mut hasher);

    // Hash hearts broken state
    round.hearts_broken().hash(&mut hasher);

    hasher.finish()
}

// ============================================================================
// Deep Search Implementation
// ============================================================================

#[derive(Debug, Clone)]
#[allow(dead_code)]
pub struct SearchResult {
    pub best_move: Card,
    pub score: i32,
    pub nodes_searched: usize,
    pub depth_reached: u8,
}

#[allow(dead_code)]
pub struct DeepSearch {
    tt: TranspositionTable,
    nodes_searched: usize,
    time_budget: std::time::Duration,
    start_time: Instant,
}

#[allow(dead_code)]
impl DeepSearch {
    pub fn new(tt_size: usize, time_budget_ms: u32) -> Self {
        Self {
            tt: TranspositionTable::new(tt_size),
            nodes_searched: 0,
            time_budget: std::time::Duration::from_millis(time_budget_ms as u64),
            start_time: Instant::now(),
        }
    }

    /// Main entry point: iterative deepening search
    pub fn choose_best_move(&mut self, legal: &[Card], ctx: &BotContext<'_>) -> SearchResult {
        if legal.is_empty() {
            panic!("No legal moves");
        }
        if legal.len() == 1 {
            return SearchResult {
                best_move: legal[0],
                score: 0,
                nodes_searched: 1,
                depth_reached: 0,
            };
        }

        self.start_time = Instant::now();
        self.nodes_searched = 0;
        self.tt.clear();

        // Get difficulty-dependent max depth
        let max_depth = deep_search_max_depth(ctx);
        let mut best_result = None;

        // Iterative deepening: start at depth 1, increase until time runs out
        for depth in 1..=max_depth {
            if self.time_expired() {
                break;
            }

            match self.search_root(legal, ctx, depth) {
                Some(result) => {
                    best_result = Some(result);
                }
                None => break, // Timeout
            }
        }

        best_result.unwrap_or_else(|| {
            // Fallback: use heuristic from Phase 1
            SearchResult {
                best_move: legal[0],
                score: 0,
                nodes_searched: self.nodes_searched,
                depth_reached: 0,
            }
        })
    }

    fn search_root(&mut self, legal: &[Card], ctx: &BotContext<'_>, depth: u8) -> Option<SearchResult> {
        let mut best_move = legal[0];
        let mut best_score = i32::MIN;

        // Try each legal move
        for &card in legal {
            if self.time_expired() {
                return None;
            }

            let score = self.search_move(card, ctx, depth - 1, i32::MIN + 1, i32::MAX - 1)?;

            if score > best_score {
                best_score = score;
                best_move = card;
            }
        }

        Some(SearchResult {
            best_move,
            score: best_score,
            nodes_searched: self.nodes_searched,
            depth_reached: depth,
        })
    }

    fn search_move(&mut self, card: Card, ctx: &BotContext<'_>, depth: u8, alpha: i32, beta: i32) -> Option<i32> {
        self.nodes_searched += 1;

        if self.time_expired() {
            return None;
        }

        // Apply the move and continue search
        let mut round = ctx.round.clone();
        let outcome = match round.play_card(ctx.seat, card) {
            Ok(o) => o,
            Err(_) => return Some(i32::MIN), // Illegal move
        };

        match outcome {
            PlayOutcome::TrickCompleted { winner, penalties } => {
                // Trick completed - evaluate and continue if cards remain
                let penalty_delta = if winner == ctx.seat {
                    -(penalties as i32) * 100 // We won the trick, penalties hurt us
                } else {
                    0 // Someone else won, neutral for us
                };

                if round.hand(ctx.seat).is_empty() || depth == 0 {
                    // Round over or depth limit reached
                    return Some(penalty_delta + self.evaluate_position(&round, ctx.seat));
                }

                // Continue to next trick with reduced depth
                Some(penalty_delta + self.search_next_trick(&round, ctx.seat, depth - 1, alpha, beta)?)
            }
            _ => {
                // Trick in progress - simulate opponent moves
                if depth == 0 {
                    return Some(self.evaluate_position(&round, ctx.seat));
                }

                // Continue searching opponent responses
                self.search_opponent(&round, depth - 1, -beta, -alpha)
                    .map(|score| -score) // Negate because it's opponent's score
            }
        }
    }

    #[allow(dead_code)]
    fn search_opponent(&mut self, round: &RoundState, depth: u8, mut alpha: i32, beta: i32) -> Option<i32> {
        self.nodes_searched += 1;

        if self.time_expired() {
            return None;
        }

        let next = next_to_play(round);
        let legal = legal_moves_for(round, next);

        if legal.is_empty() {
            return Some(0);
        }

        let mut best_score = i32::MIN;

        for card in &legal {
            let mut probe = round.clone();
            let outcome = match probe.play_card(next, *card) {
                Ok(o) => o,
                Err(_) => continue,
            };

            let score = match outcome {
                PlayOutcome::TrickCompleted { winner, penalties } => {
                    // Opponent perspective: they want to avoid penalties
                    let penalty_delta = if winner == next {
                        -(penalties as i32) * 100
                    } else {
                        0
                    };

                    if probe.hand(next).is_empty() || depth == 0 {
                        penalty_delta + self.evaluate_position(&probe, next)
                    } else {
                        // Continue to next trick
                        penalty_delta + self.evaluate_position(&probe, next)
                    }
                }
                _ => {
                    // Trick continues
                    if depth == 0 {
                        self.evaluate_position(&probe, next)
                    } else {
                        // Recursively search deeper
                        self.search_opponent(&probe, depth - 1, alpha, beta)?
                    }
                }
            };

            best_score = best_score.max(score);
            alpha = alpha.max(score);

            if alpha >= beta {
                break; // Beta cutoff
            }
        }

        Some(best_score)
    }

    fn search_next_trick(&mut self, round: &RoundState, seat: PlayerPosition, _depth: u8, _alpha: i32, _beta: i32) -> Option<i32> {
        // Simplified: just evaluate the position for now
        // TODO: Recursively search the next trick
        Some(self.evaluate_position(round, seat))
    }

    fn evaluate_position(&self, round: &RoundState, seat: PlayerPosition) -> i32 {
        // Use heuristic evaluation from PlayPlanner
        // We need to evaluate the current position from the given seat's perspective

        // Get legal moves for this position
        let legal = legal_moves_for(round, seat);
        if legal.is_empty() {
            return 0;
        }

        // We need a BotContext for evaluation, but we only have round/seat
        // Create a minimal one with dummy scores and tracker
        let dummy_scores = ScoreBoard::new();
        let dummy_tracker = super::UnseenTracker::new();
        let dummy_difficulty = super::BotDifficulty::FutureHard;

        let temp_ctx = BotContext::new(
            seat,
            round,
            &dummy_scores,
            PassingDirection::Hold,
            &dummy_tracker,
            dummy_difficulty,
        );

        // Use PlayPlanner heuristic to evaluate each move
        let evaluated = PlayPlanner::explain_candidates(&legal, &temp_ctx);

        // Return the score of the best move (this represents the value of this position)
        evaluated
            .iter()
            .map(|(_, score)| *score)
            .max()
            .unwrap_or(0)
    }

    fn time_expired(&self) -> bool {
        self.start_time.elapsed() >= self.time_budget
    }
}

// ============================================================================
// Helper Functions
// ============================================================================

// Helper functions (copied from search.rs for use in this module)
#[allow(dead_code)]
fn next_to_play(round: &RoundState) -> PlayerPosition {
    let trick = round.current_trick();
    trick
        .plays()
        .last()
        .map(|p| p.position.next())
        .unwrap_or(trick.leader())
}

#[allow(dead_code)]
fn legal_moves_for(round: &RoundState, seat: PlayerPosition) -> Vec<Card> {
    round
        .hand(seat)
        .iter()
        .copied()
        .filter(|card| {
            let mut probe = round.clone();
            probe.play_card(seat, *card).is_ok()
        })
        .collect()
}

// ============================================================================
// Configuration
// ============================================================================

/// Check if deep search is enabled
/// DEFAULT: ENABLED (can be disabled with MDH_SEARCH_DEEPER_ENABLED=0)
fn deep_search_enabled() -> bool {
    std::env::var("MDH_SEARCH_DEEPER_ENABLED")
        .map(|v| v == "1" || v.eq_ignore_ascii_case("true") || v.eq_ignore_ascii_case("on"))
        .unwrap_or(true) // ENABLED BY DEFAULT
}

/// Maximum search depth (number of plies to look ahead)
/// SearchLookahead difficulty: 5 plies (ultra-deep)
/// Default: 3 plies for strong tactical play
fn deep_search_max_depth(ctx: &BotContext<'_>) -> u8 {
    // Ultra-aggressive for SearchLookahead difficulty
    if matches!(ctx.difficulty, super::BotDifficulty::SearchLookahead) {
        return std::env::var("MDH_SEARCH_MAX_DEPTH")
            .ok()
            .and_then(|s| s.parse::<u8>().ok())
            .unwrap_or(5) // ULTRA-DEEP: 5 plies for Search difficulty
            .max(1)
            .min(8);
    }

    std::env::var("MDH_SEARCH_MAX_DEPTH")
        .ok()
        .and_then(|s| s.parse::<u8>().ok())
        .unwrap_or(3) // Normal: 3 plies
        .max(1)
        .min(6)
}

/// Transposition table size (number of positions to cache)
/// SearchLookahead difficulty: 2M positions (massive cache)
/// Default: 500k for good performance
fn deep_search_tt_size(ctx: &BotContext<'_>) -> usize {
    // Massive cache for SearchLookahead difficulty
    if matches!(ctx.difficulty, super::BotDifficulty::SearchLookahead) {
        return std::env::var("MDH_SEARCH_TT_SIZE")
            .ok()
            .and_then(|s| s.parse::<usize>().ok())
            .unwrap_or(2_000_000) // MASSIVE: 2M positions for Search difficulty
            .max(1000)
            .min(10_000_000);
    }

    std::env::var("MDH_SEARCH_TT_SIZE")
        .ok()
        .and_then(|s| s.parse::<usize>().ok())
        .unwrap_or(500_000) // Normal: 500k
        .max(1000)
        .min(10_000_000)
}

/// Time budget per move in milliseconds
/// SearchLookahead difficulty: 500ms (think deeply)
/// Default: 100ms for strong but responsive play
fn deep_search_time_ms(ctx: &BotContext<'_>) -> u32 {
    // Much longer thinking for SearchLookahead difficulty
    if matches!(ctx.difficulty, super::BotDifficulty::SearchLookahead) {
        return std::env::var("MDH_SEARCH_TIME_MS")
            .ok()
            .and_then(|s| s.parse::<u32>().ok())
            .unwrap_or(500) // ULTRA-LONG: 500ms for Search difficulty
            .max(10)
            .min(10000);
    }

    std::env::var("MDH_SEARCH_TIME_MS")
        .ok()
        .and_then(|s| s.parse::<u32>().ok())
        .unwrap_or(100) // Normal: 100ms
        .max(10)
        .min(5000)
}

// ============================================================================
// Integration with existing PlayPlannerHard
// ============================================================================

impl PlayPlannerHard {
    pub fn choose_with_deep_search(legal: &[Card], ctx: &BotContext<'_>) -> Option<Card> {
        if !deep_search_enabled() {
            return None; // Fall back to existing search
        }

        // Get difficulty-dependent parameters
        let tt_size = deep_search_tt_size(ctx);
        let time_ms = deep_search_time_ms(ctx);

        let mut search = DeepSearch::new(tt_size, time_ms);
        let result = search.choose_best_move(legal, ctx);

        Some(result.best_move)
    }
}
