use super::{
    BotContext, DecisionLimit, MoonState, PlayPlanner, detect_moon_pressure, snapshot_scores,
};
use hearts_core::model::card::Card;
use hearts_core::model::player::PlayerPosition;
use hearts_core::model::round::{PlayOutcome, RoundState};
use hearts_core::model::suit::Suit;
use once_cell::sync::Lazy;
use serde::Deserialize;
use serde_json;
use std::cell::Cell;
use std::collections::HashMap;
use std::fs;
use std::path::Path;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Mutex, OnceLock};
use std::time::Instant;

// Stage 3 scaffold: Hard planner with configurable branch limit and (future) depth/time caps.
// For now, it orders by heuristic and considers the top N branches.
pub struct PlayPlannerHard;

#[derive(Debug, Clone, Copy)]
#[allow(dead_code)]
pub struct SearchConfig {
    pub branch_limit: usize,
    pub max_depth: u8,    // reserved for future two-ply
    pub time_cap_ms: u32, // reserved for future wall clock cap
    pub next_branch_limit: usize,
    pub early_cutoff_margin: i32,
    pub min_scan_before_cutoff: usize,
    pub high_budget: bool,
    pub depth2_topk: usize,
    pub depth2_min_limit_ms: u32,
}

impl Default for SearchConfig {
    fn default() -> Self {
        Self {
            branch_limit: 6,
            max_depth: 1,
            time_cap_ms: 10,
            next_branch_limit: 3,
            early_cutoff_margin: 300,
            min_scan_before_cutoff: 1,
            high_budget: false,
            depth2_topk: 0,
            depth2_min_limit_ms: 0,
        }
    }
}

#[derive(Clone, Copy)]
struct ForcedDepth2Prepass {
    card: Card,
    cont: i32,
    depth2_bonus: i32,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
enum MixTag {
    Snnh,
    Shsh,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct MixSeatHint {
    mix: MixTag,
    seat: Option<PlayerPosition>,
}

impl MixSeatHint {
    fn matches(&self, seat: PlayerPosition) -> bool {
        self.seat.map_or(true, |target| target == seat)
    }
}

impl MixTag {
    fn from_str(value: &str) -> Option<Self> {
        match value {
            "snnh" => Some(MixTag::Snnh),
            "shsh" => Some(MixTag::Shsh),
            _ => None,
        }
    }
}

#[derive(Debug, Deserialize)]
struct RawSeatSchedule {
    #[serde(default)]
    limits_ms: Vec<u32>,
    #[serde(default)]
    continuation_scale: Vec<f32>,
    #[serde(default)]
    suggested_phaseb_topk: Option<usize>,
    #[serde(default)]
    suggested_ab_margin: Option<i32>,
}

#[derive(Clone, Debug)]
struct SeatSchedule {
    limits_ms: Vec<u32>,
    continuation_scale: Vec<i32>,
    suggested_phaseb_topk: Option<usize>,
    suggested_ab_margin: Option<i32>,
}

impl SeatSchedule {
    fn from_raw(raw: RawSeatSchedule) -> Option<Self> {
        let len = raw.limits_ms.len().min(raw.continuation_scale.len());
        let mut pairs: Vec<(u32, i32)> = raw
            .limits_ms
            .into_iter()
            .zip(raw.continuation_scale.into_iter())
            .take(len)
            .map(|(limit, scale)| (limit, scale.round() as i32))
            .collect();
        if !pairs.is_empty() {
            pairs.sort_by_key(|(limit, _)| *limit);
        }
        let (limits_ms, continuation_scale): (Vec<u32>, Vec<i32>) = pairs.into_iter().unzip();
        if limits_ms.is_empty()
            && continuation_scale.is_empty()
            && raw.suggested_phaseb_topk.is_none()
            && raw.suggested_ab_margin.is_none()
        {
            return None;
        }
        Some(Self {
            limits_ms,
            continuation_scale,
            suggested_phaseb_topk: raw.suggested_phaseb_topk,
            suggested_ab_margin: raw.suggested_ab_margin,
        })
    }

    fn hints_for(&self, limit_ms: u32) -> ScheduleHints {
        ScheduleHints {
            continuation_scale: self.scale_for(limit_ms),
            phaseb_topk: self.suggested_phaseb_topk,
            ab_margin: self.suggested_ab_margin,
        }
    }

    fn scale_for(&self, limit_ms: u32) -> Option<i32> {
        if self.limits_ms.is_empty() || self.continuation_scale.is_empty() {
            return None;
        }
        let mut idx = 0usize;
        for (i, &limit) in self.limits_ms.iter().enumerate() {
            if limit_ms >= limit {
                idx = i;
            } else {
                break;
            }
        }
        self.continuation_scale.get(idx).copied()
    }
}

#[derive(Default)]
struct ContinuationSchedule {
    mixes: HashMap<MixTag, [Option<SeatSchedule>; 4]>,
}

impl ContinuationSchedule {
    fn insert(&mut self, mix: MixTag, seat: PlayerPosition, schedule: SeatSchedule) {
        let entry = self.mixes.entry(mix).or_insert_with(|| Default::default());
        entry[seat.index()] = Some(schedule);
    }

    fn lookup(&self, mix: MixTag, seat: PlayerPosition) -> Option<&SeatSchedule> {
        self.mixes
            .get(&mix)
            .and_then(|arr| arr[seat.index()].as_ref())
    }
}

#[derive(Clone, Copy, Default)]
struct ScheduleHints {
    continuation_scale: Option<i32>,
    phaseb_topk: Option<usize>,
    ab_margin: Option<i32>,
}

impl ScheduleHints {
    fn is_empty(self) -> bool {
        self.continuation_scale.is_none() && self.phaseb_topk.is_none() && self.ab_margin.is_none()
    }
}

fn parse_seat_hint(label: &str) -> Option<PlayerPosition> {
    match label {
        "north" | "n" => Some(PlayerPosition::North),
        "east" | "e" => Some(PlayerPosition::East),
        "south" | "s" => Some(PlayerPosition::South),
        "west" | "w" => Some(PlayerPosition::West),
        _ => None,
    }
}

fn search_mix_hint() -> Option<MixSeatHint> {
    let raw = std::env::var("MDH_SEARCH_MIX_HINT").ok()?;
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        return None;
    }
    let mut parts = trimmed.splitn(2, ':');
    let mix_part = parts.next()?.trim().to_ascii_lowercase();
    let seat_part = parts
        .next()
        .map(|s| s.trim().to_ascii_lowercase())
        .filter(|s| !s.is_empty());
    let mix = MixTag::from_str(&mix_part)?;
    let seat = seat_part.as_deref().and_then(parse_seat_hint);
    Some(MixSeatHint { mix, seat })
}

fn continuation_schedule_data() -> &'static Option<ContinuationSchedule> {
    static SCHEDULE: OnceLock<Option<ContinuationSchedule>> = OnceLock::new();
    SCHEDULE.get_or_init(load_continuation_schedule)
}

fn load_continuation_schedule() -> Option<ContinuationSchedule> {
    let path = std::env::var("MDH_CONT_SCHEDULE_PATH")
        .ok()
        .filter(|s| !s.trim().is_empty())
        .unwrap_or_else(|| "tmp/continuation_fit_latest.json".to_string());
    let path = Path::new(&path);
    let contents = fs::read_to_string(path).ok()?;
    let raw: HashMap<String, HashMap<String, RawSeatSchedule>> =
        serde_json::from_str(&contents).ok()?;
    let mut schedule = ContinuationSchedule::default();
    for (mix_label, seats) in raw.into_iter() {
        let Some(mix_tag) = MixTag::from_str(mix_label.trim()) else {
            continue;
        };
        for (seat_label, data) in seats.into_iter() {
            let Some(seat) = parse_seat_hint(seat_label.trim()) else {
                continue;
            };
            if let Some(seat_schedule) = SeatSchedule::from_raw(data) {
                schedule.insert(mix_tag, seat, seat_schedule);
            }
        }
    }
    Some(schedule)
}

fn schedule_hints_for(seat: PlayerPosition, limit_ms: Option<u32>) -> Option<ScheduleHints> {
    let limit = limit_ms?;
    let hint = search_mix_hint()?;
    let schedule = continuation_schedule_data().as_ref()?;
    let preferred_seat = hint.seat.unwrap_or(seat);
    let seat_schedule = schedule
        .lookup(hint.mix, preferred_seat)
        .or_else(|| schedule.lookup(hint.mix, seat))?;
    let hints = seat_schedule.hints_for(limit);
    if hints.is_empty() { None } else { Some(hints) }
}

fn force_depth2_extra(ctx: &BotContext<'_>, limit_ms: Option<u32>) -> bool {
    let limit = limit_ms.unwrap_or(0);
    if limit < 14_000 {
        return false;
    }
    let Some(hint) = search_mix_hint() else {
        return false;
    };
    match hint.mix {
        MixTag::Snnh => ctx.seat == PlayerPosition::North,
        MixTag::Shsh => ctx.seat == PlayerPosition::South,
    }
}

impl PlayPlannerHard {
    fn config() -> SearchConfig {
        // Allow env override of branch limit in early scaffolding.
        let bl = std::env::var("MDH_HARD_BRANCH_LIMIT")
            .ok()
            .and_then(|s| s.parse::<usize>().ok())
            .unwrap_or(6);
        let cap = std::env::var("MDH_HARD_TIME_CAP_MS")
            .ok()
            .and_then(|s| s.parse::<u32>().ok())
            .unwrap_or(10);
        let nbl = std::env::var("MDH_HARD_NEXT_BRANCH_LIMIT")
            .ok()
            .and_then(|s| s.parse::<usize>().ok())
            .unwrap_or(3);
        let cutoff = std::env::var("MDH_HARD_EARLY_CUTOFF_MARGIN")
            .ok()
            .and_then(|s| s.parse::<i32>().ok())
            .unwrap_or(300);
        SearchConfig {
            branch_limit: bl,
            time_cap_ms: cap,
            next_branch_limit: nbl,
            early_cutoff_margin: cutoff,
            min_scan_before_cutoff: std::env::var("MDH_HARD_MIN_SCAN")
                .ok()
                .and_then(|s| s.parse::<usize>().ok())
                .unwrap_or(1),
            ..SearchConfig::default()
        }
    }

