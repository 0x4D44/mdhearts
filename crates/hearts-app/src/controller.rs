#![cfg_attr(not(windows), allow(dead_code))]

use crate::bot::MoonState;
use crate::bot::{
    BotContext, BotDifficulty, DecisionLimit, PassPlanner, PlayPlanner, UnseenTracker,
};
use hearts_core::game::match_state::MatchState;
use hearts_core::model::card::Card;
use hearts_core::model::passing::PassingDirection;
use hearts_core::model::player::PlayerPosition;
use hearts_core::model::rank::Rank;
use hearts_core::model::round::{PlayError, PlayOutcome, RoundPhase, RoundState};
use hearts_core::model::score::ScoreBoard;
use hearts_core::model::suit::Suit;
use std::time::{Duration, Instant};
#[cfg(windows)]
use windows::Win32::System::Diagnostics::Debug::OutputDebugStringW;
#[cfg(windows)]
use windows::core::PCWSTR;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TimeoutFallback {
    HeuristicBest,
    FirstLegal,
    SkipAndLog,
}

impl TimeoutFallback {
    pub const fn label(self) -> &'static str {
        match self {
            TimeoutFallback::HeuristicBest => "heuristic_best",
            TimeoutFallback::FirstLegal => "first_legal",
            TimeoutFallback::SkipAndLog => "skip_and_log",
        }
    }

    pub fn from_env_value(raw: &str) -> Option<Self> {
        let trimmed = raw.trim();
        if let Ok(code) = trimmed.parse::<u8>() {
            return match code {
                0 => Some(TimeoutFallback::HeuristicBest),
                1 => Some(TimeoutFallback::FirstLegal),
                2 => Some(TimeoutFallback::SkipAndLog),
                _ => None,
            };
        }
        match trimmed.to_ascii_lowercase().as_str() {
            "heuristic" | "heuristic_best" | "heuristicbest" | "best" => {
                Some(TimeoutFallback::HeuristicBest)
            }
            "first" | "first_legal" | "firstlegal" | "legal" => Some(TimeoutFallback::FirstLegal),
            "skip" | "skip_and_log" | "skipandlog" | "skip-log" => {
                Some(TimeoutFallback::SkipAndLog)
            }
            _ => None,
        }
    }
}

impl Default for TimeoutFallback {
    fn default() -> Self {
        TimeoutFallback::HeuristicBest
    }
}

#[derive(Clone, Copy, Debug)]
pub struct ThinkConfig {
    pub max_duration: Duration,
    pub fallback: TimeoutFallback,
}

impl Default for ThinkConfig {
    fn default() -> Self {
        Self {
            max_duration: Duration::from_secs(10),
            fallback: TimeoutFallback::default(),
        }
    }
}

impl ThinkConfig {
    pub fn from_env() -> Self {
        let mut cfg = Self::default();
        if let Ok(raw_ms) = std::env::var("MDH_THINK_LIMIT_MS") {
            if let Ok(ms) = raw_ms.trim().parse::<u32>() {
                cfg.max_duration = if ms == 0 {
                    Duration::ZERO
                } else {
                    Duration::from_millis(ms as u64)
                };
            }
        }
        if let Ok(raw_fallback) = std::env::var("MDH_THINK_FALLBACK") {
            if let Some(fallback) = TimeoutFallback::from_env_value(&raw_fallback) {
                cfg.fallback = fallback;
            }
        }
        cfg
    }

    pub fn limit_millis(&self) -> Option<u32> {
        if self.max_duration.is_zero() {
            None
        } else {
            Some(self.max_duration.as_millis().min(u32::MAX as u128) as u32)
        }
    }
}

fn test_force_autoplay_timeout() -> bool {
    std::env::var("MDH_TEST_FORCE_AUTOP_TIMEOUT")
        .map(|v| {
            let lower = v.trim().to_ascii_lowercase();
            matches!(lower.as_str(), "1" | "true" | "on")
        })
        .unwrap_or(false)
}

#[derive(Clone)]
pub struct BotSnapshot {
    round: RoundState,
    scores: ScoreBoard,
    passing_direction: PassingDirection,
    tracker: UnseenTracker,
}

impl BotSnapshot {
    pub fn capture(match_state: &MatchState, tracker: &UnseenTracker) -> Self {
        Self {
            round: match_state.round().clone(),
            scores: *match_state.scores(),
            passing_direction: match_state.passing_direction(),
            tracker: tracker.clone(),
        }
    }

    pub fn make_thread_local(mut self) -> Self {
        self.tracker = self.tracker.clone_with_fresh_cache();
        self
    }

    pub fn bot_context(&self, seat: PlayerPosition, difficulty: BotDifficulty) -> BotContext<'_> {
        let (scores, bias_delta) = biased_scores(&self.scores, seat);
        BotContext::new(
            seat,
            &self.round,
            scores,
            self.passing_direction,
            &self.tracker,
            difficulty,
        )
        .with_controller_bias_delta(bias_delta)
    }

    pub fn tracker(&self) -> &UnseenTracker {
        &self.tracker
    }
}

pub struct BotThinkRequest {
    pub seat: PlayerPosition,
    pub legal: Vec<Card>,
    pub enforce_two: bool,
    pub difficulty: BotDifficulty,
    pub snapshot: BotSnapshot,
    pub config: ThinkConfig,
}

#[derive(Debug)]
pub struct BotThinkResult {
    pub seat: PlayerPosition,
    pub chosen: Option<Card>,
    pub elapsed: Duration,
    pub timed_out: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AutoplayOutcome {
    Played(PlayerPosition, Card),
    SkippedTimeout,
    NoLegal,
    NotExpected,
}

pub struct GameController {
    match_state: MatchState,
    last_trick: Option<TrickSummary>,
    bot_difficulty: BotDifficulty,
    unseen_tracker: UnseenTracker,
    think_config: ThinkConfig,
}

impl GameController {
    fn dbg(msg: &str) {
        fn debug_enabled() -> bool {
            static ON: std::sync::OnceLock<bool> = std::sync::OnceLock::new();
            *ON.get_or_init(|| {
                std::env::var("MDH_DEBUG_LOGS")
                    .map(|v| {
                        v == "1" || v.eq_ignore_ascii_case("true") || v.eq_ignore_ascii_case("on")
                    })
                    .unwrap_or(false)
            })
        }
        if !debug_enabled() {
            return;
        }
        #[cfg(windows)]
        {
            let mut wide: Vec<u16> = msg.encode_utf16().collect();
            wide.push(0);
            unsafe {
                OutputDebugStringW(PCWSTR(wide.as_ptr()));
            }
        }
        #[cfg(not(windows))]
        {
            eprintln!("{msg}");
        }
    }
    pub fn new_with_seed(seed: Option<u64>, starting: PlayerPosition) -> Self {
        let match_state = if let Some(s) = seed {
            MatchState::with_seed(starting, s)
        } else {
            MatchState::new(starting)
        };
        let mut unseen_tracker = UnseenTracker::new();
        unseen_tracker.reset_for_round(match_state.round());
        crate::telemetry::hard::reset();
        let this = Self {
            match_state,
            last_trick: None,
            bot_difficulty: BotDifficulty::from_env(),
            unseen_tracker,
            think_config: ThinkConfig::from_env(),
        };
        Self::dbg(&format!(
            "mdhearts: AI weights {} | hard {} | moon {}",
            crate::bot::debug_weights_string(),
            crate::bot::search::debug_hard_weights_string(),
            debug_moon_config_string()
        ));
        this
    }

