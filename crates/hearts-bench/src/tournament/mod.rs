mod external;
mod permutations;

use std::fs::{self, File};
use std::io::{BufWriter, Write};
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use crate::analytics::{AnalyticsCollector, AnalyticsError};
use external::ExternalPolicy;
use hearts_bot::bot::{BotDifficulty, BotFeatures, UnseenTracker};
use hearts_bot::policy::{HeuristicPolicy, Policy, PolicyContext};
use hearts_core::belief::Belief;
use hearts_core::game::match_state::MatchState;
use hearts_core::model::player::PlayerPosition;
use hearts_core::model::round::RoundPhase;
use rand::{RngCore, SeedableRng, rngs::StdRng};
use serde::Serialize;
use thiserror::Error;
use tracing::{Level, event};

use crate::config::{AgentConfig, AgentKind, BenchmarkConfig, ResolvedOutputs};
use crate::telemetry::{
    TelemetryError, TelemetryOutputs, append_highlights_to_markdown, write_summary_outputs,
};

use permutations::SeatPermutations;

const MAX_SEAT_PERMUTATIONS: usize = 24;

/// Primary entry point for orchestrating tournaments.
pub struct TournamentRunner {
    config: BenchmarkConfig,
    outputs: ResolvedOutputs,
    agents: Vec<AgentBlueprint>,
    seat_permutations: SeatPermutations,
    logging_enabled: bool,
    bot_features: BotFeatures,
}

/// Summary details returned after a run.
pub struct RunSummary {
    pub hands_played: usize,
    pub permutations: usize,
    pub rows_written: usize,
    pub jsonl_path: PathBuf,
    pub summary_path: PathBuf,
    pub plot_path: Option<PathBuf>,
    pub telemetry_path: Option<PathBuf>,
    pub telemetry_outputs: Option<TelemetryOutputs>,
}

impl TournamentRunner {
    /// Build a runner from a validated configuration.
    pub fn new(config: BenchmarkConfig, outputs: ResolvedOutputs) -> Result<Self, RunnerError> {
        let agents = AgentBlueprint::from_configs(&config.agents)?;

        if agents.len() != 4 {
            return Err(RunnerError::SeatCount {
                found: agents.len(),
            });
        }

        if config.deals.permutations > MAX_SEAT_PERMUTATIONS {
            return Err(RunnerError::PermutationLimit {
                requested: config.deals.permutations,
                max: MAX_SEAT_PERMUTATIONS,
            });
        }

        let seat_permutations = SeatPermutations::new(config.deals.permutations);

        Ok(Self {
            logging_enabled: config.logging.enable_structured,
            config,
            outputs,
            agents,
            seat_permutations,
            bot_features: BotFeatures::from_env(),
        })
    }

    /// Execute the tournament, streaming JSONL rows to disk.
    pub fn run(&self) -> Result<RunSummary, RunnerError> {
        ensure_parent(self.outputs.jsonl.parent())?;
        ensure_parent(self.outputs.summary_md.parent())?;
        if !self.outputs.plots_dir.as_os_str().is_empty() {
            fs::create_dir_all(&self.outputs.plots_dir)?;
        }

        let mut writer = BufWriter::new(File::create(&self.outputs.jsonl)?);
        let permutations = self.seat_permutations.as_slice();
        let mut rng = StdRng::seed_from_u64(self.config.deals.seed.unwrap_or(0));
        let mut rows_written = 0usize;
        let mut analytics = AnalyticsCollector::new(&self.config)?;

        for hand_index in 0..self.config.deals.hands {
            let base_seed = rng.next_u64();

            for (perm_index, perm) in permutations.iter().enumerate() {
                let outcome = self.play_hand(hand_index, perm_index, base_seed, perm)?;
                analytics.record_hand(hand_index, perm_index, &outcome)?;
                rows_written += write_hand_rows(
                    &mut writer,
                    &self.config,
                    hand_index,
                    perm_index,
                    base_seed,
                    &outcome,
                )?;
            }
        }

        writer.flush()?;

        let summary = analytics.finalize()?;
        summary.write_markdown(&self.outputs.summary_md)?;
        let plot_path = match summary.render_plot(&self.outputs.plots_dir) {
            Ok(path) => Some(path),
            Err(err) => {
                eprintln!("WARN: {}", err);
                None
            }
        };

        let telemetry_dir = self
            .outputs
            .summary_md
            .parent()
            .map(|p| p.to_path_buf())
            .unwrap_or_else(|| PathBuf::from("."));

        let telemetry_path = if self.logging_enabled {
            Some(telemetry_dir.join("telemetry.jsonl"))
        } else {
            None
        };

        let telemetry_outputs = if let Some(path) = telemetry_path.as_ref() {
            write_summary_outputs(path, &telemetry_dir)?
        } else {
            None
        };

        if let Some(outputs) = telemetry_outputs.as_ref() {
            append_highlights_to_markdown(&self.outputs.summary_md, outputs)?;
        }

        Ok(RunSummary {
            hands_played: self.config.deals.hands,
            permutations: permutations.len(),
            rows_written,
            jsonl_path: self.outputs.jsonl.clone(),
            summary_path: self.outputs.summary_md.clone(),
            plot_path,
            telemetry_path,
            telemetry_outputs,
        })
    }
}