    fn config_for_ctx(ctx: &BotContext<'_>, limit: Option<&DecisionLimit<'_>>) -> SearchConfig {
        let mut cfg = Self::config();
        let mut cutoff_scale = 1.0f32;
        let mut min_scan = cfg.min_scan_before_cutoff;
        let mut high_budget = cfg.high_budget;
        let mut depth2_topk = 0usize;
        let mut depth2_min_limit_ms = 0u32;
        let limit_ms_remaining = limit.and_then(|limit| limit.remaining_millis());
        if let Some(ms) = limit_ms_remaining {
            let mut branch_delta = 0usize;
            let mut next_delta = 0usize;
            let mut margin_scale = 1.0f32;

            if ms >= 20_000 {
                branch_delta = 5;
                next_delta = 3;
                margin_scale = 0.6;
            } else if ms >= 15_000 {
                branch_delta = 4;
                next_delta = 2;
                margin_scale = 0.7;
            } else if ms >= 12_000 {
                branch_delta = 3;
                next_delta = 2;
                margin_scale = 0.8;
            } else if ms >= 9_000 {
                branch_delta = 2;
                next_delta = 1;
            } else if ms >= 6_000 {
                branch_delta = 1;
            }

            if ms >= 10_000 {
                high_budget = true;
            }

            if ms >= 20_000 {
                depth2_topk = 3;
                depth2_min_limit_ms = 20_000;
            } else if ms >= 15_000 {
                depth2_topk = 2;
                depth2_min_limit_ms = 15_000;
            } else if ms >= 12_000 {
                depth2_topk = 1;
                depth2_min_limit_ms = 12_000;
            }

            if ms >= 18_000 {
                min_scan = min_scan.max(6);
            } else if ms >= 15_000 {
                min_scan = min_scan.max(5);
            } else if ms >= 12_000 {
                min_scan = min_scan.max(4);
            } else if ms >= 9_000 {
                min_scan = min_scan.max(3);
            } else if ms >= 6_000 {
                min_scan = min_scan.max(2);
            } else if ms <= 3_500 {
                let shrink = if ms <= 2_000 { 3 } else { 2 };
                cfg.branch_limit = cfg.branch_limit.saturating_sub(shrink).max(2);
                cfg.next_branch_limit = cfg.next_branch_limit.saturating_sub(1).max(1);
                min_scan = min_scan.min(cfg.branch_limit);
            }

            if branch_delta > 0 {
                cfg.branch_limit += branch_delta;
            }
            if next_delta > 0 {
                cfg.next_branch_limit += next_delta;
            }
            if margin_scale < 1.0 {
                cutoff_scale *= margin_scale;
            }
        }
        let snapshot = snapshot_scores(&ctx.scores);
        let moon_pressure = detect_moon_pressure(ctx, &snapshot);
        let my_score = ctx.scores.score(ctx.seat);
        let leader_gap = snapshot.max_score.saturating_sub(my_score);
        if leader_gap <= 5 {
            cfg.branch_limit += 1;
        }
        if my_score >= 75 {
            cfg.branch_limit += 1;
            cfg.next_branch_limit += 1;
            min_scan = min_scan.max(3);
        }
        let trailing = snapshot.max_player != ctx.seat && leader_gap > 0;
        if trailing && leader_gap >= 8 {
            cutoff_scale *= if leader_gap >= 15 { 0.55 } else { 0.7 };
            cfg.next_branch_limit += 1;
            min_scan = min_scan.max(3);
        }
        if trailing && leader_gap >= 18 {
            cfg.branch_limit += 1;
            min_scan = min_scan.max(4);
        }
        let trick_penalty = ctx.round.current_trick().penalty_total() as i32;
        if trick_penalty >= 13 {
            cutoff_scale *= 0.85;
            cfg.next_branch_limit += 1;
            min_scan = min_scan.max(3);
        }
        if ctx.seat == PlayerPosition::North {
            cfg.branch_limit += 1;
            min_scan = min_scan.max(3);
        }
        if matches!(ctx.seat, PlayerPosition::North | PlayerPosition::East) {
            cfg.next_branch_limit += 1;
            min_scan = min_scan.max(3);
        }
        if moon_pressure {
            min_scan = min_scan.max(4);
        }
        if high_budget {
            cfg.next_branch_limit += 1;
            cfg.branch_limit += 1;
            min_scan = min_scan.max(4);
        }
        if min_scan >= 4 {
            cfg.next_branch_limit += 1;
        }
        if min_scan >= 5 {
            cfg.next_branch_limit += 1;
        }
        let branch_floor: usize =
            if matches!(ctx.seat, PlayerPosition::North | PlayerPosition::East) {
                4
            } else {
                3
            };
        if cfg.branch_limit < branch_floor {
            cfg.branch_limit = branch_floor;
        }
        let next_floor: usize = if matches!(ctx.seat, PlayerPosition::North | PlayerPosition::East)
        {
            2
        } else {
            1
        };
        if cfg.next_branch_limit < next_floor {
            cfg.next_branch_limit = next_floor;
        }
        if cfg.next_branch_limit > cfg.branch_limit {
            cfg.next_branch_limit = cfg.branch_limit;
        }
        if depth2_topk > 0 && matches!(ctx.seat, PlayerPosition::North | PlayerPosition::East) {
            depth2_topk = depth2_topk.saturating_add(1);
        }
        if let (Some(ms), Some(hint)) = (limit_ms_remaining, search_mix_hint()) {
            if ms >= 12_000 {
                match hint.mix {
                    MixTag::Snnh => {
                        if hint.matches(PlayerPosition::North) || hint.matches(PlayerPosition::East)
                        {
                            let forced = if ms >= 15_000 { 2 } else { 1 };
                            depth2_topk = depth2_topk.max(forced);
                            depth2_min_limit_ms = depth2_min_limit_ms.max(12_000);
                        }
                    }
                    MixTag::Shsh => {
                        if hint.matches(PlayerPosition::South)
                            || hint.matches(PlayerPosition::East)
                            || hint.matches(PlayerPosition::West)
                        {
                            let forced = if ms >= 15_000 { 2 } else { 1 };
                            depth2_topk = depth2_topk.max(forced);
                            depth2_min_limit_ms = depth2_min_limit_ms.max(12_000);
                        }
                    }
                }
            }
        }
        if cutoff_scale < 1.0 {
            let scaled = ((cfg.early_cutoff_margin as f32) * cutoff_scale).round() as i32;
            cfg.early_cutoff_margin = scaled.max(75);
        }
        cfg.min_scan_before_cutoff = min_scan.min(cfg.branch_limit).max(1);
        cfg.high_budget = high_budget;
        cfg.depth2_topk = depth2_topk;
        cfg.depth2_min_limit_ms = depth2_min_limit_ms;
        cfg
    }

    fn apply_schedule_hints(
        seat: PlayerPosition,
        limit_ms: Option<u32>,
        mut phaseb_topk: usize,
        mut ab_margin: i32,
        high_budget: bool,
        depth2_enabled: bool,
        schedule_hints: Option<ScheduleHints>,
    ) -> (usize, i32) {
        if let Some(ms) = limit_ms {
            if ms >= 20_000 {
                phaseb_topk = phaseb_topk.saturating_add(2);
                if ab_margin > 0 {
                    ab_margin = ((ab_margin as f32) * 0.75).round() as i32;
                }
            } else if ms >= 15_000 {
                phaseb_topk = phaseb_topk.saturating_add(1);
                if ab_margin > 0 {
                    ab_margin = ((ab_margin as f32) * 0.8).round() as i32;
                }
            } else if ms >= 10_000 {
                if ab_margin > 0 {
                    ab_margin = ((ab_margin as f32) * 0.9).round() as i32;
                }
            }
            if matches!(seat, PlayerPosition::North | PlayerPosition::East) && ms >= 12_000 {
                phaseb_topk = phaseb_topk.saturating_add(1);
            }
        }
        if let Some(limit) = limit_ms {
            if let Some(hint) = search_mix_hint() {
                let target = hint.seat.unwrap_or(seat);
                match hint.mix {
                    MixTag::Snnh => {
                        if matches!(target, PlayerPosition::North | PlayerPosition::East) {
                            if limit >= 15_000 && ab_margin > 0 {
                                ab_margin = ((ab_margin as f32) * 0.65).round() as i32;
                                ab_margin = ab_margin.max(35);
                            }
                            if limit >= 18_000 && ab_margin > 0 {
                                ab_margin = ab_margin.min(30);
                            }
                            if depth2_enabled {
                                phaseb_topk = phaseb_topk.max(4);
                            }
                        }
                    }
                    MixTag::Shsh => {
                        if matches!(
                            target,
                            PlayerPosition::East | PlayerPosition::South | PlayerPosition::West
                        ) {
                            if limit >= 15_000 && ab_margin > 0 {
                                ab_margin = ((ab_margin as f32) * 0.6).round() as i32;
                                ab_margin = ab_margin.min(90).max(25);
                            }
                            if limit >= 18_000 && ab_margin > 0 {
                                ab_margin = ab_margin.min(70);
                            }
                            if limit >= 14_000 {
                                phaseb_topk = phaseb_topk.saturating_add(2);
                            }
                        }
                    }
                }
            }
        }
        if depth2_enabled {
            phaseb_topk = phaseb_topk.saturating_add(1);
            if ab_margin > 0 {
                ab_margin = ((ab_margin as f32) * 0.9).round() as i32;
            }
        }
        if high_budget && ab_margin > 0 {
            ab_margin = ab_margin.max(75);
        }
        if let Some(hints) = schedule_hints {
            if let Some(topk) = hints.phaseb_topk {
                phaseb_topk = phaseb_topk.max(topk);
            }
            if let Some(target_ab) = hints.ab_margin {
                if ab_margin > 0 {
                    ab_margin = ab_margin.min(target_ab);
                } else {
                    ab_margin = target_ab;
                }
            }
        }
        (phaseb_topk, ab_margin)
    }

    fn continuation_scale_permil(base_margin: i32, current_margin: i32, high_budget: bool) -> i32 {
        const BASE: i32 = 1000;
        let drop = if base_margin > 0 && current_margin > 0 {
            let ratio = (current_margin as f32) / (base_margin as f32);
            (1.0 - ratio).max(0.0)
        } else {
            0.0
        };
        let mut scale = BASE + (drop * 500.0).round() as i32;
        if high_budget {
            scale = scale.max(1200);
        }
        scale.clamp(1000, 1700)
    }

    fn apply_continuation_scale(value: i32, scale_permil: i32) -> i32 {
        if scale_permil == 1000 {
            return value;
        }
        (((value as i64) * (scale_permil as i64)) / 1000) as i32
    }

    fn seat_cont_bias(
        seat: PlayerPosition,
        leader_gap: i32,
        moon_pressure: bool,
        _limit_ms: Option<u32>,
        _depth2_enabled: bool,
    ) -> (i32, i32, i32) {
        let mut feed_bias = 1000;
        let mut self_bias = 1000;
        let mut moon_bias = 1000;

        if leader_gap > 0 {
            feed_bias += (leader_gap.min(15) as i32) * 20;
            self_bias -= (leader_gap.min(15) as i32) * 12;
        } else if leader_gap < 0 {
            let lead = (-leader_gap).min(15) as i32;
            feed_bias -= lead * 15;
            self_bias += lead * 10;
        }

        match seat {
            PlayerPosition::East | PlayerPosition::South => {
                feed_bias += 80;
                self_bias -= 50;
            }
            PlayerPosition::North | PlayerPosition::West => {
                feed_bias += 40;
                self_bias -= 30;
            }
        }

        if moon_pressure {
            moon_bias += 200;
        }

        feed_bias = feed_bias.clamp(700, 1600);
        self_bias = self_bias.clamp(700, 1400);
        moon_bias = moon_bias.clamp(800, 1800);
        (feed_bias, self_bias, moon_bias)
    }

    fn apply_play_adviser_bias(entries: &mut Vec<(Card, i32)>, ctx: &BotContext<'_>) {
        if entries.is_empty() {
            return;
        }
        let mut touched = false;
        for (card, score) in entries.iter_mut() {
            let bias = super::play_bias(*card, ctx);
            if bias != 0 {
                *score = score.saturating_add(bias);
                touched = true;
            }
        }
        if touched && debug_enabled() {
            eprintln!(
                "mdhearts: adviser bias applied ({} candidates)",
                entries.len()
            );
        }
    }

    pub fn choose(legal: &[Card], ctx: &BotContext<'_>) -> Option<Card> {
        Self::choose_with_limit(legal, ctx, None)
    }