    pub fn new_from_match_state(match_state: hearts_core::game::match_state::MatchState) -> Self {
        let mut unseen_tracker = UnseenTracker::new();
        unseen_tracker.reset_for_round(match_state.round());
        crate::telemetry::hard::reset();
        let this = Self {
            match_state,
            last_trick: None,
            bot_difficulty: BotDifficulty::from_env(),
            unseen_tracker,
            think_config: ThinkConfig::from_env(),
        };
        Self::dbg(&format!(
            "mdhearts: AI weights {} | hard {} | moon {}",
            crate::bot::debug_weights_string(),
            crate::bot::search::debug_hard_weights_string(),
            debug_moon_config_string()
        ));
        this
    }

    pub fn bot_context(&self, seat: PlayerPosition) -> BotContext<'_> {
        let (scores, bias_delta) = biased_scores(self.match_state.scores(), seat);
        BotContext::new(
            seat,
            self.match_state.round(),
            scores,
            self.match_state.passing_direction(),
            &self.unseen_tracker,
            self.bot_difficulty,
        )
        .with_controller_bias_delta(bias_delta)
    }

    pub fn set_bot_difficulty(&mut self, difficulty: BotDifficulty) {
        self.bot_difficulty = difficulty;
    }

    pub fn bot_difficulty(&self) -> BotDifficulty {
        self.bot_difficulty
    }

    pub fn think_config(&self) -> ThinkConfig {
        self.think_config
    }

    pub fn set_think_config(&mut self, config: ThinkConfig) {
        self.think_config = config;
    }

    pub fn prepare_bot_think(&self, seat: PlayerPosition) -> Option<BotThinkRequest> {
        if self.in_passing_phase() {
            return None;
        }
        if seat != self.expected_to_play() {
            return None;
        }
        let legal = self.legal_moves(seat);
        if legal.is_empty() {
            return None;
        }
        let enforce_two = {
            let round = self.match_state.round();
            round.is_first_trick() && round.current_trick().leader() == seat
        };
        crate::telemetry::hard::record_pre_decision(
            seat,
            &self.unseen_tracker,
            self.bot_difficulty,
        );
        let snapshot =
            BotSnapshot::capture(&self.match_state, &self.unseen_tracker).make_thread_local();
        Some(BotThinkRequest {
            seat,
            legal,
            enforce_two,
            difficulty: self.bot_difficulty,
            snapshot,
            config: self.think_config,
        })
    }

    pub fn apply_bot_move(
        &mut self,
        seat: PlayerPosition,
        card: Card,
    ) -> Result<(PlayerPosition, Card), PlayError> {
        let _ = self.play(seat, card)?;
        Ok((seat, card))
    }

    pub fn timeout_fallback_card(&self, seat: PlayerPosition) -> Option<Card> {
        if self.in_passing_phase() {
            return None;
        }
        if seat != self.expected_to_play() {
            return None;
        }
        let legal = self.legal_moves(seat);
        if legal.is_empty() {
            return None;
        }
        let enforce_two = {
            let round = self.match_state.round();
            round.is_first_trick() && round.current_trick().leader() == seat
        };
        if enforce_two {
            let two = Card::new(Rank::Two, Suit::Clubs);
            if let Some(card) = legal.iter().copied().find(|c| *c == two) {
                return Some(card);
            }
        }
        match self.think_config.fallback {
            TimeoutFallback::SkipAndLog => None,
            TimeoutFallback::FirstLegal => legal.first().copied(),
            TimeoutFallback::HeuristicBest => {
                let ctx = self.bot_context(seat);
                match self.bot_difficulty {
                    BotDifficulty::SearchLookahead | BotDifficulty::FutureHard => {
                        crate::bot::PlayPlannerHard::choose(&legal, &ctx)
                    }
                    _ => PlayPlanner::choose(&legal, &ctx),
                }
                .or_else(|| legal.first().copied())
            }
        }
    }

    pub fn match_over(&self) -> bool {
        self.match_state
            .scores()
            .standings()
            .iter()
            .copied()
            .any(|score| score >= 100)
    }

    pub fn match_winner(&self) -> Option<PlayerPosition> {
        if self.match_over() {
            Some(self.match_state.scores().leading_player())
        } else {
            None
        }
    }

    #[cfg(test)]
    fn configure_for_test(&mut self) {
        self.bot_difficulty = BotDifficulty::NormalHeuristic;
        self.unseen_tracker
            .reset_for_round(self.match_state.round());
    }
    pub fn status_text(&self) -> String {
        let round = self.match_state.round();
        let passing = self.match_state.passing_direction().as_str();
        let leader = round.current_trick().leader();
        format!(
            "Round {} | Passing: {} | Leader: {}",
            self.match_state.round_number(),
            passing,
            leader
        )
    }

    pub fn legal_moves(&self, seat: PlayerPosition) -> Vec<Card> {
        self.match_state.round().legal_cards(seat)
    }

    pub fn play(&mut self, seat: PlayerPosition, card: Card) -> Result<PlayOutcome, PlayError> {
        // Snapshot trick before applying the play so we can reconstruct on completion
        let pre_plays: Vec<(PlayerPosition, Card)> = {
            let round = self.match_state.round();
            round
                .current_trick()
                .plays()
                .iter()
                .map(|p| (p.position, p.card))
                .collect()
        };
        let out = {
            let round = self.match_state.round_mut();
            round.play_card(seat, card)
        };

        let out = match out {
            Ok(value) => {
                // Track card reveal
                self.unseen_tracker.note_card_played(seat, card);
                // If this was a follow where suit was not followed, we can deduce a void.
                if let Some(lead_suit) = pre_plays.first().map(|p| p.1.suit)
                    && card.suit != lead_suit
                {
                    self.unseen_tracker.note_void(seat, lead_suit);
                }
                value
            }
            Err(err) => return Err(err),
        };

        if let PlayOutcome::TrickCompleted { winner, penalties } = out {
            let mut plays = pre_plays;
            plays.push((seat, card));
            let hearts_broken = self.match_state.round().hearts_broken();
            self.unseen_tracker
                .note_trick_completion(&plays, winner, penalties, hearts_broken);
            self.last_trick = Some(TrickSummary { winner, plays });
            // Update moon state heuristics for the winner and others.
            self.update_moon_states_after_trick(winner, penalties);
        }

        // Defer end-of-round auto-advance to the UI so we can finish
        // trick collect animations before dealing the next round.

        Ok(out)
    }