fn ensure_parent(path: Option<&Path>) -> Result<(), RunnerError> {
    if let Some(dir) = path.filter(|dir| !dir.as_os_str().is_empty()) {
        fs::create_dir_all(dir)?;
    }
    Ok(())
}

fn write_hand_rows(
    writer: &mut BufWriter<File>,
    config: &BenchmarkConfig,
    hand_index: usize,
    permutation_index: usize,
    base_seed: u64,
    outcome: &HandOutcome,
) -> Result<usize, RunnerError> {
    let deal_id = format!("H{hand_index:05}_P{permutation_index:02}");
    let seating = outcome.seating.clone();

    let mut rows_written = 0usize;
    for seat_result in &outcome.seat_results {
        let seat_label = seat_label(seat_result.seat).to_string();
        let row = DealLogRow {
            run_id: config.run_id.clone(),
            deal_id: deal_id.clone(),
            hand_index,
            permutation_index,
            deal_seed: base_seed,
            seat: seat_label,
            bot: seat_result.agent_name.clone(),
            seating: seating.clone(),
            points: seat_result.points,
            pph: seat_result.points as f64,
            speed_ms_turn: seat_result.metrics.avg_ms_per_decision,
            decisions: seat_result.metrics.decisions,
        };

        serde_json::to_writer(&mut *writer, &row)?;
        writer.write_all(b"\n")?;
        rows_written += 1;
    }

    Ok(rows_written)
}