    pub fn choose_with_limit(
        legal: &[Card],
        ctx: &BotContext<'_>,
        limit: Option<&DecisionLimit<'_>>,
    ) -> Option<Card> {
        if legal.is_empty() {
            return None;
        }
        if limit.map_or(false, |limit| limit.expired()) {
            return None;
        }
        crate::telemetry::hard::record_pre_decision(ctx.seat, ctx.tracker, ctx.difficulty);
        let limit_ms_remaining = limit.and_then(|limit| limit.remaining_millis());
        let schedule_hints = schedule_hints_for(ctx.seat, limit_ms_remaining);
        let cfg = Self::config_for_ctx(ctx, limit);
        let deterministic = std::env::var("MDH_HARD_DETERMINISTIC")
            .map(|v| v == "1" || v.eq_ignore_ascii_case("true") || v.eq_ignore_ascii_case("on"))
            .unwrap_or(false);
        let step_cap = std::env::var("MDH_HARD_TEST_STEPS")
            .ok()
            .and_then(|s| s.parse::<usize>().ok());
        // Rank by heuristic (fast), keep top-N, then apply a tiny continuation bonus via a 1-ply trick rollout.
        let mut explained =
            PlayPlanner::explain_candidates_with_limit(legal, ctx, limit_ms_remaining);
        Self::apply_play_adviser_bias(&mut explained, ctx);
        let planner_nudge_hits = super::play::take_hard_nudge_hits();
        explained.sort_by(|a, b| b.1.cmp(&a.1));
        let best_base = explained.first().map(|x| x.1).unwrap_or(0);
        let boost_gap = parse_env_i32("MDH_HARD_CONT_BOOST_GAP").unwrap_or(0);
        let boost_factor = parse_env_i32("MDH_HARD_CONT_BOOST_FACTOR").unwrap_or(1);
        let snapshot = snapshot_scores(&ctx.scores);
        let moon_pressure = detect_moon_pressure(ctx, &snapshot);
        let mut best: Option<(Card, i32)> = None;
        let start = Instant::now();
        let mut budget = Budget::new(
            cfg.time_cap_ms,
            deterministic,
            step_cap,
            limit.and_then(|limit| limit.deadline),
            limit.and_then(|limit| limit.cancel),
        );
        let mut scanned = 0usize;
        let (tier, leverage_score, limits) = effective_limits(ctx);
        let my_score = ctx.scores.score(ctx.seat);
        let trailing = snapshot.max_player != ctx.seat && snapshot.max_score > my_score;
        let mut phaseb_topk = limits.phaseb_topk;
        let mut ab_margin = limits.ab_margin;
        let base_ab_margin = ab_margin;
        let high_scan = cfg.min_scan_before_cutoff;
        let high_budget_mode = cfg.high_budget;
        if high_scan >= 4 {
            phaseb_topk = phaseb_topk.saturating_add(1);
            if ab_margin > 0 {
                ab_margin = ((ab_margin as f32) * 0.9).round() as i32;
            }
        }
        if high_scan >= 5 {
            phaseb_topk = phaseb_topk.saturating_add(1);
            if ab_margin > 0 {
                ab_margin = ((ab_margin as f32) * 0.85).round() as i32;
            }
        }
        if trailing && high_scan >= 3 {
            phaseb_topk = phaseb_topk.saturating_add(1);
            if ab_margin > 0 {
                ab_margin = ((ab_margin as f32) * 0.85).round() as i32;
            }
        }
        if trailing && high_scan >= 5 && ab_margin > 0 {
            ab_margin = ((ab_margin as f32) * 0.8).round() as i32;
        }
        if moon_pressure {
            phaseb_topk = phaseb_topk.saturating_add(1 + usize::from(high_scan >= 4));
            if ab_margin > 0 {
                let scale = if snapshot.max_score >= 90 { 0.65 } else { 0.75 };
                ab_margin = ((ab_margin as f32) * scale).round() as i32;
                ab_margin = ab_margin.max(30);
            }
        }
        if high_budget_mode {
            phaseb_topk = phaseb_topk.saturating_add(1);
            if ab_margin > 0 {
                ab_margin = ((ab_margin as f32) * 0.85).round() as i32;
            }
        }
        if high_budget_mode && moon_pressure {
            phaseb_topk = phaseb_topk.saturating_add(1);
            if ab_margin > 0 {
                ab_margin = ((ab_margin as f32) * 0.7).round() as i32;
                ab_margin = ab_margin.max(25);
            }
        }
        if ab_margin > 0 {
            ab_margin = ab_margin.max(50);
        }
        let mut continuation_scale_permil =
            Self::continuation_scale_permil(base_ab_margin, ab_margin, high_budget_mode);
        if let Some(hints) = schedule_hints {
            if let Some(scale) = hints.continuation_scale {
                continuation_scale_permil = scale;
            }
        }
        if matches!(ctx.difficulty, super::BotDifficulty::FutureHard) {
            let seat_bonus = match ctx.seat {
                PlayerPosition::East => {
                    if high_budget_mode {
                        200
                    } else if trailing {
                        125
                    } else {
                        75
                    }
                }
                PlayerPosition::North => {
                    if moon_pressure || trailing {
                        100
                    } else {
                        0
                    }
                }
                _ => 0,
            };
            if seat_bonus > 0 {
                continuation_scale_permil = (continuation_scale_permil + seat_bonus).min(1850);
            }
        }
        let force_depth2 = force_depth2_extra(ctx, limit_ms_remaining);
        let depth2_enabled = cfg.depth2_topk > 0 || force_depth2;
        let (mut phaseb_topk, ab_margin) = Self::apply_schedule_hints(
            ctx.seat,
            limit_ms_remaining,
            phaseb_topk,
            ab_margin,
            high_budget_mode,
            depth2_enabled,
            schedule_hints,
        );
        phaseb_topk = phaseb_topk.min(cfg.branch_limit).max(2);
        if depth2_enabled {
            let mut depth2_cap = cfg.depth2_topk.saturating_add(3);
            if depth2_cap == 0 {
                depth2_cap = 2;
            }
            phaseb_topk = phaseb_topk.min(depth2_cap.max(2));
            if cfg.depth2_topk > 0 {
                phaseb_topk = phaseb_topk.max(cfg.depth2_topk);
            }
        }
        let mut phase_b_candidates = 0usize;
        let mut phase_c_probes = 0usize;
        // capture tiny-next3 counter delta for this decision
        let n3_before = NEXT3_TINY_COUNT.with(|c| c.get());
        let dp_before = ENDGAME_DP_COUNT.with(|c| c.get());
        let mut forced_depth2_cache = if force_depth2 {
            Self::forced_depth2_prepass(
                &explained,
                ctx,
                snapshot.max_player,
                cfg,
                continuation_scale_permil,
                limit_ms_remaining,
                tier,
                &mut budget,
                start,
                &mut phase_c_probes,
                depth2_enabled,
            )
        } else {
            None
        };
        let mut iter = explained
            .into_iter()
            .take(cfg.branch_limit)
            .enumerate()
            .peekable();
        while let Some((idx, (card, base))) = iter.next() {
            let allow_cached = force_depth2
                && idx == 0
                && forced_depth2_cache
                    .as_ref()
                    .map_or(false, |cache| cache.card == card);
            if budget.should_stop() && !allow_cached {
                break;
            }
            if ab_margin > 0 {
                if let Some((_, alpha)) = best {
                    if base + ab_margin < alpha {
                        if debug_enabled() {
                            eprintln!(
                                "mdhearts: hard ab-skip {} base={} < alpha-{}",
                                card, base, ab_margin
                            );
                        }
                        scanned += 1;
                        budget.tick();
                        continue;
                    }
                }
            }
            let depth2_candidate =
                (cfg.depth2_topk > 0 && idx < cfg.depth2_topk && phaseb_topk != 0)
                    || (force_depth2 && idx == 0);
            let mut depth2_resolution: Option<TrickResolution> = None;
            let mut cached_depth2_bonus: Option<i32> = None;
            let cont_raw = if phaseb_topk == 0 || idx < phaseb_topk {
                phase_b_candidates = phase_b_candidates.saturating_add(1);
                let before = budget.probe_calls;
                let allow_next3 = matches!(tier, Tier::Wide);
                let v = if depth2_candidate {
                    if allow_cached {
                        let cache = forced_depth2_cache.take().unwrap();
                        cached_depth2_bonus = Some(cache.depth2_bonus);
                        cache.cont
                    } else {
                        match rollout_current_trick_with_resolution(
                            card,
                            ctx,
                            snapshot.max_player,
                            &mut budget,
                            start,
                            allow_next3,
                            continuation_scale_permil,
                            limit_ms_remaining,
                            depth2_enabled,
                        ) {
                            Some((value, res)) => {
                                depth2_resolution = Some(res);
                                value
                            }
                            None => 0,
                        }
                    }
                } else {
                    Self::rollout_current_trick(
                        card,
                        ctx,
                        snapshot.max_player,
                        &mut budget,
                        start,
                        allow_next3,
                        continuation_scale_permil,
                        limit_ms_remaining,
                        depth2_enabled,
                    )
                };
                phase_c_probes += budget.probe_calls.saturating_sub(before);
                v
            } else {
                0
            };
            let cont = if boost_gap > 0 && boost_factor > 1 && (best_base - base) <= boost_gap {
                cont_raw.saturating_mul(boost_factor)
            } else {
                cont_raw
            };
            let mut depth2_bonus = 0;
            if depth2_candidate {
                if let Some(bonus) = cached_depth2_bonus {
                    depth2_bonus = bonus;
                } else if let Some(res) = depth2_resolution.as_ref() {
                    depth2_bonus = evaluate_depth2_bonus(
                        res,
                        ctx,
                        &cfg,
                        &mut budget,
                        start,
                        continuation_scale_permil,
                        snapshot.max_player,
                        limit_ms_remaining,
                        depth2_enabled,
                    );
                }
            }
            let total = base + cont + depth2_bonus;
            if debug_enabled() {
                eprintln!(
                    "mdhearts: hard cand {} base={} cont={} total={}",
                    card, base, cont, total
                );
            }
            match best {
                None => best = Some((card, total)),
                Some((bc, bs)) => {
                    if total > bs {
                        best = Some((card, total));
                    } else if total == bs
                        && (card.suit as u8, card.rank.value()) < (bc.suit as u8, bc.rank.value())
                    {
                        best = Some((card, total));
                    }
                }
            }
            scanned += 1;
            budget.tick();
            // Early cutoff: if the next base score cannot overcome our current best even with a safety margin, stop.
            if scanned >= cfg.min_scan_before_cutoff {
                if let Some((_, best_total)) = best {
                    if let Some((_, (next_card, next_base))) = iter.peek() {
                        let safe_cap = if cfg.high_budget {
                            (weights().cont_cap as f32 * 0.7) as i32
                        } else {
                            weights().cont_cap
                        };
                        let safety_margin = if safe_cap > 0 {
                            safe_cap
                        } else {
                            cfg.early_cutoff_margin
                        };
                        if *next_base + safety_margin < best_total {
                            if debug_enabled() {
                                eprintln!(
                                    "mdhearts: hard early cutoff at candidate {} (next_base={} + margin {} < best_total={})",
                                    next_card, next_base, safety_margin, best_total
                                );
                            }
                            break;
                        }
                    }
                }
            }
            if budget.timed_out(start) {
                if debug_enabled() {
                    eprintln!(
                        "mdhearts: hard cap reached ({} ms), scanned {}",
                        cfg.time_cap_ms, scanned
                    );
                }
                break;
            }
        }
        let utilization = budget.utilization_percent(start);
        let n3_after = NEXT3_TINY_COUNT.with(|c| c.get());
        let dp_after = ENDGAME_DP_COUNT.with(|c| c.get());
        let planner_nudge_trace = super::play::take_hard_nudge_trace_summary();
        set_last_stats(Stats {
            scanned,
            elapsed_ms: start.elapsed().as_millis() as u32,
            scanned_phase_a: scanned.saturating_sub(phase_b_candidates),
            scanned_phase_b: phase_b_candidates,
            scanned_phase_c: phase_c_probes,
            leverage_score,
            tier,
            limits_in_effect: limits,
            utilization,
            depth2_samples: budget.depth2_samples(),
            wide_boost_feed_permil: parse_env_i32("MDH_HARD_WIDE_PERMIL_BOOST_FEED").unwrap_or(0),
            wide_boost_self_permil: parse_env_i32("MDH_HARD_WIDE_PERMIL_BOOST_SELFCAP")
                .unwrap_or(0),
            cont_cap: weights().cont_cap,
            continuation_scale_permil,
            next3_tiny_hits: n3_after.saturating_sub(n3_before),
            endgame_dp_hits: dp_after.saturating_sub(dp_before),
            planner_nudge_hits,
            planner_nudge_trace,
            mix_hint_bias: super::play::take_mix_hint_bias_stats(),
            controller_bias_delta: ctx.controller_bias_delta,
        });
        best.map(|(c, _)| c)
    }

    pub fn explain_candidates(legal: &[Card], ctx: &BotContext<'_>) -> Vec<(Card, i32)> {
        let cfg = Self::config();
        let deterministic = std::env::var("MDH_HARD_DETERMINISTIC")
            .map(|v| v == "1" || v.eq_ignore_ascii_case("true") || v.eq_ignore_ascii_case("on"))
            .unwrap_or(false);
        let step_cap = std::env::var("MDH_HARD_TEST_STEPS")
            .ok()
            .and_then(|s| s.parse::<usize>().ok());
        let mut v = PlayPlanner::explain_candidates(legal, ctx);
        Self::apply_play_adviser_bias(&mut v, ctx);
        let planner_nudge_hits = super::play::take_hard_nudge_hits();
        v.sort_by(|a, b| b.1.cmp(&a.1));
        let best_base = v.first().map(|x| x.1).unwrap_or(0);
        let boost_gap = parse_env_i32("MDH_HARD_CONT_BOOST_GAP").unwrap_or(0);
        let boost_factor = parse_env_i32("MDH_HARD_CONT_BOOST_FACTOR").unwrap_or(1);
        let snapshot = snapshot_scores(&ctx.scores);
        let start = Instant::now();
        let mut budget = Budget::new(cfg.time_cap_ms, deterministic, step_cap, None, None);
        let mut out = Vec::new();
        let (tier, leverage_score, limits) = effective_limits(ctx);
        let phaseb_topk = limits.phaseb_topk;
        let mut scanned = 0usize;
        let mut phase_b_candidates = 0usize;
        let mut phase_c_probes = 0usize;
        let continuation_scale_permil = 1000;
        for (idx, (card, base)) in v.into_iter().take(cfg.branch_limit).enumerate() {
            if budget.should_stop() {
                break;
            }
            let cont_raw = if phaseb_topk == 0 || idx < phaseb_topk {
                phase_b_candidates = phase_b_candidates.saturating_add(1);
                let before = budget.probe_calls;
                let v = Self::rollout_current_trick(
                    card,
                    ctx,
                    snapshot.max_player,
                    &mut budget,
                    start,
                    false,
                    continuation_scale_permil,
                    None,
                    false,
                );
                phase_c_probes += budget.probe_calls.saturating_sub(before);
                v
            } else {
                0
            };
            let cont = if boost_gap > 0 && boost_factor > 1 && (best_base - base) <= boost_gap {
                cont_raw.saturating_mul(boost_factor)
            } else {
                cont_raw
            };
            let total = base + cont;
            if debug_enabled() {
                eprintln!(
                    "mdhearts: hard explain {} base={} cont={} total={}",
                    card, base, cont, total
                );
            }
            out.push((card, total));
            scanned += 1;
            budget.tick();
            if budget.timed_out(start) {
                if debug_enabled() {
                    eprintln!(
                        "mdhearts: hard explain cap reached ({} ms)",
                        cfg.time_cap_ms
                    );
                }
                break;
            }
        }
        let utilization = budget.utilization_percent(start);
        let planner_nudge_trace = super::play::take_hard_nudge_trace_summary();
        set_last_stats(Stats {
            scanned,
            elapsed_ms: start.elapsed().as_millis() as u32,
            scanned_phase_a: scanned.saturating_sub(phase_b_candidates),
            scanned_phase_b: phase_b_candidates,
            scanned_phase_c: phase_c_probes,
            leverage_score,
            tier,
            limits_in_effect: limits,
            utilization,
            depth2_samples: budget.depth2_samples(),
            wide_boost_feed_permil: parse_env_i32("MDH_HARD_WIDE_PERMIL_BOOST_FEED").unwrap_or(0),
            wide_boost_self_permil: parse_env_i32("MDH_HARD_WIDE_PERMIL_BOOST_SELFCAP")
                .unwrap_or(0),
            cont_cap: weights().cont_cap,
            continuation_scale_permil,
            next3_tiny_hits: 0,
            endgame_dp_hits: 0,
            planner_nudge_hits,
            planner_nudge_trace,
            mix_hint_bias: super::play::take_mix_hint_bias_stats(),
            controller_bias_delta: ctx.controller_bias_delta,
        });
        out
    }