    fn update_moon_states_after_trick(&mut self, winner: PlayerPosition, penalties: u8) {
        // Stage 2 heuristic: commit if a seat shows sustained control early with low penalties; abort on loss of control or fragmented points.
        let round = self.match_state.round();
        let scores = self.match_state.scores();
        let cards_played = 52usize.saturating_sub(self.unseen_tracker.unseen_count());
        let totals = round.penalty_totals();
        let cfg = moon_cfg();
        use hearts_core::model::rank::Rank;
        use hearts_core::model::suit::Suit;

        for seat in PlayerPosition::LOOP.iter().copied() {
            let state = self.unseen_tracker.moon_state(seat);
            // Commit gate: winner of a clean, early trick, with good hearts capacity and some control history.
            if (state == MoonState::Inactive || state == MoonState::Considering)
                && seat == winner
                && penalties == 0
                && cards_played <= cfg.commit_cards_played_max
                && scores.score(seat) < cfg.commit_max_score
            {
                let tricks_won = {
                    let mut counts = [0u8; 4];
                    for trick in round.trick_history() {
                        if let Some(w) = trick.winner() {
                            counts[w.index()] = counts[w.index()].saturating_add(1);
                        }
                    }
                    counts[seat.index()]
                };
                let hand = round.hand(seat);
                let hearts_in_hand = hand.iter().filter(|c| c.suit == Suit::Hearts).count();
                let control_hearts = hand
                    .iter()
                    .filter(|c| c.suit == Suit::Hearts && c.rank >= Rank::Ten)
                    .count();
                if hearts_in_hand >= cfg.commit_min_hearts
                    && control_hearts >= cfg.commit_min_control_hearts
                {
                    if tricks_won as usize >= cfg.commit_min_tricks_won as usize {
                        let next = if state == MoonState::Considering {
                            MoonState::Committed
                        } else {
                            MoonState::Considering
                        };
                        Self::dbg(&format!(
                            "mdhearts: moon {:?} -> {:?} (seat={:?}, tricks_won={}, hearts={}, control_hearts={})",
                            state, next, seat, tricks_won, hearts_in_hand, control_hearts
                        ));
                        self.unseen_tracker.set_moon_state(seat, next);
                    } else if state == MoonState::Inactive && tricks_won >= 1 {
                        Self::dbg(&format!(
                            "mdhearts: moon {:?} -> Considering (seat={:?}, tricks_won={}, hearts={}, control_hearts={})",
                            state, seat, tricks_won, hearts_in_hand, control_hearts
                        ));
                        self.unseen_tracker
                            .set_moon_state(seat, MoonState::Considering);
                    }
                }
            }

            // Abort conditions for committed moon: lost control, opponents collected hearts, near-end, or too few hearts left
            if state == MoonState::Committed {
                let my_total = totals[seat.index()];
                let others_hearts: u32 = PlayerPosition::LOOP
                    .iter()
                    .copied()
                    .filter(|&p| p != seat)
                    .map(|p| totals[p.index()] as u32)
                    .sum();
                let near_end = cards_played >= cfg.abort_near_end_cards_played_min;
                let lost_control_recent =
                    cfg.abort_on_lost_control_clean && winner != seat && penalties == 0;
                let hearts_left = round
                    .hand(seat)
                    .iter()
                    .filter(|c| c.suit == Suit::Hearts)
                    .count();
                if others_hearts >= cfg.abort_others_hearts_min
                    || near_end
                    || lost_control_recent
                    || hearts_left < cfg.abort_min_hearts_left
                {
                    Self::dbg(&format!(
                        "mdhearts: moon ABORT for {:?} (others_hearts={}, near_end={}, lost_control={}, hearts_left={}, my_total={})",
                        seat, others_hearts, near_end, lost_control_recent, hearts_left, my_total
                    ));
                    self.unseen_tracker
                        .set_moon_state(seat, MoonState::Inactive);
                }
            }
        }
    }

    pub fn in_passing_phase(&self) -> bool {
        matches!(self.match_state.round().phase(), RoundPhase::Passing(_))
    }

    pub fn submit_pass(
        &mut self,
        seat: PlayerPosition,
        cards: [Card; 3],
    ) -> Result<(), hearts_core::model::passing::PassingError> {
        let result = self.match_state.round_mut().submit_pass(seat, cards);
        if result.is_ok() {
            self.unseen_tracker.note_pass_selection(seat, &cards);
        }
        result
    }

    pub fn resolve_passes(&mut self) -> Result<(), hearts_core::model::passing::PassingError> {
        self.match_state.round_mut().resolve_passes()
    }

    pub fn standings(&self) -> [u32; 4] {
        *self.match_state.scores().standings()
    }

    pub fn round_number(&self) -> u32 {
        self.match_state.round_number()
    }

    pub fn penalties_this_round(&self) -> [u8; 4] {
        self.match_state.round_penalties()
    }

    pub fn tricks_won_this_round(&self) -> [u8; 4] {
        let mut counts = [0u8; 4];
        let round = self.match_state.round();
        for trick in round.trick_history() {
            if let Some(w) = trick.winner() {
                let idx = w.index();
                counts[idx] = counts[idx].saturating_add(1);
            }
        }
        counts
    }

    pub fn passing_direction(&self) -> hearts_core::model::passing::PassingDirection {
        self.match_state.passing_direction()
    }

    pub fn explain_candidates_for(&self, seat: PlayerPosition) -> Vec<(Card, i32)> {
        let legal = self.legal_moves(seat);
        let ctx = self.bot_context(seat);
        match self.bot_difficulty {
            BotDifficulty::SearchLookahead | BotDifficulty::FutureHard => {
                crate::bot::PlayPlannerHard::explain_candidates(&legal, &ctx)
            }
            _ => crate::bot::PlayPlanner::explain_candidates(&legal, &ctx),
        }
    }

    // Test-only helpers
    #[cfg(test)]
    pub fn set_round_and_scores_for_test(
        &mut self,
        round: hearts_core::model::round::RoundState,
        scores: [u32; 4],
    ) {
        *self.match_state.round_mut() = round;
        self.match_state.scores_mut().set_totals(scores);
        self.unseen_tracker
            .reset_for_round(self.match_state.round());
    }

    #[cfg(test)]
    pub fn moon_state_for_test(&self, seat: PlayerPosition) -> crate::bot::MoonState {
        self.unseen_tracker.moon_state(seat)
    }

    #[cfg(test)]
    pub fn current_trick_plays_for_test(&self) -> usize {
        self.match_state.round().current_trick().plays().len()
    }
}

fn biased_scores(base: &ScoreBoard, seat: PlayerPosition) -> (ScoreBoard, Option<i32>) {
    match mix_hint_bias_delta(seat) {
        Some(delta) => (base.with_bias(seat, delta), Some(delta)),
        None => (*base, None),
    }
}

fn mix_hint_bias_delta(seat: PlayerPosition) -> Option<i32> {
    let raw = std::env::var("MDH_SEARCH_MIX_HINT").ok()?;
    mix_hint_bias_delta_from_hint(raw.trim(), seat)
}