impl TournamentRunner {
    fn play_hand(
        &self,
        hand_index: usize,
        permutation_index: usize,
        base_seed: u64,
        permutation: &[usize; 4],
    ) -> Result<HandOutcome, RunnerError> {
        let mut match_state = MatchState::with_seed(PlayerPosition::North, base_seed);
        let mut seats = build_seat_states(permutation, &self.agents)?;

        for seat in &mut seats {
            seat.tracker.reset_for_round(match_state.round());
        }

        loop {
            if match_state.is_round_ready_for_scoring() {
                break;
            }

            if matches!(match_state.round().phase(), RoundPhase::Passing(_)) {
                for seat in &mut seats {
                    let cards = {
                        let round = match_state.round();
                        let belief_holder = if self.bot_features.belief_enabled() {
                            Some(Belief::from_state(round, seat.seat))
                        } else {
                            None
                        };
                        let belief_ref = belief_holder.as_ref();
                        let ctx = PolicyContext {
                            seat: seat.seat,
                            hand: round.hand(seat.seat),
                            round,
                            scores: match_state.scores(),
                            passing_direction: match_state.passing_direction(),
                            tracker: &seat.tracker,
                            belief: belief_ref,
                            features: self.bot_features,
                        };
                        let start = Instant::now();
                        let cards = seat.policy.choose_pass(&ctx);
                        let elapsed_ms = seat.metrics.record(start.elapsed());

                        if self.logging_enabled && tracing::enabled!(Level::INFO) {
                            let cards_str = cards
                                .iter()
                                .map(|c| format!("{:?}", c))
                                .collect::<Vec<_>>()
                                .join(",");
                            event!(
                                target: "hearts_bench::pass",
                                Level::INFO,
                                run_id = %self.config.run_id,
                                hand_index = hand_index as u32,
                                permutation_index = permutation_index as u32,
                                seat = seat_label(seat.seat),
                                cards = %cards_str,
                                elapsed_ms
                            );
                        }

                        cards
                    };

                    seat.tracker.note_pass_selection(seat.seat, &cards);
                    match_state
                        .round_mut()
                        .submit_pass(seat.seat, cards)
                        .map_err(|err| {
                            RunnerError::game(format!("pass submission failed: {:?}", err))
                        })?;
                }

                match_state.round_mut().resolve_passes().map_err(|err| {
                    RunnerError::game(format!("pass resolution failed: {:?}", err))
                })?;

                for seat in &mut seats {
                    seat.tracker.reset_for_round(match_state.round());
                }

                continue;
            }

            let expected_seat = {
                let round = match_state.round();
                let current_trick = round.current_trick();
                if let Some(last_play) = current_trick.plays().last() {
                    last_play.position.next()
                } else {
                    current_trick.leader()
                }
            };

            let seat_index = expected_seat.index();
            let card = {
                let round = match_state.round();
                let scores = match_state.scores();
                let passing_direction = match_state.passing_direction();
                let seat_state = &mut seats[seat_index];
                let belief_holder = if self.bot_features.belief_enabled() {
                    Some(Belief::from_state(round, expected_seat))
                } else {
                    None
                };
                let belief_ref = belief_holder.as_ref();
                let ctx = PolicyContext {
                    seat: expected_seat,
                    hand: round.hand(expected_seat),
                    round,
                    scores,
                    passing_direction,
                    tracker: &seat_state.tracker,
                    belief: belief_ref,
                    features: self.bot_features,
                };
                let start = Instant::now();
                let card = seat_state.policy.choose_play(&ctx);
                let elapsed_ms = seat_state.metrics.record(start.elapsed());

                if self.logging_enabled && tracing::enabled!(Level::INFO) {
                    let card_display = format!("{:?}", card);
                    event!(
                        target: "hearts_bench::play",
                        Level::INFO,
                        run_id = %self.config.run_id,
                        hand_index = hand_index as u32,
                        permutation_index = permutation_index as u32,
                        seat = seat_label(expected_seat),
                        card = %card_display,
                        elapsed_ms
                    );
                }

                card
            };

            seats[seat_index]
                .tracker
                .note_card_played(expected_seat, card);
            let play_result = {
                let round = match_state.round_mut();
                round.play_card(expected_seat, card)
            };
            if let Err(err) = play_result {
                let trick = match_state.round().current_trick();
                let trick_state = format!("leader={:?}, plays={:?}", trick.leader(), trick.plays());
                return Err(RunnerError::game(format!(
                    "invalid card play: {:?} (seat: {:?}, card: {:?}, trick: {})",
                    err, expected_seat, card, trick_state
                )));
            }
        }

        let penalties = match_state.round_penalties();
        let seating = seats
            .iter()
            .map(|seat| SeatSnapshot {
                seat: seat_label(seat.seat).to_string(),
                bot: seat.agent_name.clone(),
            })
            .collect();

        let mut seat_results = Vec::new();
        for seat in seats.into_iter() {
            let metrics = seat.metrics.finalize();
            seat_results.push(SeatResult {
                agent_name: seat.agent_name,
                seat: seat.seat,
                points: penalties[seat.seat.index()],
                metrics,
            });
        }

        let moon_shooter = detect_moon_shooter(&penalties);

        Ok(HandOutcome {
            seating,
            seat_results,
            penalties,
            moon_shooter,
        })
    }
}

fn build_seat_states(
    permutation: &[usize; 4],
    agents: &[AgentBlueprint],
) -> Result<Vec<SeatState>, RunnerError> {
    let mut seats = Vec::with_capacity(4);
    for (seat_idx, agent_idx) in permutation.iter().enumerate() {
        let seat = PlayerPosition::from_index(seat_idx).ok_or_else(|| {
            RunnerError::game(format!("invalid seat index generated: {}", seat_idx))
        })?;
        let agent = agents
            .get(*agent_idx)
            .ok_or(RunnerError::InvalidPermutation {
                index: seat_idx,
                agent_index: *agent_idx,
            })?;
        seats.push(SeatState::new(seat, agent));
    }
    Ok(seats)
}

fn seat_label(position: PlayerPosition) -> &'static str {
    match position {
        PlayerPosition::North => "north",
        PlayerPosition::East => "east",
        PlayerPosition::South => "south",
        PlayerPosition::West => "west",
    }
}

fn detect_moon_shooter(penalties: &[u8; 4]) -> Option<PlayerPosition> {
    let zero_count = penalties.iter().filter(|&&v| v == 0).count();
    let twenty_six_count = penalties.iter().filter(|&&v| v == 26).count();

    // Standard shoot-the-moon: shooter 0, others 26
    if zero_count == 1
        && twenty_six_count == 3
        && let Some((idx, _)) = penalties.iter().enumerate().find(|(_, p)| **p == 0)
    {
        return PlayerPosition::from_index(idx);
    }

    // Alternative variant: shooter 26, others 0 (if penalties inverted)
    if twenty_six_count == 1
        && zero_count == 3
        && let Some((idx, _)) = penalties.iter().enumerate().find(|(_, p)| **p == 26)
    {
        return PlayerPosition::from_index(idx);
    }

    None
}