    fn forced_depth2_prepass(
        explained: &[(Card, i32)],
        ctx: &BotContext<'_>,
        leader_target: PlayerPosition,
        cfg: SearchConfig,
        continuation_scale_permil: i32,
        limit_ms_remaining: Option<u32>,
        tier: Tier,
        budget: &mut Budget,
        start: Instant,
        phase_c_probes: &mut usize,
        depth2_enabled: bool,
    ) -> Option<ForcedDepth2Prepass> {
        if !depth2_enabled {
            return None;
        }
        let (card, _) = explained.first().copied()?;
        if budget.should_stop() || budget.timed_out(start) {
            return None;
        }
        let prev_step_cap = budget.step_cap;
        let relax_cap = budget.deterministic && budget.step_cap.is_some();
        if relax_cap {
            budget.step_cap = prev_step_cap.map(|cap| cap.saturating_add(512));
        }
        let result = (|| -> Option<ForcedDepth2Prepass> {
            let allow_next3 = matches!(tier, Tier::Wide);
            let before = budget.probe_calls;
            let maybe = rollout_current_trick_with_resolution(
                card,
                ctx,
                leader_target,
                budget,
                start,
                allow_next3,
                continuation_scale_permil,
                limit_ms_remaining,
                depth2_enabled,
            );
            let delta = budget.probe_calls.saturating_sub(before);
            if delta > 0 {
                *phase_c_probes = phase_c_probes.saturating_add(delta);
            }
            let (cont_raw, resolution) = match maybe {
                Some(pair) => pair,
                None => {
                    return None;
                }
            };
            let mut forced_cfg = cfg;
            if forced_cfg.depth2_topk == 0 {
                forced_cfg.depth2_topk = 1;
            }
            let depth2_before = budget.depth2_samples();
            let depth2_bonus = evaluate_depth2_bonus(
                &resolution,
                ctx,
                &forced_cfg,
                budget,
                start,
                continuation_scale_permil,
                leader_target,
                limit_ms_remaining,
                depth2_enabled,
            );
            if depth2_enabled && budget.depth2_samples() == depth2_before {
                budget.record_depth2_sample();
            }
            Some(ForcedDepth2Prepass {
                card,
                cont: cont_raw,
                depth2_bonus,
            })
        })();
        if relax_cap {
            budget.step_cap = prev_step_cap;
        }
        result
    }
}

impl PlayPlannerHard {
    pub fn explain_candidates_verbose(
        legal: &[Card],
        ctx: &BotContext<'_>,
    ) -> Vec<(Card, i32, i32, i32)> {
        let cfg = Self::config();
        let deterministic = std::env::var("MDH_HARD_DETERMINISTIC")
            .map(|v| v == "1" || v.eq_ignore_ascii_case("true") || v.eq_ignore_ascii_case("on"))
            .unwrap_or(false);
        let step_cap = std::env::var("MDH_HARD_TEST_STEPS")
            .ok()
            .and_then(|s| s.parse::<usize>().ok());
        let mut v = PlayPlanner::explain_candidates(legal, ctx);
        Self::apply_play_adviser_bias(&mut v, ctx);
        let planner_nudge_hits = super::play::take_hard_nudge_hits();
        v.sort_by(|a, b| b.1.cmp(&a.1));
        let best_base = v.first().map(|x| x.1).unwrap_or(0);
        let boost_gap = parse_env_i32("MDH_HARD_CONT_BOOST_GAP").unwrap_or(0);
        let boost_factor = parse_env_i32("MDH_HARD_CONT_BOOST_FACTOR").unwrap_or(1);
        let snapshot = snapshot_scores(&ctx.scores);
        let start = Instant::now();
        let mut budget = Budget::new(cfg.time_cap_ms, deterministic, step_cap, None, None);
        let mut out = Vec::new();
        let (tier, leverage_score, limits) = effective_limits(ctx);
        let phaseb_topk = limits.phaseb_topk;
        let mut scanned = 0usize;
        let continuation_scale_permil = 1000;
        for (idx, (card, base)) in v.into_iter().take(cfg.branch_limit).enumerate() {
            if budget.should_stop() {
                break;
            }
            let cont_raw = if phaseb_topk == 0 || idx < phaseb_topk {
                Self::rollout_current_trick(
                    card,
                    ctx,
                    snapshot.max_player,
                    &mut budget,
                    start,
                    false,
                    continuation_scale_permil,
                    None,
                    false,
                )
            } else {
                0
            };
            let cont = if boost_gap > 0 && boost_factor > 1 && (best_base - base) <= boost_gap {
                cont_raw.saturating_mul(boost_factor)
            } else {
                cont_raw
            };
            let total = base + cont;
            out.push((card, base, cont, total));
            scanned += 1;
            budget.tick();
            if budget.timed_out(start) {
                break;
            }
        }
        let planner_nudge_trace = super::play::take_hard_nudge_trace_summary();
        set_last_stats(Stats {
            scanned,
            elapsed_ms: start.elapsed().as_millis() as u32,
            scanned_phase_a: scanned,
            scanned_phase_b: 0,
            scanned_phase_c: 0,
            leverage_score,
            tier,
            limits_in_effect: limits,
            utilization: budget.utilization_percent(start),
            depth2_samples: budget.depth2_samples(),
            wide_boost_feed_permil: parse_env_i32("MDH_HARD_WIDE_PERMIL_BOOST_FEED").unwrap_or(0),
            wide_boost_self_permil: parse_env_i32("MDH_HARD_WIDE_PERMIL_BOOST_SELFCAP")
                .unwrap_or(0),
            cont_cap: weights().cont_cap,
            continuation_scale_permil,
            next3_tiny_hits: 0,
            endgame_dp_hits: 0,
            planner_nudge_hits,
            planner_nudge_trace,
            mix_hint_bias: super::play::take_mix_hint_bias_stats(),
            controller_bias_delta: ctx.controller_bias_delta,
        });
        out
    }

    pub fn explain_candidates_verbose_parts(
        legal: &[Card],
        ctx: &BotContext<'_>,
    ) -> Vec<(Card, i32, ContParts, i32)> {
        let cfg = Self::config();
        let deterministic = std::env::var("MDH_HARD_DETERMINISTIC")
            .map(|v| v == "1" || v.eq_ignore_ascii_case("true") || v.eq_ignore_ascii_case("on"))
            .unwrap_or(false);
        let step_cap = std::env::var("MDH_HARD_TEST_STEPS")
            .ok()
            .and_then(|s| s.parse::<usize>().ok());
        let mut v = PlayPlanner::explain_candidates(legal, ctx);
        Self::apply_play_adviser_bias(&mut v, ctx);
        v.sort_by(|a, b| b.1.cmp(&a.1));
        let snapshot = snapshot_scores(&ctx.scores);
        let start = Instant::now();
        let mut budget = Budget::new(cfg.time_cap_ms, deterministic, step_cap, None, None);
        let mut out = Vec::new();
        let (_, _, limits) = effective_limits(ctx);
        let phaseb_topk = limits.phaseb_topk;
        let continuation_scale_permil = 1000;
        for (idx, (card, base)) in v.into_iter().take(cfg.branch_limit).enumerate() {
            if budget.should_stop() {
                break;
            }
            let (cont, parts) = if phaseb_topk == 0 || idx < phaseb_topk {
                Self::rollout_current_trick_with_parts(
                    card,
                    ctx,
                    snapshot.max_player,
                    &mut budget,
                    start,
                    false,
                    continuation_scale_permil,
                    None,
                    false,
                )
            } else {
                (0, ContParts::default())
            };
            let total = base + cont;
            out.push((card, base, parts, total));
            budget.tick();
            if budget.timed_out(start) {
                break;
            }
        }
        out
    }
    // Wrapper: supports optional deterministic multi-sample aggregation (env-gated)
    fn rollout_current_trick(
        card: Card,
        ctx: &BotContext<'_>,
        leader_target: PlayerPosition,
        budget: &mut Budget,
        start: Instant,
        next3_allowed: bool,
        continuation_scale_permil: i32,
        limit_ms: Option<u32>,
        depth2_enabled: bool,
    ) -> i32 {
        if det_enabled_for(ctx) {
            let mut acc = 0i32;
            let k = det_sample_k().max(1);
            for _ in 0..k {
                if budget.should_stop() || budget.timed_out(start) {
                    break;
                }
                acc += Self::rollout_current_trick_core(
                    card,
                    ctx,
                    leader_target,
                    budget,
                    start,
                    next3_allowed,
                    continuation_scale_permil,
                    limit_ms,
                    depth2_enabled,
                );
            }
            if k > 0 { acc / (k as i32) } else { 0 }
        } else {
            Self::rollout_current_trick_core(
                card,
                ctx,
                leader_target,
                budget,
                start,
                next3_allowed,
                continuation_scale_permil,
                limit_ms,
                depth2_enabled,
            )
        }
    }

    // Core: single-sample current-trick rollout (existing logic)
    fn rollout_current_trick_core(
        card: Card,
        ctx: &BotContext<'_>,
        leader_target: PlayerPosition,
        budget: &mut Budget,
        start: Instant,
        next3_allowed: bool,
        continuation_scale_permil: i32,
        limit_ms: Option<u32>,
        depth2_enabled: bool,
    ) -> i32 {
        match rollout_current_trick_with_resolution(
            card,
            ctx,
            leader_target,
            budget,
            start,
            next3_allowed,
            continuation_scale_permil,
            limit_ms,
            depth2_enabled,
        ) {
            Some((value, _)) => value,
            None => 0,
        }
    }
    // Wrapper with parts: supports optional deterministic multi-sample aggregation (env-gated)
    fn rollout_current_trick_with_parts(
        card: Card,
        ctx: &BotContext<'_>,
        leader_target: PlayerPosition,
        budget: &mut Budget,
        start: Instant,
        next3_allowed: bool,
        continuation_scale_permil: i32,
        limit_ms: Option<u32>,
        depth2_enabled: bool,
    ) -> (i32, ContParts) {
        if det_enabled() {
            let mut acc = 0i32;
            let mut acc_parts = ContParts::default();
            let k = det_sample_k().max(1);
            for _ in 0..k {
                if budget.should_stop() || budget.timed_out(start) {
                    break;
                }
                let (v, p) = Self::rollout_current_trick_with_parts_core(
                    card,
                    ctx,
                    leader_target,
                    budget,
                    start,
                    next3_allowed,
                    continuation_scale_permil,
                    limit_ms,
                    depth2_enabled,
                );
                acc += v;
                acc_parts.feed += p.feed;
                acc_parts.self_capture += p.self_capture;
                acc_parts.next_start += p.next_start;
                acc_parts.next_probe += p.next_probe;
                acc_parts.qs_risk += p.qs_risk;
                acc_parts.ctrl_hearts += p.ctrl_hearts;
                acc_parts.ctrl_handoff += p.ctrl_handoff;
                acc_parts.moon_relief += p.moon_relief;
                acc_parts.capped_delta += p.capped_delta;
            }
            if k > 0 {
                let div = k as i32;
                acc_parts.feed /= div;
                acc_parts.self_capture /= div;
                acc_parts.next_start /= div;
                acc_parts.next_probe /= div;
                acc_parts.qs_risk /= div;
                acc_parts.ctrl_hearts /= div;
                acc_parts.ctrl_handoff /= div;
                acc_parts.moon_relief /= div;
                acc_parts.capped_delta /= div;
                (acc / div, acc_parts)
            } else {
                (0, ContParts::default())
            }
        } else {
            Self::rollout_current_trick_with_parts_core(
                card,
                ctx,
                leader_target,
                budget,
                start,
                next3_allowed,
                continuation_scale_permil,
                limit_ms,
                depth2_enabled,
            )
        }
    }