fn mix_hint_bias_delta_from_hint(raw: &str, seat: PlayerPosition) -> Option<i32> {
    if raw.is_empty() {
        return None;
    }
    let mut parts = raw.splitn(3, ':');
    let mix = parts.next()?.trim().to_ascii_lowercase();
    if mix.is_empty() {
        return None;
    }

    let part_two = parts.next().map(|s| s.trim()).filter(|s| !s.is_empty());
    let part_three = parts.next().map(|s| s.trim()).filter(|s| !s.is_empty());

    let seat_hint = part_two.and_then(parse_hint_seat);

    let mut delta_hint = part_three.and_then(parse_bias_delta).or_else(|| {
        if seat_hint.is_none() {
            part_two.and_then(parse_bias_delta)
        } else {
            None
        }
    });

    if let Some(target) = seat_hint {
        if target != seat {
            return None;
        }
    }

    if delta_hint.is_none() {
        delta_hint = default_bias_for_mix(&mix);
    }

    delta_hint
}

fn parse_hint_seat(label: &str) -> Option<PlayerPosition> {
    match label.to_ascii_lowercase().as_str() {
        "" => None,
        "n" | "north" => Some(PlayerPosition::North),
        "e" | "east" => Some(PlayerPosition::East),
        "s" | "south" => Some(PlayerPosition::South),
        "w" | "west" => Some(PlayerPosition::West),
        _ => None,
    }
}

fn parse_bias_delta(raw: &str) -> Option<i32> {
    if raw.is_empty() {
        return None;
    }
    raw.parse::<i32>().ok()
}

fn default_bias_for_mix(mix: &str) -> Option<i32> {
    match mix {
        "shsh" => Some(8),
        _ => None,
    }
}

#[cfg(test)]
mod mix_hint_bias_tests {
    use super::{default_bias_for_mix, mix_hint_bias_delta_from_hint};
    use hearts_core::model::player::PlayerPosition;

    #[test]
    fn applies_default_for_shsh_all_seats() {
        assert_eq!(
            mix_hint_bias_delta_from_hint("shsh", PlayerPosition::East),
            default_bias_for_mix("shsh")
        );
        assert_eq!(
            mix_hint_bias_delta_from_hint("shsh", PlayerPosition::North),
            default_bias_for_mix("shsh")
        );
    }

    #[test]
    fn respects_seat_specific_hints() {
        assert_eq!(
            mix_hint_bias_delta_from_hint("shsh:e", PlayerPosition::East),
            default_bias_for_mix("shsh")
        );
        assert_eq!(
            mix_hint_bias_delta_from_hint("shsh:e", PlayerPosition::North),
            None
        );
    }

    #[test]
    fn parses_explicit_deltas_with_and_without_seats() {
        assert_eq!(
            mix_hint_bias_delta_from_hint("shsh::+12", PlayerPosition::South),
            Some(12)
        );
        assert_eq!(
            mix_hint_bias_delta_from_hint("snnh:+5", PlayerPosition::North),
            Some(5)
        );
        assert_eq!(
            mix_hint_bias_delta_from_hint("snnh:w:-3", PlayerPosition::West),
            Some(-3)
        );
    }

    #[test]
    fn ignores_unrecognized_mixes_without_explicit_delta() {
        assert_eq!(
            mix_hint_bias_delta_from_hint("unknown:e", PlayerPosition::East),
            None
        );
    }
}

// --- Moon tuning config ---
#[derive(Debug, Clone, Copy)]
struct MoonConfig {
    commit_cards_played_max: usize,
    commit_max_score: u32,
    commit_min_tricks_won: u8,
    commit_min_hearts: usize,
    commit_min_control_hearts: usize,
    abort_others_hearts_min: u32,
    abort_near_end_cards_played_min: usize,
    abort_min_hearts_left: usize,
    abort_on_lost_control_clean: bool,
}

fn parse_env_u32(key: &str) -> Option<u32> {
    std::env::var(key).ok().and_then(|s| s.parse::<u32>().ok())
}
fn parse_env_usize(key: &str) -> Option<usize> {
    std::env::var(key)
        .ok()
        .and_then(|s| s.parse::<usize>().ok())
}
fn parse_env_u8(key: &str) -> Option<u8> {
    std::env::var(key).ok().and_then(|s| s.parse::<u8>().ok())
}
fn parse_env_bool(key: &str) -> Option<bool> {
    std::env::var(key).ok().and_then(|v| {
        let l = v.to_ascii_lowercase();
        Some(l == "1" || l == "true" || l == "on")
    })
}

fn moon_cfg() -> MoonConfig {
    MoonConfig {
        commit_cards_played_max: parse_env_usize("MDH_MOON_COMMIT_MAX_CARDS").unwrap_or(20),
        commit_max_score: parse_env_u32("MDH_MOON_COMMIT_MAX_SCORE").unwrap_or(70),
        commit_min_tricks_won: parse_env_u8("MDH_MOON_COMMIT_MIN_TRICKS").unwrap_or(2),
        commit_min_hearts: parse_env_usize("MDH_MOON_COMMIT_MIN_HEARTS").unwrap_or(5),
        commit_min_control_hearts: parse_env_usize("MDH_MOON_COMMIT_MIN_CONTROL").unwrap_or(3),
        abort_others_hearts_min: parse_env_u32("MDH_MOON_ABORT_OTHERS_HEARTS").unwrap_or(3),
        abort_near_end_cards_played_min: parse_env_usize("MDH_MOON_ABORT_NEAREND_CARDS")
            .unwrap_or(36),
        abort_min_hearts_left: parse_env_usize("MDH_MOON_ABORT_MIN_HEARTS_LEFT").unwrap_or(3),
        abort_on_lost_control_clean: parse_env_bool("MDH_MOON_ABORT_LOST_CONTROL").unwrap_or(true),
    }
}

fn debug_moon_config_string() -> String {
    let c = moon_cfg();
    format!(
        "commit_max_cards={} commit_max_score={} commit_min_tricks={} commit_min_hearts={} commit_min_control={} abort_others_hearts={} abort_nearend_cards={} abort_min_hearts_left={} abort_lost_control={}",
        c.commit_cards_played_max,
        c.commit_max_score,
        c.commit_min_tricks_won,
        c.commit_min_hearts,
        c.commit_min_control_hearts,
        c.abort_others_hearts_min,
        c.abort_near_end_cards_played_min,
        c.abort_min_hearts_left,
        c.abort_on_lost_control_clean,
    )
}

#[derive(Debug, Clone)]
pub struct TrickSummary {
    pub winner: PlayerPosition,
    pub plays: Vec<(PlayerPosition, Card)>,
}

impl GameController {
    pub fn expected_to_play(&self) -> PlayerPosition {
        let trick = self.match_state.round().current_trick();
        trick
            .plays()
            .last()
            .map(|p| p.position.next())
            .unwrap_or(trick.leader())
    }

    pub fn take_last_trick_summary(&mut self) -> Option<TrickSummary> {
        self.last_trick.take()
    }

    pub fn last_trick(&self) -> Option<&TrickSummary> {
        self.last_trick.as_ref()
    }