struct SeatState {
    seat: PlayerPosition,
    agent_name: String,
    policy: Box<dyn Policy>,
    tracker: UnseenTracker,
    metrics: DecisionMetrics,
}

impl SeatState {
    fn new(seat: PlayerPosition, agent: &AgentBlueprint) -> Self {
        Self {
            seat,
            agent_name: agent.name.clone(),
            policy: agent.spawn_policy(),
            tracker: UnseenTracker::new(),
            metrics: DecisionMetrics::default(),
        }
    }
}

pub struct HandOutcome {
    pub seating: Vec<SeatSnapshot>,
    pub seat_results: Vec<SeatResult>,
    pub penalties: [u8; 4],
    pub moon_shooter: Option<PlayerPosition>,
}

#[derive(Clone, Serialize)]
pub struct SeatSnapshot {
    pub seat: String,
    pub bot: String,
}

pub struct SeatResult {
    pub agent_name: String,
    pub seat: PlayerPosition,
    pub points: u8,
    pub metrics: DecisionSummary,
}

#[derive(Default)]
struct DecisionMetrics {
    total: Duration,
    decisions: u32,
}

impl DecisionMetrics {
    fn record(&mut self, duration: Duration) -> f64 {
        self.total += duration;
        self.decisions += 1;
        duration.as_secs_f64() * 1000.0
    }

    fn finalize(self) -> DecisionSummary {
        let avg_ms = if self.decisions == 0 {
            0.0
        } else {
            self.total.as_secs_f64() * 1000.0 / f64::from(self.decisions)
        };

        DecisionSummary {
            decisions: self.decisions,
            avg_ms_per_decision: avg_ms,
            total_ms: self.total.as_secs_f64() * 1000.0,
        }
    }
}

#[derive(Clone)]
pub struct DecisionSummary {
    pub decisions: u32,
    pub avg_ms_per_decision: f64,
    pub total_ms: f64,
}

#[derive(Serialize)]
struct DealLogRow {
    run_id: String,
    deal_id: String,
    hand_index: usize,
    permutation_index: usize,
    deal_seed: u64,
    seat: String,
    bot: String,
    seating: Vec<SeatSnapshot>,
    points: u8,
    pph: f64,
    speed_ms_turn: f64,
    decisions: u32,
}