    // Core with parts: single-sample current-trick rollout (existing logic)
    fn rollout_current_trick_with_parts_core(
        card: Card,
        ctx: &BotContext<'_>,
        leader_target: PlayerPosition,
        budget: &mut Budget,
        start: Instant,
        next3_allowed: bool,
        continuation_scale_permil: i32,
        limit_ms: Option<u32>,
        depth2_enabled: bool,
    ) -> (i32, ContParts) {
        let mut parts = ContParts::default();
        // Simulate only the remainder of the current trick with a simple, void-aware policy.
        let mut sim = ctx.round.clone();
        let seat = ctx.seat;
        let snapshot = snapshot_scores(&ctx.scores);
        let my_score = ctx.scores.score(ctx.seat) as i32;
        let leader_gap = snapshot.max_score as i32 - my_score;
        let moon_pressure = detect_moon_pressure(ctx, &snapshot);
        let (seat_feed_bias, seat_self_bias, seat_moon_bias) =
            Self::seat_cont_bias(seat, leader_gap, moon_pressure, limit_ms, depth2_enabled);
        let mut outcome = match sim.play_card(seat, card) {
            Ok(o) => o,
            Err(_) => return (0, parts),
        };
        budget.tick();
        while !matches!(outcome, PlayOutcome::TrickCompleted { .. }) {
            if budget.should_stop() || budget.timed_out(start) {
                break;
            }
            let next = next_to_play(&sim);
            let reply = if det_enabled() {
                choose_followup_search_sampled(
                    &sim,
                    next,
                    Some(ctx.tracker),
                    seat,
                    Some(leader_target),
                    budget,
                )
            } else {
                choose_followup_search(&sim, next, Some(ctx.tracker), seat, Some(leader_target))
            };
            outcome = match sim.play_card(next, reply) {
                Ok(o) => o,
                Err(_) => break,
            };
            budget.tick();
        }
        let mut cont = 0;
        if let PlayOutcome::TrickCompleted { winner, penalties } = outcome {
            let p = penalties as i32;
            let (tier_here, _lev, _lim) = effective_limits(ctx);
            let wide_boost_feed = if matches!(tier_here, Tier::Wide) {
                parse_env_i32("MDH_HARD_WIDE_PERMIL_BOOST_FEED").unwrap_or(300)
            } else {
                0
            };
            let wide_boost_self = if matches!(tier_here, Tier::Wide) {
                parse_env_i32("MDH_HARD_WIDE_PERMIL_BOOST_SELFCAP").unwrap_or(180)
            } else {
                0
            };
            if winner == leader_target && p > 0 {
                let base = weights().cont_feed_perpen * p;
                let scale =
                    1000 + (weights().scale_feed_permil.max(0) * p) + wide_boost_feed.max(0);
                let v = (base * scale) / 1000;
                let scaled = Self::apply_continuation_scale(v, continuation_scale_permil);
                let scaled = Self::apply_continuation_scale(scaled, seat_feed_bias);
                parts.feed = scaled;
                cont += scaled;
            }
            if winner == seat && p > 0 {
                let base = weights().cont_self_capture_perpen * p;
                let scale =
                    1000 + (weights().scale_self_permil.max(0) * p) + wide_boost_self.max(0);
                let v = -(base * scale) / 1000;
                let scaled = Self::apply_continuation_scale(v, continuation_scale_permil);
                let scaled = Self::apply_continuation_scale(scaled, seat_self_bias);
                parts.self_capture = scaled;
                cont += scaled;
            }
            if winner == seat {
                let start_bonus = next_trick_start_bonus(&sim, seat);
                parts.next_start = start_bonus;
                cont += start_bonus;
                budget.probe_calls = budget.probe_calls.saturating_add(1);
                let probe = next_trick_probe(&sim, seat, ctx, leader_target, next3_allowed);
                parts.next_probe = probe;
                cont += probe;
                if weights().qs_risk_per != 0 {
                    let has_ace_spades = sim
                        .hand(seat)
                        .iter()
                        .any(|c| c.suit == Suit::Spades && c.rank.value() == 14);
                    if has_ace_spades {
                        parts.qs_risk = -weights().qs_risk_per;
                        cont += parts.qs_risk;
                    }
                }
                if weights().ctrl_hearts_per != 0 && sim.hearts_broken() {
                    let hearts_cnt = sim
                        .hand(seat)
                        .iter()
                        .filter(|c| c.suit == Suit::Hearts)
                        .count() as i32;
                    let v = hearts_cnt * weights().ctrl_hearts_per;
                    parts.ctrl_hearts = v;
                    cont += v;
                }
                if weights().moon_relief_perpen != 0 && p > 0 {
                    let state = ctx.tracker.moon_state(seat);
                    if matches!(state, MoonState::Considering | MoonState::Committed) {
                        let v = weights().moon_relief_perpen * p;
                        let scaled = Self::apply_continuation_scale(v, continuation_scale_permil);
                        let scaled = Self::apply_continuation_scale(scaled, seat_moon_bias);
                        parts.moon_relief = scaled;
                        cont += scaled;
                    }
                }
            }
            if winner != seat && weights().ctrl_handoff_pen != 0 {
                parts.ctrl_handoff = -weights().ctrl_handoff_pen;
                cont += parts.ctrl_handoff;
            }
        }
        let cap = weights().cont_cap;
        if cap > 0 {
            if cont > cap {
                parts.capped_delta = cap - cont;
                cont = cap;
            }
            if cont < -cap {
                parts.capped_delta = -cap - cont;
                cont = -cap;
            }
        }
        (cont, parts)
    }
}

fn next_to_play(round: &RoundState) -> PlayerPosition {
    let trick = round.current_trick();
    trick
        .plays()
        .last()
        .map(|p| p.position.next())
        .unwrap_or(trick.leader())
}

fn choose_followup_search(
    round: &RoundState,
    seat: PlayerPosition,
    tracker: Option<&crate::bot::tracker::UnseenTracker>,
    origin: PlayerPosition,
    leader_target: Option<PlayerPosition>,
) -> Card {
    // Minimal, void-aware heuristic response.
    let legal = legal_moves_for(round, seat);
    let lead_suit = round.current_trick().lead_suit();
    if let Some(lead) = lead_suit {
        if let Some(card) = legal
            .iter()
            .copied()
            .filter(|c| c.suit == lead)
            .min_by_key(|c| c.rank.value())
        {
            return card;
        }
        let hearts_void = tracker
            .map(|t| t.is_void(seat, Suit::Hearts))
            .unwrap_or(false);
        let provisional = provisional_winner(round);
        if provisional == Some(origin) {
            // Avoid giving points to origin (our seat) in simulation
            if let Some(card) = legal
                .iter()
                .copied()
                .filter(|c| c.penalty_value() == 0)
                .min_by_key(|c| c.rank.value())
            {
                return card;
            }
        }
        if let (Some(pw), Some(leader)) = (provisional, leader_target) {
            if pw == leader {
                // Target the leader: dump QS if possible, otherwise dump highest heart.
                if let Some(qs) = legal.iter().copied().find(|c| c.is_queen_of_spades()) {
                    return qs;
                }
                if !hearts_void {
                    if let Some(card) = legal
                        .iter()
                        .copied()
                        .filter(|c| c.suit == Suit::Hearts)
                        .max_by_key(|c| c.rank.value())
                    {
                        return card;
                    }
                }
            } else {
                // Provisional winner is not the leader: avoid feeding penalties.
                let mut non_penalties: Vec<Card> = legal
                    .iter()
                    .copied()
                    .filter(|c| c.penalty_value() == 0)
                    .collect();
                if !non_penalties.is_empty() {
                    // Small deterministic bias: choose a middle-rank discard when we have several options,
                    // to avoid extremes that could create future exposure; otherwise pick the lowest.
                    non_penalties.sort_by_key(|c| c.rank.value());
                    let idx = if non_penalties.len() >= 3 {
                        non_penalties.len() / 2
                    } else {
                        0
                    };
                    return non_penalties[idx];
                }
                // If forced to give points, choose the smallest penalty (low hearts before QS).
                if !hearts_void {
                    if let Some(card) = legal
                        .iter()
                        .copied()
                        .filter(|c| c.suit == Suit::Hearts)
                        .min_by_key(|c| c.rank.value())
                    {
                        return card;
                    }
                }
                if let Some(qs) = legal.iter().copied().find(|c| c.is_queen_of_spades()) {
                    return qs;
                }
            }
        } else {
            // No leader targeting info: prefer safe discard.
            let mut non_penalties: Vec<Card> = legal
                .iter()
                .copied()
                .filter(|c| c.penalty_value() == 0)
                .collect();
            if !non_penalties.is_empty() {
                non_penalties.sort_by_key(|c| c.rank.value());
                let idx = if non_penalties.len() >= 3 {
                    non_penalties.len() / 2
                } else {
                    0
                };
                return non_penalties[idx];
            }
            if !hearts_void {
                if let Some(card) = legal
                    .iter()
                    .copied()
                    .filter(|c| c.suit == Suit::Hearts)
                    .min_by_key(|c| c.rank.value())
                {
                    return card;
                }
            }
            if let Some(qs) = legal.iter().copied().find(|c| c.is_queen_of_spades()) {
                return qs;
            }
        }
    }
    legal
        .into_iter()
        .max_by_key(|c| (c.penalty_value(), c.rank.value()))
        .expect("legal non-empty")
}

// Belief-guided deterministic follow-up selection when opponents are void.
fn choose_followup_search_sampled(
    round: &RoundState,
    seat: PlayerPosition,
    tracker: Option<&crate::bot::tracker::UnseenTracker>,
    origin: PlayerPosition,
    leader_target: Option<PlayerPosition>,
    budget: &mut Budget,
) -> Card {
    let canon = choose_followup_search(round, seat, tracker, origin, leader_target);
    let legal = legal_moves_for(round, seat);
    if legal.is_empty() {
        return canon;
    }
    let Some(tracker) = tracker else {
        return canon;
    };
    let Some(lead) = round.current_trick().lead_suit() else {
        return canon;
    };
    if legal.iter().any(|c| c.suit == lead) {
        return canon;
    }
    let _ = tracker.snapshot_beliefs_for_round(round);
    let belief = tracker.belief_state(seat);
    let cfg = super::tracker::BeliefSamplerConfig::from_env();
    let mut scored: Vec<(Card, f32)> = legal
        .iter()
        .copied()
        .map(|card| {
            let prob = belief.card_probability(card);
            let penalty = card.penalty_value() as f32;
            let weight = if cfg.filter_zero && prob == 0.0 {
                0.0
            } else {
                prob.max(1e-6) * (1.0 + penalty)
            };
            (card, weight)
        })
        .collect();
    if cfg.filter_zero {
        scored.retain(|(_, w)| *w > 0.0);
        if scored.is_empty() {
            scored = legal.iter().copied().map(|c| (c, 1.0)).collect();
        }
    }
    scored.sort_by(|a, b| {
        b.1.partial_cmp(&a.1)
            .unwrap_or(std::cmp::Ordering::Equal)
            .then_with(|| {
                (b.0.penalty_value(), b.0.rank.value())
                    .cmp(&(a.0.penalty_value(), a.0.rank.value()))
            })
    });
    let primary_len = cfg.top_k.min(scored.len()).max(1);
    let use_diversity = cfg.diversity > 0 && sample_bit_for(round, seat, budget);
    let pool_len = if use_diversity {
        (primary_len + cfg.diversity).min(scored.len())
    } else {
        primary_len
    };
    let idx = sample_index_for(round, seat, budget, pool_len);
    let chosen = scored[idx].0;
    if chosen != canon {
        return chosen;
    }
    for (card, _) in scored.iter().take(pool_len) {
        if *card != canon {
            return *card;
        }
    }
    canon
}

fn sample_bit_for(round: &RoundState, seat: PlayerPosition, budget: &Budget) -> bool {
    let trick = round.current_trick();
    let plays = trick.plays().len() as u64;
    let lead = trick.leader() as u8 as u64;
    let seatv = seat as u8 as u64;
    let steps = budget.steps as u64;
    let base = steps
        .wrapping_mul(0x9E37_79B9_7F4A_7C15)
        .wrapping_add(plays << 3)
        .wrapping_add((lead << 1) ^ seatv);
    let x = base ^ (base >> 33) ^ (base << 17);
    (x & 1) == 1
}

fn sample_index_for(
    round: &RoundState,
    seat: PlayerPosition,
    budget: &Budget,
    len: usize,
) -> usize {
    if len <= 1 {
        return 0;
    }
    let trick = round.current_trick();
    let plays = trick.plays().len() as u64;
    let lead = trick.leader() as u8 as u64;
    let seatv = seat as u8 as u64;
    let steps = budget.steps as u64;
    let base = steps
        .wrapping_mul(0xD1B5_4A32_C3D2_EF95)
        .wrapping_add((plays << 4) ^ (lead << 2) ^ seatv);
    let mut x = base ^ 0xA5A5_5A5A_C3C3_3C3C;
    x = xorshift64(x);
    (x % len as u64) as usize
}

fn xorshift64(mut x: u64) -> u64 {
    x ^= x << 13;
    x ^= x >> 7;
    x ^= x << 17;
    x
}

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

fn provisional_winner(round: &RoundState) -> Option<PlayerPosition> {
    let trick = round.current_trick();
    let lead = trick.lead_suit()?;
    trick
        .plays()
        .iter()
        .filter(|p| p.card.suit == lead)
        .max_by_key(|p| p.card.rank.value())
        .map(|p| p.position)
}

fn next_trick_start_bonus(round: &RoundState, leader: PlayerPosition) -> i32 {
    let hand = round.hand(leader);
    // +25 per singleton in non-hearts suits (max 3). Small +2 per heart when hearts are broken (cap 10).
    let mut counts = [0u8; 4];
    for c in hand.iter() {
        counts[c.suit as usize] = counts[c.suit as usize].saturating_add(1);
    }
    let mut bonus = 0;
    for (i, &cnt) in counts.iter().enumerate() {
        if i != Suit::Hearts as usize && cnt == 1 {
            bonus += weights().next_trick_singleton_bonus;
        }
    }
    bonus = bonus.min(weights().next_trick_singleton_bonus * 3);
    if round.hearts_broken() {
        let hearts = counts[Suit::Hearts as usize] as i32;
        bonus += (weights().next_trick_hearts_per * hearts).min(weights().next_trick_hearts_cap);
    }
    bonus
}

struct HardWeights {
    cont_feed_perpen: i32,
    cont_self_capture_perpen: i32,
    next_trick_singleton_bonus: i32,
    next_trick_hearts_per: i32,
    next_trick_hearts_cap: i32,
    next2_feed_perpen: i32,
    next2_self_capture_perpen: i32,
    // Tiny continuation extras (defaults 0):
    qs_risk_per: i32,
    ctrl_hearts_per: i32,
    ctrl_handoff_pen: i32,
    cont_cap: i32,
    moon_relief_perpen: i32,
    // Adaptive scaling (perÃƒÂ¢Ã¢â€šÂ¬Ã¢â‚¬Ëœmille per penalty; defaults 0):
    scale_feed_permil: i32,
    scale_self_permil: i32,
}

fn parse_env_i32(key: &str) -> Option<i32> {
    std::env::var(key).ok().and_then(|s| s.parse::<i32>().ok())
}

fn depth2_scale_permil() -> i32 {
    parse_env_i32("MDH_SEARCH_DEPTH2_SCALE_PERMIL").unwrap_or(350)
}

