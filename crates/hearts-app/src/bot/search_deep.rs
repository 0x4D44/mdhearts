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

    pub fn store(
        &mut self,
        hash: u64,
        depth: u8,
        score: i32,
        best_move: Option<Card>,
        node_type: NodeType,
    ) {
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

    // Hash the player-to-move (critical for correct transposition table lookups)
    let next_to_play = round
        .current_trick()
        .plays()
        .last()
        .map(|play| play.position.next())
        .unwrap_or_else(|| round.current_trick().leader());
    next_to_play.hash(&mut hasher);

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
    killer_moves: Vec<Option<Card>>, // Killer moves indexed by depth
}

#[allow(dead_code)]
impl DeepSearch {
    pub fn new(tt_size: usize, time_budget_ms: u32) -> Self {
        Self {
            tt: TranspositionTable::new(tt_size),
            nodes_searched: 0,
            time_budget: std::time::Duration::from_millis(time_budget_ms as u64),
            start_time: Instant::now(),
            killer_moves: vec![None; 20], // Support up to 20 plies
        }
    }

    /// Main entry point: iterative deepening search
    pub fn choose_best_move(&mut self, legal: &[Card], ctx: &BotContext<'_>) -> SearchResult {
        if legal.is_empty() {
            // Return graceful error instead of panic - this should never happen in valid game state
            eprintln!("WARNING: choose_best_move called with empty legal moves");
            return SearchResult {
                best_move: Card::new(
                    hearts_core::model::rank::Rank::Two,
                    hearts_core::model::suit::Suit::Clubs,
                ),
                score: 0,
                nodes_searched: 0,
                depth_reached: 0,
            };
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
        self.killer_moves = vec![None; 20];

        // Get difficulty-dependent max depth
        let max_depth = deep_search_max_depth(ctx);
        let mut best_result = None;
        let mut prev_score: i32 = 0;

        // Iterative deepening with aspiration windows: start at depth 1, increase until time runs out
        for depth in 1..=max_depth {
            if self.time_expired() {
                break;
            }

            // Aspiration window: search with narrow window first for speed
            let window = if depth > 2 { 50 } else { i32::MAX / 2 };
            // Use saturating arithmetic to prevent overflow
            let alpha = prev_score.saturating_sub(window);
            let beta = prev_score.saturating_add(window);

            // Try search with aspiration window
            let result = if depth > 2 {
                // Try narrow window first
                match self.search_root_windowed(legal, ctx, depth, alpha, beta) {
                    Some(r) if r.score > alpha && r.score < beta => Some(r),
                    _ => {
                        // Re-search with full window if we fell outside
                        self.search_root(legal, ctx, depth)
                    }
                }
            } else {
                self.search_root(legal, ctx, depth)
            };

            match result {
                Some(result) => {
                    prev_score = result.score;
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

    fn search_root_windowed(
        &mut self,
        legal: &[Card],
        ctx: &BotContext<'_>,
        depth: u8,
        mut alpha: i32,
        beta: i32,
    ) -> Option<SearchResult> {
        let mut best_move = legal[0];
        let mut best_score = i32::MIN;

        // CRITICAL: Order moves by heuristic for better alpha-beta pruning
        let ordered_moves = self.order_moves_with_killer(legal, ctx, depth);

        // Try each legal move in order of promise
        for card in ordered_moves {
            if self.time_expired() {
                return None;
            }

            let score = self.search_move(card, ctx, depth - 1, alpha, beta)?;

            if score > best_score {
                best_score = score;
                best_move = card;
            }

            alpha = alpha.max(score);
            if alpha >= beta {
                // Store killer move
                let depth_idx = depth as usize;
                if depth_idx < self.killer_moves.len() {
                    self.killer_moves[depth_idx] = Some(card);
                }
                break;
            }
        }

        Some(SearchResult {
            best_move,
            score: best_score,
            nodes_searched: self.nodes_searched,
            depth_reached: depth,
        })
    }

    fn search_root(
        &mut self,
        legal: &[Card],
        ctx: &BotContext<'_>,
        depth: u8,
    ) -> Option<SearchResult> {
        let mut best_move = legal[0];
        let mut best_score = i32::MIN;
        // Use safe bounds to prevent overflow when combined with penalty deltas
        let mut alpha = -100_000;
        let beta = 100_000;

        // CRITICAL: Order moves by heuristic + killer moves for better alpha-beta pruning
        let ordered_moves = self.order_moves_with_killer(legal, ctx, depth);

        // Try each legal move in order of promise
        for card in ordered_moves {
            if self.time_expired() {
                return None;
            }

            let score = self.search_move(card, ctx, depth - 1, alpha, beta)?;

            if score > best_score {
                best_score = score;
                best_move = card;
            }

            alpha = alpha.max(score);
            if alpha >= beta {
                // Store killer move
                let depth_idx = depth as usize;
                if depth_idx < self.killer_moves.len() {
                    self.killer_moves[depth_idx] = Some(card);
                }
                break;
            }
        }

        Some(SearchResult {
            best_move,
            score: best_score,
            nodes_searched: self.nodes_searched,
            depth_reached: depth,
        })
    }

    /// Order moves by killer move + heuristic evaluation (best first) for better alpha-beta pruning
    fn order_moves_with_killer(
        &self,
        legal: &[Card],
        ctx: &BotContext<'_>,
        depth: u8,
    ) -> Vec<Card> {
        let mut evaluated = PlayPlanner::explain_candidates(legal, ctx);

        // Boost killer move score massively to try it first
        if let Some(killer) = self.killer_moves.get(depth as usize).and_then(|k| *k) {
            for (card, score) in &mut evaluated {
                if *card == killer {
                    *score += 1_000_000; // Massive boost to try killer first
                    break;
                }
            }
        }

        // Sort by score descending (killer move will be first if present)
        evaluated.sort_by(|a, b| b.1.cmp(&a.1));
        evaluated.into_iter().map(|(card, _)| card).collect()
    }

    /// Order moves by heuristic evaluation (best first) for better alpha-beta pruning
    fn order_moves(&self, legal: &[Card], ctx: &BotContext<'_>) -> Vec<Card> {
        let mut evaluated = PlayPlanner::explain_candidates(legal, ctx);
        // Sort by score descending (best moves first)
        evaluated.sort_by(|a, b| b.1.cmp(&a.1));
        evaluated.into_iter().map(|(card, _)| card).collect()
    }

    /// Order moves for opponent simulation (simple heuristic)
    fn order_moves_simple(
        &self,
        legal: &[Card],
        _round: &RoundState,
        _seat: PlayerPosition,
    ) -> Vec<Card> {
        let mut moves: Vec<_> = legal.iter().copied().collect();

        // Simple ordering: prefer low cards to avoid taking penalties
        // This is a rough heuristic for opponent play
        moves.sort_by_key(|card| {
            let rank_val = match card.rank {
                hearts_core::model::rank::Rank::Two => 0,
                hearts_core::model::rank::Rank::Three => 1,
                hearts_core::model::rank::Rank::Four => 2,
                hearts_core::model::rank::Rank::Five => 3,
                hearts_core::model::rank::Rank::Six => 4,
                hearts_core::model::rank::Rank::Seven => 5,
                hearts_core::model::rank::Rank::Eight => 6,
                hearts_core::model::rank::Rank::Nine => 7,
                hearts_core::model::rank::Rank::Ten => 8,
                hearts_core::model::rank::Rank::Jack => 9,
                hearts_core::model::rank::Rank::Queen => 10,
                hearts_core::model::rank::Rank::King => 11,
                hearts_core::model::rank::Rank::Ace => 12,
            };

            // Prefer clubs, then diamonds, then spades, then hearts (to avoid penalties)
            let suit_val = match card.suit {
                hearts_core::model::suit::Suit::Clubs => 0,
                hearts_core::model::suit::Suit::Diamonds => 1,
                hearts_core::model::suit::Suit::Spades => 2,
                hearts_core::model::suit::Suit::Hearts => 3,
            };

            (suit_val, rank_val)
        });

        moves
    }

    fn search_move(
        &mut self,
        card: Card,
        ctx: &BotContext<'_>,
        depth: u8,
        alpha: i32,
        beta: i32,
    ) -> Option<i32> {
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
                Some(
                    penalty_delta
                        + self.search_next_trick(&round, ctx.seat, depth - 1, alpha, beta)?,
                )
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
    fn search_opponent(
        &mut self,
        round: &RoundState,
        depth: u8,
        mut alpha: i32,
        beta: i32,
    ) -> Option<i32> {
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

        // Use move ordering for better pruning
        let ordered = self.order_moves_simple(&legal, round, next);

        for card in &ordered {
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
                        // Continue to next trick with recursive search
                        penalty_delta + self.search_opponent(&probe, depth - 1, alpha, beta)?
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

    fn search_next_trick(
        &mut self,
        round: &RoundState,
        seat: PlayerPosition,
        depth: u8,
        alpha: i32,
        beta: i32,
    ) -> Option<i32> {
        if depth == 0 {
            return Some(self.evaluate_position(round, seat));
        }

        // Determine who leads the next trick
        let next_leader = next_to_play(round);

        if next_leader == seat {
            // We lead the next trick - search our options
            let legal = legal_moves_for(round, seat);
            if legal.is_empty() {
                return Some(0);
            }

            let mut best_score = i32::MIN;
            let mut local_alpha = alpha;

            // Use simple move ordering (we don't have BotContext here)
            let ordered = self.order_moves_simple(&legal, round, seat);

            for &card in &ordered {
                if self.time_expired() {
                    return None;
                }

                // Recursively search this move
                let score =
                    self.search_position_after_move(round, seat, card, depth, local_alpha, beta)?;
                best_score = best_score.max(score);
                local_alpha = local_alpha.max(score);

                if local_alpha >= beta {
                    break; // Beta cutoff
                }
            }

            Some(best_score)
        } else {
            // Opponent leads - simulate their play
            self.search_opponent(round, depth, alpha, beta)
        }
    }

    fn search_position_after_move(
        &mut self,
        round: &RoundState,
        seat: PlayerPosition,
        card: Card,
        depth: u8,
        alpha: i32,
        beta: i32,
    ) -> Option<i32> {
        self.nodes_searched += 1;

        if self.time_expired() {
            return None;
        }

        // Apply the move
        let mut new_round = round.clone();
        let outcome = match new_round.play_card(seat, card) {
            Ok(o) => o,
            Err(_) => return Some(i32::MIN), // Illegal move
        };

        // Check transposition table for cached result
        let hash = hash_position(&new_round, seat);
        if let Some(score) = self.tt.probe(hash, depth, alpha, beta) {
            return Some(score);
        }

        let result = match outcome {
            PlayOutcome::TrickCompleted { winner, penalties } => {
                let penalty_delta = if winner == seat {
                    -(penalties as i32) * 100
                } else {
                    0
                };

                if new_round.hand(seat).is_empty() || depth == 0 {
                    penalty_delta + self.evaluate_position(&new_round, seat)
                } else {
                    // Continue to next trick
                    penalty_delta
                        + self.search_next_trick(&new_round, seat, depth - 1, alpha, beta)?
                }
            }
            _ => {
                // Trick in progress - simulate opponent moves
                if depth == 0 {
                    self.evaluate_position(&new_round, seat)
                } else {
                    self.search_opponent(&new_round, depth - 1, -beta, -alpha)
                        .map(|score| -score)?
                }
            }
        };

        // Store result in transposition table
        let node_type = if result <= alpha {
            NodeType::UpperBound
        } else if result >= beta {
            NodeType::LowerBound
        } else {
            NodeType::Exact
        };
        self.tt.store(hash, depth, result, Some(card), node_type);

        Some(result)
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
        evaluated.iter().map(|(_, score)| *score).max().unwrap_or(0)
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
/// SearchLookahead difficulty: 10 plies (MAXIMUM DEPTH - near perfect play!)
/// Default: 3 plies for strong tactical play
fn deep_search_max_depth(ctx: &BotContext<'_>) -> u8 {
    // MAXIMUM depth for SearchLookahead difficulty
    if matches!(ctx.difficulty, super::BotDifficulty::SearchLookahead) {
        return std::env::var("MDH_SEARCH_MAX_DEPTH")
            .ok()
            .and_then(|s| s.parse::<u8>().ok())
            .unwrap_or(10) // MAXIMUM: 10 plies for Search difficulty
            .max(1)
            .min(13); // Can search whole hand
    }

    std::env::var("MDH_SEARCH_MAX_DEPTH")
        .ok()
        .and_then(|s| s.parse::<u8>().ok())
        .unwrap_or(3) // Normal: 3 plies
        .max(1)
        .min(6)
}

/// Transposition table size (number of positions to cache)
/// SearchLookahead difficulty: 10M positions (MASSIVE cache)
/// Default: 500k for good performance
fn deep_search_tt_size(ctx: &BotContext<'_>) -> usize {
    // MASSIVE cache for SearchLookahead difficulty
    if matches!(ctx.difficulty, super::BotDifficulty::SearchLookahead) {
        return std::env::var("MDH_SEARCH_TT_SIZE")
            .ok()
            .and_then(|s| s.parse::<usize>().ok())
            .unwrap_or(10_000_000) // MAXIMUM: 10M positions for Search difficulty
            .max(1000)
            .min(20_000_000);
    }

    std::env::var("MDH_SEARCH_TT_SIZE")
        .ok()
        .and_then(|s| s.parse::<usize>().ok())
        .unwrap_or(500_000) // Normal: 500k
        .max(1000)
        .min(10_000_000)
}

/// Time budget per move in milliseconds
/// SearchLookahead difficulty: 2000ms (think EXTREMELY deeply)
/// Default: 100ms for strong but responsive play
fn deep_search_time_ms(ctx: &BotContext<'_>) -> u32 {
    // VERY long thinking for SearchLookahead difficulty
    if matches!(ctx.difficulty, super::BotDifficulty::SearchLookahead) {
        return std::env::var("MDH_SEARCH_TIME_MS")
            .ok()
            .and_then(|s| s.parse::<u32>().ok())
            .unwrap_or(2000) // MAXIMUM: 2000ms for Search difficulty
            .max(10)
            .min(60000); // Up to 60 seconds
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
    pub fn choose_with_deep_search(
        legal: &[Card],
        ctx: &BotContext<'_>,
        limit: Option<&super::DecisionLimit<'_>>,
    ) -> Option<Card> {
        if !deep_search_enabled() {
            return None; // Fall back to existing search
        }

        // Get difficulty-dependent parameters
        let tt_size = deep_search_tt_size(ctx);

        // Use UI time limit if available, otherwise use difficulty-based default
        let time_ms = if let Some(remaining) = limit.and_then(|l| l.remaining_millis()) {
            // Use the actual remaining time from UI setting
            remaining.max(10) // At least 10ms
        } else {
            // Fallback to difficulty-based default
            deep_search_time_ms(ctx)
        };

        let mut search = DeepSearch::new(tt_size, time_ms);
        let result = search.choose_best_move(legal, ctx);

        Some(result.best_move)
    }
}