#[derive(Debug, Error)]
pub enum RunnerError {
    #[error("{0}")]
    Agent(#[from] AgentError),
    #[error("I/O error: {source}")]
    Io {
        #[from]
        source: std::io::Error,
    },
    #[error("failed to serialize log row: {source}")]
    Serialize {
        #[from]
        source: serde_json::Error,
    },
    #[error("game execution failed: {message}")]
    Game { message: String },
    #[error("configuration requires exactly 4 agents but found {found}")]
    SeatCount { found: usize },
    #[error("requested {requested} seat permutations exceeds maximum of {max}")]
    PermutationLimit { requested: usize, max: usize },
    #[error("permutation index {index} references invalid agent index {agent_index}")]
    InvalidPermutation { index: usize, agent_index: usize },
    #[error("analytics error: {0}")]
    Analytics(#[from] AnalyticsError),
    #[error("telemetry summarisation failed: {0}")]
    Telemetry(#[from] TelemetryError),
}

impl RunnerError {
    fn game(message: String) -> Self {
        RunnerError::Game { message }
    }
}

#[derive(Debug, Error)]
pub enum AgentError {
    #[error("unsupported agent kind {kind:?} for agent '{name}'")]
    UnsupportedKind { name: String, kind: AgentKind },
    #[error("invalid heuristic parameter for agent '{name}': {message}")]
    InvalidHeuristicParam { name: String, message: String },
    #[error("invalid external parameter for agent '{name}': {message}")]
    InvalidExternalParam { name: String, message: String },
}

struct AgentBlueprint {
    name: String,
    implementation: AgentImplementation,
}

enum AgentImplementation {
    Heuristic(HeuristicOptions),
    External(ExternalOptions),
}

impl AgentBlueprint {
    fn from_configs(configs: &[AgentConfig]) -> Result<Vec<Self>, AgentError> {
        configs.iter().map(Self::from_config).collect()
    }

    fn from_config(config: &AgentConfig) -> Result<Self, AgentError> {
        let implementation = match config.kind {
            AgentKind::Heuristic => {
                let options = HeuristicOptions::from_params(&config.name, &config.params)?;
                AgentImplementation::Heuristic(options)
            }
            AgentKind::External => {
                let options = ExternalOptions::from_params(&config.name, &config.params)?;
                AgentImplementation::External(options)
            }
            ref kind => {
                return Err(AgentError::UnsupportedKind {
                    name: config.name.clone(),
                    kind: kind.clone(),
                });
            }
        };

        Ok(Self {
            name: config.name.clone(),
            implementation,
        })
    }

    fn spawn_policy(&self) -> Box<dyn Policy> {
        match &self.implementation {
            AgentImplementation::Heuristic(opts) => Box::new(HeuristicPolicy::new(opts.difficulty)),
            AgentImplementation::External(opts) => opts.spawn_policy(&self.name),
        }
    }
}

struct HeuristicOptions {
    difficulty: BotDifficulty,
}

impl HeuristicOptions {
    fn from_params(name: &str, params: &serde_yaml::Value) -> Result<Self, AgentError> {
        if params.is_null() {
            return Ok(Self {
                difficulty: BotDifficulty::NormalHeuristic,
            });
        }

        let mapping = params
            .as_mapping()
            .ok_or_else(|| AgentError::InvalidHeuristicParam {
                name: name.to_string(),
                message: "expected mapping for heuristic params".to_string(),
            })?;

        let difficulty_value = mapping
            .iter()
            .find_map(|(key, value)| (key.as_str() == Some("difficulty")).then_some(value));

        let difficulty = if let Some(value) = difficulty_value {
            let text = value
                .as_str()
                .ok_or_else(|| AgentError::InvalidHeuristicParam {
                    name: name.to_string(),
                    message: "difficulty must be a string".to_string(),
                })?;

            match text.to_ascii_lowercase().as_str() {
                "easy" | "legacy" => BotDifficulty::EasyLegacy,
                "normal" | "default" => BotDifficulty::NormalHeuristic,
                "hard" | "future" => BotDifficulty::FutureHard,
                other => {
                    return Err(AgentError::InvalidHeuristicParam {
                        name: name.to_string(),
                        message: format!("unknown difficulty '{other}'"),
                    });
                }
            }
        } else {
            BotDifficulty::NormalHeuristic
        };

        Ok(Self { difficulty })
    }
}

#[derive(Clone, Copy)]
pub(super) enum ExternalFallback {
    Heuristic(BotDifficulty),
    Error,
}

#[allow(dead_code)]
#[derive(Clone)]
pub(super) struct ExternalOptions {
    pub(super) command: Option<String>,
    pub(super) args: Vec<String>,
    pub(super) working_dir: Option<PathBuf>,
    pub(super) timeout_ms: Option<u64>,
    pub(super) fallback: ExternalFallback,
}

impl ExternalOptions {
    fn from_params(name: &str, params: &serde_yaml::Value) -> Result<Self, AgentError> {
        let mut command = None;
        let mut args = Vec::new();
        let mut working_dir = None;
        let mut timeout_ms = None;
        let mut fallback = ExternalFallback::Heuristic(BotDifficulty::NormalHeuristic);

        if params.is_null() {
            return Ok(Self {
                command,
                args,
                working_dir,
                timeout_ms,
                fallback: ExternalFallback::Heuristic(BotDifficulty::NormalHeuristic),
            });
        }

        let mapping = params
            .as_mapping()
            .ok_or_else(|| AgentError::InvalidExternalParam {
                name: name.to_string(),
                message: "expected mapping for external params".to_string(),
            })?;

        for (key, value) in mapping {
            match key.as_str() {
                Some("command") => {
                    command = value.as_str().map(|s| s.to_string());
                    if command.is_none() {
                        return Err(AgentError::InvalidExternalParam {
                            name: name.to_string(),
                            message: "command must be a string".to_string(),
                        });
                    }
                }
                Some("args") => {
                    if let Some(seq) = value.as_sequence() {
                        args = seq
                            .iter()
                            .filter_map(|v| v.as_str().map(|s| s.to_string()))
                            .collect();
                    } else {
                        return Err(AgentError::InvalidExternalParam {
                            name: name.to_string(),
                            message: "args must be an array of strings".to_string(),
                        });
                    }
                }
                Some("working_dir") => {
                    working_dir = value.as_str().map(PathBuf::from);
                }
                Some("timeout_ms") => {
                    timeout_ms = value.as_u64();
                }
                Some("fallback") => {
                    if let Some(fallback_str) = value.as_str() {
                        fallback = match fallback_str.to_ascii_lowercase().as_str() {
                            "error" | "none" => ExternalFallback::Error,
                            "heuristic_easy" => {
                                ExternalFallback::Heuristic(BotDifficulty::EasyLegacy)
                            }
                            "heuristic_normal" => {
                                ExternalFallback::Heuristic(BotDifficulty::NormalHeuristic)
                            }
                            "heuristic_hard" => {
                                ExternalFallback::Heuristic(BotDifficulty::FutureHard)
                            }
                            other => {
                                return Err(AgentError::InvalidExternalParam {
                                    name: name.to_string(),
                                    message: format!("unknown fallback '{other}'"),
                                });
                            }
                        };
                    } else {
                        return Err(AgentError::InvalidExternalParam {
                            name: name.to_string(),
                            message: "fallback must be a string".to_string(),
                        });
                    }
                }
                _ => {}
            }
        }

        Ok(Self {
            command,
            args,
            working_dir,
            timeout_ms,
            fallback,
        })
    }

    fn spawn_policy(&self, name: &str) -> Box<dyn Policy> {
        Box::new(ExternalPolicy::new(name.to_string(), self.clone()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::iter::FromIterator;

    #[test]
    fn permutations_enumerate_unique_values() {
        let perms = SeatPermutations::new(24);
        assert_eq!(perms.as_slice().len(), 24);

        let mut seen = perms.as_slice().to_vec();
        seen.sort();
        seen.dedup();
        assert_eq!(seen.len(), 24);
    }

    #[test]
    fn heuristic_params_default_to_normal() {
        let params = serde_yaml::Value::Mapping(Default::default());
        let options = HeuristicOptions::from_params("bot", &params).unwrap();
        assert!(matches!(options.difficulty, BotDifficulty::NormalHeuristic));
    }

    #[test]
    fn heuristic_params_parse_easy() {
        let params = serde_yaml::Mapping::from_iter([(
            serde_yaml::Value::String("difficulty".into()),
            serde_yaml::Value::String("easy".into()),
        )]);
        let options =
            HeuristicOptions::from_params("bot", &serde_yaml::Value::Mapping(params)).unwrap();
        assert!(matches!(options.difficulty, BotDifficulty::EasyLegacy));
    }

    #[test]
    fn external_options_default_fallback() {
        let params = serde_yaml::Mapping::from_iter([(
            serde_yaml::Value::String("command".into()),
            serde_yaml::Value::String("/opt/xinxin".into()),
        )]);
        let options =
            ExternalOptions::from_params("xinxin", &serde_yaml::Value::Mapping(params)).unwrap();
        let mut policy = options.spawn_policy("xinxin");

        // basic sanity: ensure policy can produce a pass for a trivial context using heuristics fallback
        use hearts_core::model::card::Card;
        use hearts_core::model::hand::Hand;
        use hearts_core::model::passing::{PassingDirection, PassingState};
        use hearts_core::model::player::PlayerPosition;
        use hearts_core::model::rank::Rank;
        use hearts_core::model::round::{RoundPhase, RoundState};
        use hearts_core::model::score::ScoreBoard;
        use hearts_core::model::suit::Suit;

        let hand_cards = vec![
            Card::new(Rank::Two, Suit::Clubs),
            Card::new(Rank::Three, Suit::Clubs),
            Card::new(Rank::Four, Suit::Clubs),
            Card::new(Rank::Five, Suit::Diamonds),
        ];
        let mut hands: [Hand; 4] = std::array::from_fn(|_| Hand::new());
        hands[PlayerPosition::North.index()] = Hand::with_cards(hand_cards.clone());
        let round = RoundState::from_hands(
            hands,
            PlayerPosition::North,
            PassingDirection::Left,
            RoundPhase::Passing(PassingState::new(PassingDirection::Left)),
        );
        let scores = ScoreBoard::new();
        let tracker = UnseenTracker::new();
        let ctx = PolicyContext {
            seat: PlayerPosition::North,
            hand: round.hand(PlayerPosition::North),
            round: &round,
            scores: &scores,
            passing_direction: round.passing_direction(),
            tracker: &tracker,
            belief: None,
            features: BotFeatures::default(),
        };
        let pass = policy.choose_pass(&ctx);
        assert_eq!(pass.len(), 3);
    }
}