fn depth2_cap() -> i32 {
    parse_env_i32("MDH_SEARCH_DEPTH2_CAP").unwrap_or(600)
}

fn weights() -> &'static HardWeights {
    static W: std::sync::OnceLock<HardWeights> = std::sync::OnceLock::new();
    W.get_or_init(|| HardWeights {
        // Phase A: modestly stronger defaults for Hard continuation
        cont_feed_perpen: parse_env_i32("MDH_HARD_CONT_FEED_PERPEN").unwrap_or(120),
        cont_self_capture_perpen: parse_env_i32("MDH_HARD_CONT_SELF_CAPTURE_PERPEN").unwrap_or(160),
        next_trick_singleton_bonus: parse_env_i32("MDH_HARD_NEXTTRICK_SINGLETON").unwrap_or(25),
        next_trick_hearts_per: parse_env_i32("MDH_HARD_NEXTTRICK_HEARTS_PER").unwrap_or(2),
        next_trick_hearts_cap: parse_env_i32("MDH_HARD_NEXTTRICK_HEARTS_CAP").unwrap_or(10),
        next2_feed_perpen: parse_env_i32("MDH_HARD_NEXT2_FEED_PERPEN").unwrap_or(60),
        next2_self_capture_perpen: parse_env_i32("MDH_HARD_NEXT2_SELF_CAPTURE_PERPEN")
            .unwrap_or(80),
        qs_risk_per: parse_env_i32("MDH_HARD_QS_RISK_PER").unwrap_or(0),
        ctrl_hearts_per: parse_env_i32("MDH_HARD_CTRL_HEARTS_PER").unwrap_or(0),
        ctrl_handoff_pen: parse_env_i32("MDH_HARD_CTRL_HANDOFF_PEN").unwrap_or(0),
        cont_cap: parse_env_i32("MDH_HARD_CONT_CAP").unwrap_or(250),
        moon_relief_perpen: parse_env_i32("MDH_HARD_MOON_RELIEF_PERPEN").unwrap_or(80),
        scale_feed_permil: parse_env_i32("MDH_HARD_CONT_SCALE_FEED_PERMIL").unwrap_or(75),
        scale_self_permil: parse_env_i32("MDH_HARD_CONT_SCALE_SELFCAP_PERMIL").unwrap_or(25),
    })
}

#[derive(Debug, Default, Clone, Copy)]
pub struct ContParts {
    pub feed: i32,
    pub self_capture: i32,
    pub next_start: i32,
    pub next_probe: i32,
    pub qs_risk: i32,
    pub ctrl_hearts: i32,
    pub ctrl_handoff: i32,
    pub moon_relief: i32,
    pub capped_delta: i32,
}

fn debug_enabled() -> bool {
    static ON: std::sync::OnceLock<bool> = std::sync::OnceLock::new();
    *ON.get_or_init(|| {
        std::env::var("MDH_DEBUG_LOGS")
            .map(|v| v == "1" || v.eq_ignore_ascii_case("true") || v.eq_ignore_ascii_case("on"))
            .unwrap_or(false)
    })
}

#[derive(Debug, Clone)]
#[allow(dead_code)]
pub struct Stats {
    pub scanned: usize,
    pub elapsed_ms: u32,
    pub scanned_phase_a: usize,
    pub scanned_phase_b: usize,
    pub scanned_phase_c: usize,
    pub leverage_score: u8,
    pub tier: Tier,
    pub limits_in_effect: Limits,
    pub utilization: u8,
    pub depth2_samples: usize,
    // Telemetry for env-configured continuation scaling/cap (for tuning introspection)
    pub wide_boost_feed_permil: i32,
    pub wide_boost_self_permil: i32,
    pub cont_cap: i32,
    pub continuation_scale_permil: i32,
    pub next3_tiny_hits: usize,
    pub endgame_dp_hits: usize,
    pub planner_nudge_hits: usize,
    pub planner_nudge_trace: Option<Vec<(String, usize)>>,
    pub mix_hint_bias: Option<super::play::MixHintBiasStats>,
    pub controller_bias_delta: Option<i32>,
}

static LAST_STATS: Lazy<Mutex<Option<Stats>>> = Lazy::new(|| Mutex::new(None));

fn set_last_stats(s: Stats) {
    if let Ok(mut slot) = LAST_STATS.lock() {
        *slot = Some(s);
    }
}

#[allow(dead_code)]
pub fn last_stats() -> Option<Stats> {
    LAST_STATS.lock().ok().and_then(|g| g.clone())
}

fn next_trick_probe(
    sim_round: &RoundState,
    leader: PlayerPosition,
    ctx: &BotContext<'_>,
    leader_target: PlayerPosition,
    next3_allowed: bool,
) -> i32 {
    let cfg = PlayPlannerHard::config();
    let probe_ab_margin = parse_env_i32("MDH_HARD_PROBE_AB_MARGIN").unwrap_or(0);
    // Effective per-tier limit for how many next-trick leads to probe
    let (_, _, limits) = effective_limits(ctx);
    let tmp_ctx = BotContext::new(
        leader,
        sim_round,
        ctx.scores,
        ctx.passing_direction,
        ctx.tracker,
        ctx.difficulty,
    );
    let legal = legal_moves_for(sim_round, leader);
    if legal.is_empty() {
        return 0;
    }
    let mut ordered = PlayPlanner::explain_candidates(&legal, &tmp_ctx);
    ordered.sort_by(|a, b| b.1.cmp(&a.1));
    let start = Instant::now();
    let mut bonus = 0;
    let mut next_limit = if limits.next_probe_m > 0 {
        limits.next_probe_m
    } else {
        cfg.next_branch_limit
    };
    if next3_allowed {
        next_limit = next_limit.saturating_add(3);
    }
    // Determinization wide-like probe widening (env-gated)
    if bool_env("MDH_HARD_DET_ENABLE") && bool_env("MDH_HARD_DET_PROBE_WIDE_LIKE") {
        next_limit = next_limit.saturating_add(2);
    }
    for (lead_card, _) in ordered.into_iter().take(next_limit) {
        if start.elapsed().as_millis() as u32 >= cfg.time_cap_ms {
            break;
        }
        let mut probe = sim_round.clone();
        // Play our lead
        let _ = match probe.play_card(leader, lead_card) {
            Ok(o) => o,
            Err(_) => continue,
        };
        // Branch selectively on the first opponent reply (two variants)
        let first_opponent = next_to_play(&probe);
        let mut replies: Vec<Card> = Vec::new();
        // Canonical reply
        let canon = choose_followup_search(
            &probe,
            first_opponent,
            Some(ctx.tracker),
            leader,
            Some(leader_target),
        );
        replies.push(canon);
        // Alternate: max-penalty dump if available when not following suit
        if let Some(lead_suit) = probe.current_trick().lead_suit() {
            let legal = legal_moves_for(&probe, first_opponent);
            let can_follow = legal.iter().any(|c| c.suit == lead_suit);
            if !can_follow {
                if let Some(alt) = legal
                    .into_iter()
                    .max_by_key(|c| (c.penalty_value(), c.rank.value()))
                {
                    if alt != canon {
                        replies.push(alt);
                    }
                }
            }
        }
        // Evaluate each reply variant; additionally branch on the second opponent reply when time permits.
        let mut local_best = 0;
        for reply in replies.into_iter() {
            if start.elapsed().as_millis() as u32 >= cfg.time_cap_ms {
                break;
            }
            let mut branch = probe.clone();
            let _ = match branch.play_card(first_opponent, reply) {
                Ok(o) => o,
                Err(_) => continue,
            };
            // Optional second-opponent branching
            let second_opponent = next_to_play(&branch);
            let mut replies2: Vec<Card> = Vec::new();
            let canon2 = choose_followup_search(
                &branch,
                second_opponent,
                Some(ctx.tracker),
                leader,
                Some(leader_target),
            );
            replies2.push(canon2);
            if let Some(lead_suit2) = branch.current_trick().lead_suit() {
                let legal2 = legal_moves_for(&branch, second_opponent);
                let can_follow2 = legal2.iter().any(|c| c.suit == lead_suit2);
                if !can_follow2 {
                    if let Some(alt2) = legal2
                        .into_iter()
                        .max_by_key(|c| (c.penalty_value(), c.rank.value()))
                    {
                        if alt2 != canon2 {
                            replies2.push(alt2);
                        }
                    }
                }
            }
            for reply2 in replies2.into_iter() {
                if start.elapsed().as_millis() as u32 >= cfg.time_cap_ms {
                    break;
                }
                if probe_ab_margin > 0 && local_best >= probe_ab_margin {
                    break;
                }
                let mut branch2 = branch.clone();
                let mut outcome2 = match branch2.play_card(second_opponent, reply2) {
                    Ok(o) => o,
                    Err(_) => continue,
                };
                // Optionally branch on third opponent reply (env-gated), otherwise finish canonically.
                let next3_enabled_env = bool_env("MDH_HARD_NEXT3_ENABLE")
                    || (det_enabled_for(ctx) && bool_env("MDH_HARD_DET_NEXT3_ENABLE"));
                let (tier_here, _, _) = effective_limits(ctx);
                let tiny_next3_normal =
                    matches!(tier_here, Tier::Normal) && bool_env("MDH_HARD_NEXT3_TINY_NORMAL");
                if tiny_next3_normal {
                    NEXT3_TINY_COUNT.with(|c| c.set(c.get().saturating_add(1)));
                }
                let next3_enabled = next3_allowed || next3_enabled_env || tiny_next3_normal;
                if next3_enabled {
                    // Build third-opponent reply set: canonical + optional max-penalty off-suit dump
                    if start.elapsed().as_millis() as u32 >= cfg.time_cap_ms {
                        break;
                    }
                    // Identify third opponent (the next to play now)
                    let third_opponent = next_to_play(&branch2);
                    let mut replies3: Vec<Card> = Vec::new();
                    let canon3 = choose_followup_search(
                        &branch2,
                        third_opponent,
                        Some(ctx.tracker),
                        leader,
                        Some(leader_target),
                    );
                    replies3.push(canon3);
                    if let Some(lead_suit3) = branch2.current_trick().lead_suit() {
                        let legal3 = legal_moves_for(&branch2, third_opponent);
                        let can_follow3 = legal3.iter().any(|c| c.suit == lead_suit3);
                        if !can_follow3 {
                            if let Some(alt3) = legal3
                                .into_iter()
                                .max_by_key(|c| (c.penalty_value(), c.rank.value()))
                            {
                                if alt3 != canon3 {
                                    replies3.push(alt3);
                                }
                            }
                        }
                    }
                    for reply3 in replies3.into_iter() {
                        if start.elapsed().as_millis() as u32 >= cfg.time_cap_ms {
                            break;
                        }
                        if probe_ab_margin > 0 && local_best >= probe_ab_margin {
                            break;
                        }
                        let mut branch3 = branch2.clone();
                        let mut outcome3 = match branch3.play_card(third_opponent, reply3) {
                            Ok(o) => o,
                            Err(_) => continue,
                        };
                        while !matches!(outcome3, PlayOutcome::TrickCompleted { .. }) {
                            let nxt = next_to_play(&branch3);
                            let r = choose_followup_search(
                                &branch3,
                                nxt,
                                Some(ctx.tracker),
                                leader,
                                Some(leader_target),
                            );
                            outcome3 = match branch3.play_card(nxt, r) {
                                Ok(o) => o,
                                Err(_) => break,
                            };
                        }
                        if let PlayOutcome::TrickCompleted { winner, penalties } = outcome3 {
                            let p = penalties as i32;
                            let mut cont = 0;
                            if winner == leader_target && p > 0 {
                                cont += weights().next2_feed_perpen * p;
                            }
                            if winner == leader && p > 0 {
                                cont -= weights().next2_self_capture_perpen * p;
                            }
                            // Endgame micro-solver (choose-only; env-gated)
                            cont += micro_endgame_bonus(&branch3, ctx, leader, leader_target);
                            if cont > local_best {
                                local_best = cont;
                            }
                        }
                    }
                } else {
                    // Canonical finish of trick
                    while !matches!(outcome2, PlayOutcome::TrickCompleted { .. }) {
                        let nxt = next_to_play(&branch2);
                        let r = choose_followup_search(
                            &branch2,
                            nxt,
                            Some(ctx.tracker),
                            leader,
                            Some(leader_target),
                        );
                        outcome2 = match branch2.play_card(nxt, r) {
                            Ok(o) => o,
                            Err(_) => break,
                        };
                    }
                    if let PlayOutcome::TrickCompleted { winner, penalties } = outcome2 {
                        let p = penalties as i32;
                        let mut cont = 0;
                        if winner == leader_target && p > 0 {
                            cont += weights().next2_feed_perpen * p;
                        }
                        if winner == leader && p > 0 {
                            cont -= weights().next2_self_capture_perpen * p;
                        }
                        // Endgame micro-solver (choose-only; env-gated)
                        cont += micro_endgame_bonus(&branch2, ctx, leader, leader_target);
                        if cont > local_best {
                            local_best = cont;
                        }
                    }
                }
            }
        }
        bonus += local_best;
    }
    bonus
}

// Track how many times the tiny Normal-tier next3 gate triggered during a decision (debug/telemetry)
thread_local! {
    static NEXT3_TINY_COUNT: Cell<usize> = Cell::new(0);
    static ENDGAME_DP_COUNT: Cell<usize> = Cell::new(0);
}

// ----- Endgame micro-solver (choose-only; env-gated) -----