    // Play a single AI move (if it's not stop_seat's turn). Returns the (seat, card) played.
    pub fn autoplay_one_with_status(
        &mut self,
        stop_seat: PlayerPosition,
    ) -> AutoplayOutcome {
        if self.in_passing_phase() {
            return AutoplayOutcome::NotExpected;
        }
        let seat = self.expected_to_play();
        if seat == stop_seat {
            return AutoplayOutcome::NotExpected;
        }
        let legal = self.legal_moves(seat);
        if legal.is_empty() {
            return AutoplayOutcome::NoLegal;
        }
        let enforce_two = {
            let round = self.match_state.round();
            round.is_first_trick() && round.current_trick().leader() == seat
        };

        crate::telemetry::hard::record_pre_decision(
            seat,
            &self.unseen_tracker,
            self.bot_difficulty,
        );
        let start = Instant::now();
        let think_limit_ms = self.think_config.limit_millis();
        let mut decision_limit = think_limit_ms.map(|ms| DecisionLimit {
            deadline: Some(start + Duration::from_millis(ms as u64)),
            cancel: None,
        });
        if test_force_autoplay_timeout() {
            decision_limit = Some(DecisionLimit {
                deadline: Some(Instant::now() - Duration::from_millis(1)),
                cancel: None,
            });
        }
        let mut last_bias_delta: Option<i32> = None;
        let mut card_to_play = if enforce_two {
            let two = Card::new(Rank::Two, Suit::Clubs);
            if legal.contains(&two) {
                Some(two)
            } else {
                legal.first().copied()
            }
        } else {
            match self.bot_difficulty {
                BotDifficulty::EasyLegacy => legal.first().copied(),
                BotDifficulty::SearchLookahead | BotDifficulty::FutureHard => {
                    let commit = {
                        let ctx_probe = self.bot_context(seat);
                        crate::bot::determine_style(&ctx_probe)
                            == crate::bot::BotStyle::AggressiveMoon
                    };
                    if commit {
                        self.unseen_tracker
                            .set_moon_state(seat, MoonState::Committed);
                    }
                    let ctx = self.bot_context(seat);
                    let result = crate::bot::PlayPlannerHard::choose_with_limit(
                        &legal,
                        &ctx,
                        decision_limit.as_ref(),
                    );
                    last_bias_delta = ctx.controller_bias_delta;
                    result
                }
                _ => {
                    let commit = {
                        let ctx_probe = self.bot_context(seat);
                        crate::bot::determine_style(&ctx_probe)
                            == crate::bot::BotStyle::AggressiveMoon
                    };
                    if commit {
                        self.unseen_tracker
                            .set_moon_state(seat, MoonState::Committed);
                    }
                    let ctx = self.bot_context(seat);
                    let result =
                        PlayPlanner::choose_with_limit(&legal, &ctx, decision_limit.as_ref());
                    last_bias_delta = ctx.controller_bias_delta;
                    result
                }
            }
        };

        let elapsed = start.elapsed();
        let timed_out = decision_limit
            .as_ref()
            .map(|limit| limit.expired())
            .unwrap_or(false);
        let mut fallback_label: Option<&'static str> = None;

        if card_to_play.is_none() && timed_out {
            fallback_label = Some(self.think_config.fallback.label());
            card_to_play = self.timeout_fallback_card(seat);
        }

        if card_to_play.is_none() && (!timed_out || fallback_label.is_none()) {
            if let Some(card) = legal.first().copied() {
                if fallback_label.is_none() {
                    fallback_label = Some("first_legal");
                }
                card_to_play = Some(card);
            }
        }

        if timed_out && fallback_label.is_none() {
            fallback_label = Some("planner_result");
        }

        let elapsed_ms = elapsed.as_millis().min(u32::MAX as u128) as u32;
        let search_stats = if matches!(
            self.bot_difficulty,
            BotDifficulty::SearchLookahead | BotDifficulty::FutureHard
        ) {
            crate::bot::search::last_stats()
                .map(|stats| crate::telemetry::hard::SearchTelemetrySnapshot::from_stats(&stats))
        } else {
            None
        };

        crate::telemetry::hard::record_post_decision(
            seat,
            &self.unseen_tracker,
            self.bot_difficulty,
            think_limit_ms,
            elapsed_ms,
            timed_out,
            fallback_label,
            search_stats,
            last_bias_delta,
        );

        if let Some(card) = card_to_play {
            Self::dbg(&format!("mdhearts: AI {:?} plays {}", seat, card));
            let _ = self.play(seat, card);
            AutoplayOutcome::Played(seat, card)
        } else if timed_out {
            AutoplayOutcome::SkippedTimeout
        } else {
            AutoplayOutcome::NoLegal
        }
    }

    pub fn autoplay_one(&mut self, stop_seat: PlayerPosition) -> Option<(PlayerPosition, Card)> {
        match self.autoplay_one_with_status(stop_seat) {
            AutoplayOutcome::Played(seat, card) => Some((seat, card)),
            _ => None,
        }
    }
    pub fn hand(&self, seat: PlayerPosition) -> Vec<Card> {
        self.match_state
            .round()
            .hand(seat)
            .iter()
            .copied()
            .collect()
    }

    pub fn legal_moves_set(&self, seat: PlayerPosition) -> std::collections::HashSet<Card> {
        use std::collections::HashSet;
        self.legal_moves(seat).into_iter().collect::<HashSet<_>>()
    }

    pub fn trick_leader(&self) -> PlayerPosition {
        self.match_state.round().current_trick().leader()
    }

    pub fn trick_plays(&self) -> Vec<(PlayerPosition, Card)> {
        self.match_state
            .round()
            .current_trick()
            .plays()
            .iter()
            .map(|p| (p.position, p.card))
            .collect()
    }

    pub fn simple_pass_for(&self, seat: PlayerPosition) -> Option<[Card; 3]> {
        let hand = self.match_state.round().hand(seat);
        match self.bot_difficulty {
            BotDifficulty::EasyLegacy => {
                if hand.len() < 3 {
                    return None;
                }
                Some([hand.cards()[0], hand.cards()[1], hand.cards()[2]])
            }
            _ => {
                let ctx = self.bot_context(seat);
                PassPlanner::choose(hand, &ctx)
            }
        }
    }

    pub fn submit_auto_passes_for_others(
        &mut self,
        except: PlayerPosition,
    ) -> Result<(), hearts_core::model::passing::PassingError> {
        for seat in PlayerPosition::LOOP.iter().copied() {
            if seat == except {
                continue;
            }
            if let Some(cards) = self.simple_pass_for(seat) {
                self.submit_pass(seat, cards)?;
            }
        }
        Ok(())
    }

    pub fn restart_round(&mut self) {
        let seed = self.match_state.seed();
        let round_num = self.match_state.round_number();
        let passing = self.match_state.passing_direction();
        let starting = self.match_state.round().starting_player();
        self.match_state =
            MatchState::with_seed_round_direction(seed, round_num, passing, starting);
        self.unseen_tracker
            .reset_for_round(self.match_state.round());
    }