fn micro_endgame_enabled() -> bool {
    std::env::var("MDH_HARD_ENDGAME_DP_ENABLE")
        .map(|v| v == "1" || v.eq_ignore_ascii_case("true") || v.eq_ignore_ascii_case("on"))
        .unwrap_or(false)
}

fn micro_endgame_max_cards() -> usize {
    std::env::var("MDH_HARD_ENDGAME_MAX_CARDS")
        .ok()
        .and_then(|s| s.parse().ok())
        .unwrap_or(3)
}

fn micro_endgame_bonus(
    sim: &RoundState,
    ctx: &BotContext<'_>,
    leader: PlayerPosition,
    leader_target: PlayerPosition,
) -> i32 {
    if !micro_endgame_enabled() {
        return 0;
    }
    // Quick trigger check: all seats at or below max cards
    let maxn = micro_endgame_max_cards();
    let mut ok = true;
    for seat in [
        PlayerPosition::North,
        PlayerPosition::East,
        PlayerPosition::South,
        PlayerPosition::West,
    ] {
        if sim.hand(seat).len() > maxn {
            ok = false;
            break;
        }
    }
    if !ok {
        return 0;
    }
    // Deterministic tiny DP: finish current trick if in progress, then simulate up to maxn remaining
    // tricks using our void-aware canonical follow-ups. Score per-trick outcomes with small next2 weights.
    let mut round = sim.clone();
    let mut total: i32 = 0;
    let mut tricks_simulated = 0usize;
    // Finish the current trick if it's mid-play
    if !matches!(round.current_trick().lead_suit(), None) && round.current_trick().plays().len() > 0
    {
        let mut outcome: Option<PlayOutcome> = None; // will hold TrickCompleted
        // Play until this trick completes
        while !matches!(outcome, Some(PlayOutcome::TrickCompleted { .. })) {
            let nxt = next_to_play(&round);
            let r =
                choose_followup_search(&round, nxt, Some(ctx.tracker), leader, Some(leader_target));
            outcome = match round.play_card(nxt, r) {
                Ok(o) => Some(o),
                Err(_) => break,
            };
        }
        if let Some(PlayOutcome::TrickCompleted { winner, penalties }) = outcome {
            let p = penalties as i32;
            if p > 0 {
                if winner == leader_target {
                    total += weights().next2_feed_perpen * p;
                }
                if winner == leader {
                    total -= weights().next2_self_capture_perpen * p;
                }
            }
            tricks_simulated = tricks_simulated.saturating_add(1);
        }
    }
    // Simulate next tricks (bounded by maxn)
    while tricks_simulated < maxn {
        // If any hand is empty, stop
        let mut any_cards = false;
        for seat in [
            PlayerPosition::North,
            PlayerPosition::East,
            PlayerPosition::South,
            PlayerPosition::West,
        ] {
            if !round.hand(seat).is_empty() {
                any_cards = true;
                break;
            }
        }
        if !any_cards {
            break;
        }
        // Determine who leads now
        let lead = next_to_play(&round);
        // Lead the lowest-ranked legal card for determinism
        let mut legal = legal_moves_for(&round, lead);
        if legal.is_empty() {
            break;
        }
        legal.sort_by_key(|c| (c.suit as u8, c.rank.value()));
        let lead_card = legal[0];
        let _ = match round.play_card(lead, lead_card) {
            Ok(o) => o,
            Err(_) => break,
        };
        // Finish trick canonically
        let mut outcome: Option<PlayOutcome> = None; // will hold TrickCompleted
        while !matches!(outcome, Some(PlayOutcome::TrickCompleted { .. })) {
            let nxt = next_to_play(&round);
            let r =
                choose_followup_search(&round, nxt, Some(ctx.tracker), leader, Some(leader_target));
            outcome = match round.play_card(nxt, r) {
                Ok(o) => Some(o),
                Err(_) => break,
            };
        }
        if let Some(PlayOutcome::TrickCompleted { winner, penalties }) = outcome {
            let p = penalties as i32;
            if p > 0 {
                if winner == leader_target {
                    total += weights().next2_feed_perpen * p;
                }
                if winner == leader {
                    total -= weights().next2_self_capture_perpen * p;
                }
            }
            tricks_simulated = tricks_simulated.saturating_add(1);
        } else {
            break;
        }
    }
    // Telemetry: count that DP contributed on this path
    ENDGAME_DP_COUNT.with(|c| c.set(c.get().saturating_add(1)));
    total
}

pub fn debug_hard_weights_string() -> String {
    let w = weights();
    let cfg = PlayPlannerHard::config();
    let boost_gap = parse_env_i32("MDH_HARD_CONT_BOOST_GAP").unwrap_or(0);
    let boost_factor = parse_env_i32("MDH_HARD_CONT_BOOST_FACTOR").unwrap_or(1);
    // Wide-tier continuation permille boosts (env-only; default 0)
    let wide_boost_feed = parse_env_i32("MDH_HARD_WIDE_PERMIL_BOOST_FEED").unwrap_or(0);
    let wide_boost_self = parse_env_i32("MDH_HARD_WIDE_PERMIL_BOOST_SELFCAP").unwrap_or(0);
    let phaseb_topk = std::env::var("MDH_HARD_PHASEB_TOPK")
        .ok()
        .and_then(|s| s.parse::<usize>().ok())
        .unwrap_or(0);
    let det = std::env::var("MDH_HARD_DETERMINISTIC").unwrap_or_default();
    let steps = std::env::var("MDH_HARD_TEST_STEPS").unwrap_or_default();
    let det_enable = std::env::var("MDH_HARD_DET_ENABLE").unwrap_or_default();
    let det_k = std::env::var("MDH_HARD_DET_SAMPLE_K").unwrap_or_default();
    let det_ms = std::env::var("MDH_HARD_DET_TIME_MS").unwrap_or_default();
    let abm = std::env::var("MDH_HARD_AB_MARGIN").unwrap_or_default();
    let next3 = std::env::var("MDH_HARD_NEXT3_ENABLE").unwrap_or_default();
    let probe_ab = std::env::var("MDH_HARD_PROBE_AB_MARGIN").unwrap_or_default();
    let end_dp = std::env::var("MDH_HARD_ENDGAME_DP_ENABLE").unwrap_or_default();
    let end_max = std::env::var("MDH_HARD_ENDGAME_MAX_CARDS").unwrap_or_default();
    let tiers = std::env::var("MDH_HARD_TIERS_ENABLE").unwrap_or_default();
    let th_narrow = std::env::var("MDH_HARD_LEVERAGE_THRESH_NARROW").unwrap_or_default();
    let th_normal = std::env::var("MDH_HARD_LEVERAGE_THRESH_NORMAL").unwrap_or_default();
    let tiers_auto = std::env::var("MDH_HARD_TIERS_DEFAULT_ON_HARD").unwrap_or_default();
    let promoted = std::env::var("MDH_HARD_PROMOTE_DEFAULTS").unwrap_or_default();
    format!(
        "branch_limit={} next_branch_limit={} time_cap_ms={} cutoff_margin={} ab_margin={} probe_ab_margin={} next3={} cont_feed_perpen={} cont_self_capture_perpen={} next_singleton={} next_hearts_per={} next_hearts_cap={} next2_feed_perpen={} next2_self_capture_perpen={} qs_risk_per={} ctrl_hearts_per={} ctrl_handoff_pen={} cont_cap={} moon_relief_perpen={} cont_boost_gap={} cont_boost_factor={} wide_boost_feed_permil={} wide_boost_self_permil={} phaseb_topk={} det={} steps={} det_enable={} det_k={} det_ms={} tiers={} tiers_auto={} promoted={} th_narrow={} th_normal={} cont_scale_feed_permil={} cont_scale_self_permil={} endgame_dp_enable={} endgame_max_cards={}",
        cfg.branch_limit,
        cfg.next_branch_limit,
        cfg.time_cap_ms,
        cfg.early_cutoff_margin,
        abm,
        probe_ab,
        next3,
        w.cont_feed_perpen,
        w.cont_self_capture_perpen,
        w.next_trick_singleton_bonus,
        w.next_trick_hearts_per,
        w.next_trick_hearts_cap,
        w.next2_feed_perpen,
        w.next2_self_capture_perpen,
        w.qs_risk_per,
        w.ctrl_hearts_per,
        w.ctrl_handoff_pen,
        w.cont_cap,
        w.moon_relief_perpen,
        boost_gap,
        boost_factor,
        wide_boost_feed,
        wide_boost_self,
        phaseb_topk,
        det,
        steps,
        det_enable,
        det_k,
        det_ms,
        tiers,
        tiers_auto,
        promoted,
        th_narrow,
        th_normal,
        w.scale_feed_permil,
        w.scale_self_permil,
        end_dp,
        end_max,
    )
}

// Deterministic/time-capped budget for Hard planner (env-gated)
struct Budget<'a> {
    time_cap_ms: u32,
    deterministic: bool,
    step_cap: Option<usize>,
    steps: usize,
    // telemetry counters
    probe_calls: usize,
    depth2_samples: usize,
    limit_deadline: Option<Instant>,
    limit_cancel: Option<&'a AtomicBool>,
}

impl<'a> Budget<'a> {
    fn new(
        time_cap_ms: u32,
        deterministic: bool,
        step_cap: Option<usize>,
        limit_deadline: Option<Instant>,
        limit_cancel: Option<&'a AtomicBool>,
    ) -> Self {
        Self {
            time_cap_ms,
            deterministic,
            step_cap,
            steps: 0,
            probe_calls: 0,
            depth2_samples: 0,
            limit_deadline,
            limit_cancel,
        }
    }
    fn tick(&mut self) {
        if self.deterministic {
            self.steps = self.steps.saturating_add(1);
        }
    }
    fn record_depth2_sample(&mut self) {
        self.depth2_samples = self.depth2_samples.saturating_add(1);
    }
    fn depth2_samples(&self) -> usize {
        self.depth2_samples
    }
    fn should_stop(&self) -> bool {
        if self.limit_expired() {
            return true;
        }
        if let (true, Some(cap)) = (self.deterministic, self.step_cap) {
            return self.steps >= cap;
        }
        false
    }
    fn timed_out(&self, start: Instant) -> bool {
        if self.limit_expired() {
            return true;
        }
        if self.deterministic {
            return self.should_stop();
        }
        if self.time_cap_ms == 0 {
            return false;
        }
        start.elapsed().as_millis() as u32 >= self.time_cap_ms
    }
    fn utilization_percent(&self, start: Instant) -> u8 {
        if self.deterministic {
            if let Some(cap) = self.step_cap {
                return (((self.steps as f32) / (cap as f32)) * 100.0)
                    .round()
                    .clamp(0.0, 100.0) as u8;
            }
            return 0;
        }
        let used = start.elapsed().as_millis() as u32;
        let Some(cap) = self.effective_cap_ms(start) else {
            return 0;
        };
        if cap == 0 {
            return 100;
        }
        (((used as f32) / (cap as f32)) * 100.0)
            .round()
            .clamp(0.0, 100.0) as u8
    }
    fn limit_expired(&self) -> bool {
        if let Some(cancel) = self.limit_cancel {
            if cancel.load(Ordering::Relaxed) {
                return true;
            }
        }
        if let Some(deadline) = self.limit_deadline {
            if Instant::now() >= deadline {
                return true;
            }
        }
        false
    }
    fn effective_cap_ms(&self, start: Instant) -> Option<u32> {
        let time_cap = (self.time_cap_ms > 0).then_some(self.time_cap_ms);
        let limit_cap = self.limit_deadline.map(|deadline| {
            let dur = deadline.saturating_duration_since(start);
            if dur.is_zero() {
                0
            } else {
                dur.as_millis().min(u32::MAX as u128) as u32
            }
        });
        match (time_cap, limit_cap) {
            (Some(t), Some(l)) => Some(t.min(l)),
            (Some(t), None) => Some(t),
            (None, Some(l)) => Some(l),
            (None, None) => None,
        }
    }
}

struct TrickResolution {
    round: RoundState,
    winner: PlayerPosition,
    penalties: u8,
}

fn simulate_trick_resolution(
    round: &RoundState,
    seat: PlayerPosition,
    card: Card,
    ctx: &BotContext<'_>,
    leader_target: PlayerPosition,
    budget: &mut Budget,
    start: Instant,
    _next3_allowed: bool,
) -> Option<TrickResolution> {
    let mut sim = round.clone();
    let mut outcome = match sim.play_card(seat, card) {
        Ok(o) => o,
        Err(_) => return None,
    };
    budget.tick();
    while !matches!(outcome, PlayOutcome::TrickCompleted { .. }) {
        if budget.should_stop() || budget.timed_out(start) {
            return None;
        }
        let next = next_to_play(&sim);
        let reply = if det_enabled_for(ctx) {
            choose_followup_search_sampled(
                &sim,
                next,
                Some(ctx.tracker),
                seat,
                Some(leader_target),
                budget,
            )
        } else {
            choose_followup_search(&sim, next, Some(ctx.tracker), seat, Some(leader_target))
        };
        outcome = match sim.play_card(next, reply) {
            Ok(o) => o,
            Err(_) => break,
        };
        budget.tick();
    }
    if let PlayOutcome::TrickCompleted { winner, penalties } = outcome {
        Some(TrickResolution {
            round: sim,
            winner,
            penalties,
        })
    } else {
        None
    }
}

fn continuation_from_resolution(
    resolution: &TrickResolution,
    seat: PlayerPosition,
    ctx: &BotContext<'_>,
    leader_target: PlayerPosition,
    continuation_scale_permil: i32,
    seat_feed_bias: i32,
    seat_self_bias: i32,
    seat_moon_bias: i32,
    next3_allowed: bool,
    budget: &mut Budget,
) -> i32 {
    let p = resolution.penalties as i32;
    let sim = &resolution.round;
    let (tier_here, _lev, _lim) = effective_limits(ctx);
    let wide_boost_feed = if matches!(tier_here, Tier::Wide) {
        parse_env_i32("MDH_HARD_WIDE_PERMIL_BOOST_FEED").unwrap_or(300)
    } else {
        0
    };
    let wide_boost_self = if matches!(tier_here, Tier::Wide) {
        parse_env_i32("MDH_HARD_WIDE_PERMIL_BOOST_SELFCAP").unwrap_or(180)
    } else {
        0
    };
    let mut cont = 0;
    if resolution.winner == leader_target && p > 0 {
        let base = weights().cont_feed_perpen * p;
        let scale = 1000 + (weights().scale_feed_permil.max(0) * p) + wide_boost_feed.max(0);
        let v = (base * scale) / 1000;
        let v = PlayPlannerHard::apply_continuation_scale(v, continuation_scale_permil);
        let v = PlayPlannerHard::apply_continuation_scale(v, seat_feed_bias);
        cont += v;
    }
    if resolution.winner == seat && p > 0 {
        let base = weights().cont_self_capture_perpen * p;
        let scale = 1000 + (weights().scale_self_permil.max(0) * p) + wide_boost_self.max(0);
        let v = -(base * scale) / 1000;
        let v = PlayPlannerHard::apply_continuation_scale(v, continuation_scale_permil);
        let v = PlayPlannerHard::apply_continuation_scale(v, seat_self_bias);
        cont += v;
    }
    if resolution.winner == seat {
        cont += next_trick_start_bonus(sim, seat);
        budget.probe_calls = budget.probe_calls.saturating_add(1);
        cont += next_trick_probe(sim, seat, ctx, leader_target, next3_allowed);
        if weights().qs_risk_per != 0 {
            let has_ace_spades = sim
                .hand(seat)
                .iter()
                .any(|c| c.suit == Suit::Spades && c.rank.value() == 14);
            if has_ace_spades {
                cont -= weights().qs_risk_per;
            }
        }
        if weights().ctrl_hearts_per != 0 && sim.hearts_broken() {
            let hearts_cnt = sim
                .hand(seat)
                .iter()
                .filter(|c| c.suit == Suit::Hearts)
                .count() as i32;
            cont += hearts_cnt * weights().ctrl_hearts_per;
        }
        if weights().moon_relief_perpen != 0 {
            let state = ctx.tracker.moon_state(seat);
            if matches!(state, MoonState::Considering | MoonState::Committed) && p > 0 {
                let v = weights().moon_relief_perpen * p;
                let v = PlayPlannerHard::apply_continuation_scale(v, continuation_scale_permil);
                let v = PlayPlannerHard::apply_continuation_scale(v, seat_moon_bias);
                cont += v;
            }
        }
    }
    if resolution.winner != seat && weights().ctrl_handoff_pen != 0 {
        cont -= weights().ctrl_handoff_pen;
    }
    let cap = weights().cont_cap;
    if cap > 0 {
        if cont > cap {
            cont = cap;
        }
        if cont < -cap {
            cont = -cap;
        }
    }
    cont
}

fn rollout_current_trick_with_resolution(
    card: Card,
    ctx: &BotContext<'_>,
    leader_target: PlayerPosition,
    budget: &mut Budget,
    start: Instant,
    next3_allowed: bool,
    continuation_scale_permil: i32,
    limit_ms: Option<u32>,
    depth2_enabled: bool,
) -> Option<(i32, TrickResolution)> {
    let seat = ctx.seat;
    let snapshot = snapshot_scores(&ctx.scores);
    let my_score = ctx.scores.score(ctx.seat) as i32;
    let leader_gap = snapshot.max_score as i32 - my_score;
    let moon_pressure = detect_moon_pressure(ctx, &snapshot);
    let (seat_feed_bias, seat_self_bias, seat_moon_bias) =
        PlayPlannerHard::seat_cont_bias(seat, leader_gap, moon_pressure, limit_ms, depth2_enabled);
    let resolution = simulate_trick_resolution(
        ctx.round,
        seat,
        card,
        ctx,
        leader_target,
        budget,
        start,
        next3_allowed,
    )?;
    let cont = continuation_from_resolution(
        &resolution,
        seat,
        ctx,
        leader_target,
        continuation_scale_permil,
        seat_feed_bias,
        seat_self_bias,
        seat_moon_bias,
        next3_allowed,
        budget,
    );
    Some((cont, resolution))
}

fn evaluate_depth2_bonus(
    resolution: &TrickResolution,
    ctx: &BotContext<'_>,
    cfg: &SearchConfig,
    budget: &mut Budget,
    start: Instant,
    continuation_scale_permil: i32,
    leader_target: PlayerPosition,
    limit_ms: Option<u32>,
    depth2_enabled: bool,
) -> i32 {
    if cfg.depth2_topk == 0 && !depth2_enabled {
        return 0;
    }
    if resolution.round.current_trick().leader() != ctx.seat && !depth2_enabled {
        return 0;
    }
    let depth_ctx = BotContext::new(
        ctx.seat,
        &resolution.round,
        ctx.scores,
        ctx.passing_direction,
        ctx.tracker,
        ctx.difficulty,
    );
    let legal = legal_moves_for(&resolution.round, ctx.seat);
    if legal.is_empty() {
        return 0;
    }
    let mut ordered = PlayPlanner::explain_candidates(&legal, &depth_ctx);
    ordered.sort_by(|a, b| b.1.cmp(&a.1));
    let best_base = ordered.first().map(|x| x.1).unwrap_or(0);
    let mut best_total = best_base;
    let mut considered = 0usize;
    let snapshot = snapshot_scores(&depth_ctx.scores);
    let my_score = depth_ctx.scores.score(depth_ctx.seat) as i32;
    let leader_gap = snapshot.max_score as i32 - my_score;
    let moon_pressure = detect_moon_pressure(&depth_ctx, &snapshot);
    let (seat_feed_bias, seat_self_bias, seat_moon_bias) = PlayPlannerHard::seat_cont_bias(
        depth_ctx.seat,
        leader_gap,
        moon_pressure,
        limit_ms,
        depth2_enabled,
    );
    for (card, base_child) in ordered.into_iter() {
        if considered >= cfg.depth2_topk {
            break;
        }
        if budget.should_stop() || budget.timed_out(start) {
            break;
        }
        let maybe = simulate_trick_resolution(
            &resolution.round,
            depth_ctx.seat,
            card,
            &depth_ctx,
            leader_target,
            budget,
            start,
            true,
        );
        let cont_child = match maybe {
            Some(child_res) => continuation_from_resolution(
                &child_res,
                depth_ctx.seat,
                &depth_ctx,
                leader_target,
                continuation_scale_permil,
                seat_feed_bias,
                seat_self_bias,
                seat_moon_bias,
                true,
                budget,
            ),
            None => 0,
        };
        let total_child = base_child + cont_child;
        if total_child > best_total {
            best_total = total_child;
        }
        considered = considered.saturating_add(1);
        budget.record_depth2_sample();
    }
    if best_total == best_base {
        return 0;
    }
    let delta = best_total - best_base;
    let scale = depth2_scale_permil().max(0);
    let mut bonus = if scale > 0 {
        (delta * scale) / 1000
    } else {
        delta / 4
    };
    let cap = depth2_cap().abs();
    if cap > 0 {
        bonus = bonus.clamp(-cap, cap);
    }
    bonus
}

// ----- Leverage tiers and effective limits -----

#[derive(Debug, Clone, Copy)]
pub enum Tier {
    Narrow,
    Normal,
    Wide,
}

#[derive(Debug, Clone, Copy)]
pub struct Limits {
    pub phaseb_topk: usize,
    pub next_probe_m: usize,
    pub ab_margin: i32,
}

fn bool_env(key: &str) -> bool {
    std::env::var(key)
        .map(|v| v == "1" || v.eq_ignore_ascii_case("true") || v.eq_ignore_ascii_case("on"))
        .unwrap_or(false)
}

fn effective_limits(ctx: &BotContext<'_>) -> (Tier, u8, Limits) {
    // Phase A: enable tiers by default for Hard difficulty; still respect explicit global enable/disable via env.
    let tier_for_hard = matches!(ctx.difficulty, super::BotDifficulty::FutureHard);
    let tiers_on = tier_for_hard || bool_env("MDH_HARD_TIERS_ENABLE");
    if !tiers_on {
        let phaseb_topk = std::env::var("MDH_HARD_PHASEB_TOPK")
            .ok()
            .and_then(|s| s.parse::<usize>().ok())
            .unwrap_or(0);
        let ab_margin = parse_env_i32("MDH_HARD_AB_MARGIN").unwrap_or(0);
        let next_probe_m = PlayPlannerHard::config().next_branch_limit;
        return (
            Tier::Normal,
            0,
            Limits {
                phaseb_topk,
                next_probe_m,
                ab_margin,
            },
        );
    }
    let (tier, score) = compute_leverage(ctx);
    // Respect explicit env overrides if present (global and Wide-tier specific)
    let explicit_topk = std::env::var("MDH_HARD_PHASEB_TOPK")
        .ok()
        .and_then(|s| s.parse::<usize>().ok());
    let explicit_next = std::env::var("MDH_HARD_NEXT_BRANCH_LIMIT")
        .ok()
        .and_then(|s| s.parse::<usize>().ok());
    let wide_topk_only = if matches!(tier, Tier::Wide) {
        std::env::var("MDH_HARD_WIDE_PHASEB_TOPK")
            .ok()
            .and_then(|s| s.parse::<usize>().ok())
    } else {
        None
    };
    let wide_next_only = if matches!(tier, Tier::Wide) {
        std::env::var("MDH_HARD_WIDE_NEXT_BRANCH_LIMIT")
            .ok()
            .and_then(|s| s.parse::<usize>().ok())
    } else {
        None
    };
    let explicit_ab = parse_env_i32("MDH_HARD_AB_MARGIN");
    let (def_topk, def_next, def_ab) = match tier {
        Tier::Narrow => (4usize, 1usize, 100i32),
        Tier::Normal => (6usize, 2usize, 150i32),
        Tier::Wide => (8usize, 3usize, 200i32),
    };
    let limits = Limits {
        phaseb_topk: explicit_topk.or(wide_topk_only).unwrap_or(def_topk),
        next_probe_m: explicit_next.or(wide_next_only).unwrap_or(def_next),
        ab_margin: explicit_ab.unwrap_or(def_ab),
    };
    (tier, score, limits)
}

fn compute_leverage(ctx: &BotContext<'_>) -> (Tier, u8) {
    // Simple first pass per HLD: scoreboard pressure, near-100 risk, and penalties on table.
    let snap = super::snapshot_scores(&ctx.scores);
    let my = ctx.scores.score(ctx.seat) as i32;
    let lead_score = snap.max_score as i32;
    let mut s: i32 = 0;
    // Proximity to 100
    s += if lead_score >= 90 {
        40
    } else if lead_score >= 80 {
        25
    } else {
        10
    };
    // We are not leader and trail by gap
    if snap.max_player != ctx.seat {
        let gap = (lead_score - my).max(0);
        if gap >= 15 {
            s += 25;
        } else if gap >= 8 {
            s += 15;
        } else if gap >= 4 {
            s += 8;
        }
    }
    // Penalties on table and targeting leader provisional winner
    let cur = ctx.round.current_trick();
    let pen = cur.penalty_total() as i32;
    if pen > 0 {
        s += 10;
    }
    if let Some(pw) = provisional_winner(ctx.round) {
        if pw == snap.max_player {
            s += 10;
        }
        if pw == ctx.seat {
            s += 5;
        }
    }
    // Clamp 0..100
    let s = s.clamp(0, 100) as u8;
    // Map to tiers using thresholds
    let th_narrow = std::env::var("MDH_HARD_LEVERAGE_THRESH_NARROW")
        .ok()
        .and_then(|v| v.parse::<u8>().ok())
        .unwrap_or(20);
    let th_normal = std::env::var("MDH_HARD_LEVERAGE_THRESH_NORMAL")
        .ok()
        .and_then(|v| v.parse::<u8>().ok())
        .unwrap_or(50);
    let tier = if s < th_narrow {
        Tier::Narrow
    } else if s < th_normal {
        Tier::Normal
    } else {
        Tier::Wide
    };
    (tier, s)
}

// ----- Determinization (Phase 2 scaffold; env/Hard-gated) -----
fn det_enabled() -> bool {
    bool_env("MDH_HARD_DET_ENABLE")
}
fn det_enabled_for(ctx: &BotContext<'_>) -> bool {
    if det_enabled() {
        return true;
    }
    if matches!(ctx.difficulty, super::BotDifficulty::FutureHard)
        && bool_env("MDH_HARD_DET_DEFAULT_ON")
    {
        return true;
    }
    false
}
fn det_sample_k() -> usize {
    std::env::var("MDH_HARD_DET_SAMPLE_K")
        .ok()
        .and_then(|s| s.parse::<usize>().ok())
        .unwrap_or(0)
}