    pub fn finish_round_if_ready(&mut self) -> Option<PlayerPosition> {
        if !self.match_state.is_round_ready_for_scoring() {
            return None;
        }
        if let Some(winner) = self.match_state.finish_round_and_start_next() {
            return Some(winner);
        }
        self.unseen_tracker
            .reset_for_round(self.match_state.round());
        crate::telemetry::hard::reset();
        None
    }
}
#[cfg(test)]
mod tests {
    use super::{GameController, TimeoutFallback};
    use crate::bot::BotDifficulty;
    use hearts_core::model::card::Card;
    use hearts_core::model::passing::PassingDirection;
    use hearts_core::model::player::PlayerPosition;
    use hearts_core::model::rank::Rank;
    use hearts_core::model::suit::Suit;
    use std::time::Duration;

    struct EnvVarGuard {
        key: &'static str,
        original: Option<String>,
    }

    impl EnvVarGuard {
        fn set(key: &'static str, value: &str) -> Self {
            let original = std::env::var(key).ok();
            unsafe { std::env::set_var(key, value) };
            Self { key, original }
        }
    }

    impl Drop for EnvVarGuard {
        fn drop(&mut self) {
            if let Some(ref value) = self.original {
                unsafe { std::env::set_var(self.key, value) };
            } else {
                unsafe { std::env::remove_var(self.key) };
            }
        }
    }

    #[test]
    fn easy_legacy_pass_returns_first_three() {
        let mut controller = GameController::new_with_seed(Some(42), PlayerPosition::North);
        controller.set_bot_difficulty(BotDifficulty::EasyLegacy);

        let seat = PlayerPosition::East;
        let hand = controller.hand(seat);
        assert!(hand.len() >= 3);
        let expected = [hand[0], hand[1], hand[2]];
        let actual = controller.simple_pass_for(seat).unwrap();
        assert_eq!(actual, expected);
    }

    #[test]
    fn easy_legacy_autoplay_uses_first_card() {
        let mut controller = GameController::new_with_seed(Some(123), PlayerPosition::North);
        controller.set_bot_difficulty(BotDifficulty::EasyLegacy);

        if controller.in_passing_phase() {
            let south_pass = controller.simple_pass_for(PlayerPosition::South).unwrap();
            controller
                .submit_pass(PlayerPosition::South, south_pass)
                .unwrap();
            controller
                .submit_auto_passes_for_others(PlayerPosition::South)
                .unwrap();
            controller.resolve_passes().unwrap();
        }

        let seat = controller.expected_to_play();
        let legal = controller.legal_moves(seat);
        assert!(!legal.is_empty());

        let result = controller.autoplay_one(PlayerPosition::South).unwrap();
        assert_eq!(result.0, seat);
        assert_eq!(result.1, legal[0]);
    }

    #[test]
    fn search_autoplay_records_think_limit() {
        let mut controller = GameController::new_with_seed(Some(321), PlayerPosition::North);
        controller.set_bot_difficulty(BotDifficulty::SearchLookahead);
        let mut config = controller.think_config();
        config.max_duration = Duration::from_millis(5);
        config.fallback = TimeoutFallback::FirstLegal;
        controller.set_think_config(config);

        if controller.in_passing_phase() {
            let south_pass = controller.simple_pass_for(PlayerPosition::South).unwrap();
            controller
                .submit_pass(PlayerPosition::South, south_pass)
                .unwrap();
            controller
                .submit_auto_passes_for_others(PlayerPosition::South)
                .unwrap();
            controller.resolve_passes().unwrap();
        }

        let (_, records) = crate::telemetry::hard::capture_for_test(|| {
            let _ = controller.autoplay_one(PlayerPosition::South);
        });
        assert!(!records.is_empty(), "expected telemetry record");
        let last = records.last().unwrap();
        assert_eq!(last.think_limit_ms, Some(5));
    }

    #[test]
    fn autoplay_timeout_records_fallback() {
        let mut controller = GameController::new_with_seed(Some(98765), PlayerPosition::North);
        controller.set_bot_difficulty(BotDifficulty::SearchLookahead);
        let mut cfg = controller.think_config();
        cfg.max_duration = Duration::from_millis(5);
        cfg.fallback = TimeoutFallback::FirstLegal;
        controller.set_think_config(cfg);

        if controller.in_passing_phase() {
            let south_pass = controller.simple_pass_for(PlayerPosition::South).unwrap();
            controller
                .submit_pass(PlayerPosition::South, south_pass)
                .unwrap();
            controller
                .submit_auto_passes_for_others(PlayerPosition::South)
                .unwrap();
            controller.resolve_passes().unwrap();
        }

        let prep_stop = controller.expected_to_play().next();
        let prep_play = controller.autoplay_one(prep_stop);
        assert!(prep_play.is_some(), "setup autoplay should succeed");
        let _timeout_guard = EnvVarGuard::set("MDH_TEST_FORCE_AUTOP_TIMEOUT", "1");

        let seat_to_play = controller.expected_to_play();
        let stop_seat = seat_to_play.next();
        assert!(
            super::test_force_autoplay_timeout(),
            "force timeout flag should be active"
        );
        let (result, records) = crate::telemetry::hard::capture_for_test(|| {
            controller.autoplay_one(stop_seat)
        });
        assert!(result.is_some(), "expected autoplay move");

        assert!(!records.is_empty(), "expected telemetry records");
        assert!(
            records.len() >= 2,
            "expected at least pre and post records"
        );
        let last = records.last().unwrap();
        assert_eq!(last.timed_out, Some(true));
        assert_eq!(last.fallback.as_deref(), Some("first_legal"));
    }

    #[test]
    fn scripted_round_cautious_lead_after_passes() {
        let mut controller = GameController::new_with_seed(Some(31415), PlayerPosition::North);
        controller.configure_for_test();

        if controller.in_passing_phase() {
            let south_pass = controller.simple_pass_for(PlayerPosition::South).unwrap();
            controller
                .submit_pass(PlayerPosition::South, south_pass)
                .unwrap();
            controller
                .submit_auto_passes_for_others(PlayerPosition::South)
                .unwrap();
            controller.resolve_passes().unwrap();
        }

        while !controller.in_passing_phase()
            && controller.expected_to_play() != PlayerPosition::South
        {
            if controller.autoplay_one(PlayerPosition::South).is_none() {
                break;
            }
        }

        assert_eq!(controller.expected_to_play(), PlayerPosition::South);
        let legal = controller.legal_moves(PlayerPosition::South);
        assert!(!legal.is_empty());
        let has_high_heart = legal
            .iter()
            .any(|card| card.suit == Suit::Hearts && card.rank >= Rank::Queen);

        if has_high_heart {
            let (_, played) = controller.autoplay_one(PlayerPosition::North).unwrap();
            assert_ne!(played.suit, Suit::Hearts);
        }

        while controller.autoplay_one(PlayerPosition::North).is_some() {
            if controller.in_passing_phase() {
                break;
            }
        }
    }
    #[test]
    fn first_ai_play_after_passes_is_two_of_clubs() {
        for seed in 0u64..1024 {
            let mut controller = GameController::new_with_seed(Some(seed), PlayerPosition::North);
            if controller.passing_direction() == PassingDirection::Hold {
                continue;
            }
            let south_pass = controller.simple_pass_for(PlayerPosition::South).unwrap();
            controller
                .submit_pass(PlayerPosition::South, south_pass)
                .unwrap();
            controller
                .submit_auto_passes_for_others(PlayerPosition::South)
                .unwrap();
            controller.resolve_passes().unwrap();

            let two = Card::new(Rank::Two, Suit::Clubs);
            let holder = PlayerPosition::LOOP
                .iter()
                .copied()
                .find(|seat| controller.hand(*seat).contains(&two))
                .expect("two of clubs dealt");
            assert_eq!(
                controller.trick_leader(),
                holder,
                "seed {} leader should hold 2C",
                seed
            );

            let mut first = None;
            loop {
                if controller.in_passing_phase() {
                    break;
                }
                let seat = controller.expected_to_play();
                if seat == PlayerPosition::South {
                    break;
                }
                match controller.autoplay_one(PlayerPosition::South) {
                    Some(play) => {
                        first.get_or_insert(play);
                    }
                    None => break,
                }
            }

            if holder == PlayerPosition::South {
                let legal = controller.legal_moves(PlayerPosition::South);
                assert_eq!(legal.len(), 1, "seed {} south legal count", seed);
                assert_eq!(
                    legal[0],
                    Card::new(Rank::Two, Suit::Clubs),
                    "seed {} south must hold 2C",
                    seed
                );
            } else if let Some((_, card)) = first {
                assert_eq!(
                    card,
                    Card::new(Rank::Two, Suit::Clubs),
                    "seed {} should lead with 2C",
                    seed
                );
            }
        }
    }

    #[test]
    fn moon_commit_then_abort_flow() {
        let mut controller = GameController::new_with_seed(Some(12345), PlayerPosition::South);
        controller.configure_for_test();

        // Hands setup
        let south = vec![
            // Hearts control
            Card::new(Rank::Ace, Suit::Hearts),
            Card::new(Rank::King, Suit::Hearts),
            Card::new(Rank::Queen, Suit::Hearts),
            Card::new(Rank::Jack, Suit::Hearts),
            Card::new(Rank::Ten, Suit::Hearts),
            // Clubs for clean captures
            Card::new(Rank::Ace, Suit::Clubs),
            Card::new(Rank::King, Suit::Clubs),
            // A low diamond to slough
            Card::new(Rank::Two, Suit::Diamonds),
        ];
        let east = vec![
            Card::new(Rank::Eight, Suit::Clubs),
            Card::new(Rank::Six, Suit::Clubs),
            Card::new(Rank::Ace, Suit::Diamonds), // later wins clean diamond trick
        ];
        let north = vec![
            Card::new(Rank::Seven, Suit::Clubs),
            Card::new(Rank::Three, Suit::Diamonds),
            Card::new(Rank::Six, Suit::Diamonds),
        ];
        let west = vec![
            Card::new(Rank::Six, Suit::Clubs),
            Card::new(Rank::Five, Suit::Clubs),
            Card::new(Rank::Four, Suit::Diamonds),
        ];

        let round = {
            use hearts_core::model::hand::Hand;
            use hearts_core::model::round::{RoundPhase, RoundState};
            use hearts_core::model::trick::Trick;
            let mut hands = [Hand::new(), Hand::new(), Hand::new(), Hand::new()];
            hands[PlayerPosition::South.index()] = Hand::with_cards(south);
            hands[PlayerPosition::East.index()] = Hand::with_cards(east);
            hands[PlayerPosition::North.index()] = Hand::with_cards(north);
            hands[PlayerPosition::West.index()] = Hand::with_cards(west);
            // Seed a completed trick to avoid first-trick (2C) restrictions
            let mut seed = Trick::new(PlayerPosition::South);
            seed.play(PlayerPosition::South, Card::new(Rank::Two, Suit::Clubs))
                .unwrap();
            seed.play(PlayerPosition::West, Card::new(Rank::Three, Suit::Clubs))
                .unwrap();
            seed.play(PlayerPosition::North, Card::new(Rank::Four, Suit::Clubs))
                .unwrap();
            seed.play(PlayerPosition::East, Card::new(Rank::Five, Suit::Clubs))
                .unwrap();
            RoundState::from_hands_with_state(
                hands,
                PlayerPosition::South,
                PassingDirection::Hold,
                RoundPhase::Playing,
                Trick::new(PlayerPosition::South),
                vec![seed],
                false,
            )
        };
        controller.set_round_and_scores_for_test(round, [10, 20, 10, 15]);

        // Trick 1 (clubs): South AC, then West 5C, North 7C, East 6C -> South wins clean
        let mut t1 = vec![
            (PlayerPosition::South, Card::new(Rank::Ace, Suit::Clubs)),
            (PlayerPosition::West, Card::new(Rank::Five, Suit::Clubs)),
            (PlayerPosition::North, Card::new(Rank::Seven, Suit::Clubs)),
            (PlayerPosition::East, Card::new(Rank::Six, Suit::Clubs)),
        ];
        for _ in 0..4 {
            let seat = controller.expected_to_play();
            let idx = t1
                .iter()
                .position(|(s, _)| *s == seat)
                .expect("seat to play present");
            let (_, card) = t1.remove(idx);
            let legal = controller.legal_moves(seat);
            assert!(legal.contains(&card), "illegal {:?} for {:?}", card, seat);
            controller.play(seat, card).expect("play ok");
        }

        assert_eq!(
            controller.moon_state_for_test(PlayerPosition::South) as u8,
            crate::bot::MoonState::Considering as u8
        );

        // Trick 2: South KC, West 6C, North 3D (void), East 8C -> South still wins clean
        let mut t2 = vec![
            (PlayerPosition::South, Card::new(Rank::King, Suit::Clubs)),
            (PlayerPosition::West, Card::new(Rank::Six, Suit::Clubs)),
            (
                PlayerPosition::North,
                Card::new(Rank::Three, Suit::Diamonds),
            ),
            (PlayerPosition::East, Card::new(Rank::Eight, Suit::Clubs)),
        ];
        for _ in 0..4 {
            let seat = controller.expected_to_play();
            let idx = t2
                .iter()
                .position(|(s, _)| *s == seat)
                .expect("seat to play present");
            let (_, card) = t2.remove(idx);
            let legal = controller.legal_moves(seat);
            assert!(legal.contains(&card), "illegal {:?} for {:?}", card, seat);
            controller.play(seat, card).expect("play ok");
        }

        assert_eq!(
            controller.moon_state_for_test(PlayerPosition::South) as u8,
            crate::bot::MoonState::Committed as u8
        );

        // Trick 3 (diamonds): South 2D, West 4D, North 3D, East AD -> East wins clean => abort
        let mut t3 = vec![
            (PlayerPosition::South, Card::new(Rank::Two, Suit::Diamonds)),
            (PlayerPosition::West, Card::new(Rank::Four, Suit::Diamonds)),
            (PlayerPosition::North, Card::new(Rank::Six, Suit::Diamonds)),
            (PlayerPosition::East, Card::new(Rank::Ace, Suit::Diamonds)),
        ];
        for _ in 0..4 {
            let seat = controller.expected_to_play();
            let idx = t3
                .iter()
                .position(|(s, _)| *s == seat)
                .expect("seat to play present");
            let (_, card) = t3.remove(idx);
            let legal = controller.legal_moves(seat);
            assert!(legal.contains(&card), "illegal {:?} for {:?}", card, seat);
            controller.play(seat, card).expect("play ok");
        }

        assert_eq!(
            controller.moon_state_for_test(PlayerPosition::South) as u8,
            crate::bot::MoonState::Inactive as u8
        );
    }

    // Note: Avoid env-mutation tests here; they can race under parallel test execution.
    // The following test was removed in favor of manual checks via CLI with MDH_DEBUG_LOGS.
    /*
    #[test]
    fn moon_abort_toggle_respected() {
        // Disable abort on lost clean control; prevent other abort criteria
        let prev_lost = std::env::var("MDH_MOON_ABORT_LOST_CONTROL").ok();
        let prev_others = std::env::var("MDH_MOON_ABORT_OTHERS_HEARTS").ok();
        let prev_near = std::env::var("MDH_MOON_ABORT_NEAREND_CARDS").ok();
        let prev_left = std::env::var("MDH_MOON_ABORT_MIN_HEARTS_LEFT").ok();
        unsafe {
            std::env::set_var("MDH_MOON_ABORT_LOST_CONTROL", "0");
            std::env::set_var("MDH_MOON_ABORT_OTHERS_HEARTS", "100");
            std::env::set_var("MDH_MOON_ABORT_NEAREND_CARDS", "100");
            std::env::set_var("MDH_MOON_ABORT_MIN_HEARTS_LEFT", "0");
        }

        let mut controller = GameController::new_with_seed(Some(12345), PlayerPosition::South);
        controller.configure_for_test();

        // Reuse the same scripted setup as in moon_commit_then_abort_flow
        let south = vec![
            Card::new(Rank::Ace, Suit::Hearts),
            Card::new(Rank::King, Suit::Hearts),
            Card::new(Rank::Queen, Suit::Hearts),
            Card::new(Rank::Jack, Suit::Hearts),
            Card::new(Rank::Ten, Suit::Hearts),
            Card::new(Rank::Ace, Suit::Clubs),
            Card::new(Rank::King, Suit::Clubs),
            Card::new(Rank::Two, Suit::Diamonds),
        ];
        let east = vec![
            Card::new(Rank::Eight, Suit::Clubs),
            Card::new(Rank::Six, Suit::Clubs),
            Card::new(Rank::Ace, Suit::Diamonds),
        ];
        let north = vec![
            Card::new(Rank::Seven, Suit::Clubs),
            Card::new(Rank::Three, Suit::Diamonds),
            Card::new(Rank::Six, Suit::Diamonds),
        ];
        let west = vec![
            Card::new(Rank::Six, Suit::Clubs),
            Card::new(Rank::Five, Suit::Clubs),
            Card::new(Rank::Four, Suit::Diamonds),
        ];
        let round = {
            use hearts_core::model::hand::Hand;
            use hearts_core::model::round::{RoundPhase, RoundState};
            use hearts_core::model::trick::Trick;
            let mut hands = [Hand::new(), Hand::new(), Hand::new(), Hand::new()];
            hands[PlayerPosition::South.index()] = Hand::with_cards(south);
            hands[PlayerPosition::East.index()] = Hand::with_cards(east);
            hands[PlayerPosition::North.index()] = Hand::with_cards(north);
            hands[PlayerPosition::West.index()] = Hand::with_cards(west);
            let mut seed = Trick::new(PlayerPosition::South);
            seed.play(PlayerPosition::South, Card::new(Rank::Two, Suit::Clubs)).unwrap();
            seed.play(PlayerPosition::West, Card::new(Rank::Three, Suit::Clubs)).unwrap();
            seed.play(PlayerPosition::North, Card::new(Rank::Four, Suit::Clubs)).unwrap();
            seed.play(PlayerPosition::East, Card::new(Rank::Five, Suit::Clubs)).unwrap();
            RoundState::from_hands_with_state(
                hands,
                PlayerPosition::South,
                PassingDirection::Hold,
                RoundPhase::Playing,
                Trick::new(PlayerPosition::South),
                vec![seed],
                false,
            )
        };
        controller.set_round_and_scores_for_test(round, [10, 20, 10, 15]);

        // Trick 1
        for (s, c) in [
            (PlayerPosition::South, Card::new(Rank::Ace, Suit::Clubs)),
            (PlayerPosition::West, Card::new(Rank::Five, Suit::Clubs)),
            (PlayerPosition::North, Card::new(Rank::Seven, Suit::Clubs)),
            (PlayerPosition::East, Card::new(Rank::Six, Suit::Clubs)),
        ] {
            controller.play(s, c).unwrap();
        }
        // Trick 2
        for (s, c) in [
            (PlayerPosition::South, Card::new(Rank::King, Suit::Clubs)),
            (PlayerPosition::West, Card::new(Rank::Six, Suit::Clubs)),
            (PlayerPosition::North, Card::new(Rank::Three, Suit::Diamonds)),
            (PlayerPosition::East, Card::new(Rank::Eight, Suit::Clubs)),
        ] {
            controller.play(s, c).unwrap();
        }
        // Trick 3 (lost clean control)
        for (s, c) in [
            (PlayerPosition::South, Card::new(Rank::Two, Suit::Diamonds)),
            (PlayerPosition::West, Card::new(Rank::Four, Suit::Diamonds)),
            (PlayerPosition::North, Card::new(Rank::Six, Suit::Diamonds)),
            (PlayerPosition::East, Card::new(Rank::Ace, Suit::Diamonds)),
        ] {
            controller.play(s, c).unwrap();
        }

        assert_eq!(
            controller.moon_state_for_test(PlayerPosition::South) as u8,
            crate::bot::MoonState::Committed as u8,
            "should remain committed when abort-on-lost-control disabled"
        );

        // Restore env
        unsafe {
            match prev_lost { Some(v) => std::env::set_var("MDH_MOON_ABORT_LOST_CONTROL", v), None => std::env::remove_var("MDH_MOON_ABORT_LOST_CONTROL") }
            match prev_others { Some(v) => std::env::set_var("MDH_MOON_ABORT_OTHERS_HEARTS", v), None => std::env::remove_var("MDH_MOON_ABORT_OTHERS_HEARTS") }
            match prev_near { Some(v) => std::env::set_var("MDH_MOON_ABORT_NEAREND_CARDS", v), None => std::env::remove_var("MDH_MOON_ABORT_NEAREND_CARDS") }
            match prev_left { Some(v) => std::env::set_var("MDH_MOON_ABORT_MIN_HEARTS_LEFT", v), None => std::env::remove_var("MDH_MOON_ABORT_MIN_HEARTS_LEFT") }
        }
    }
    */
}


