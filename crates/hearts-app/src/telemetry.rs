use crate::bot::UnseenTracker;
use crate::bot::search::Stats as SearchStats;
use hearts_core::model::player::PlayerPosition;
use parking_lot::RwLock;
use serde::Serialize;
use std::fs::{self, File};
use std::io::{self, Write};
use std::path::{Path, PathBuf};
use std::sync::OnceLock;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

static HARD_SINK: OnceLock<TelemetrySink> = OnceLock::new();
static DECISION_COUNTER: AtomicU64 = AtomicU64::new(1);

#[derive(Debug)]
pub struct TelemetrySink {
    retention: usize,
    records: RwLock<Vec<HardTelemetryRecord>>,
}

#[derive(Debug, Clone, Serialize)]
pub struct HardTelemetryRecord {
    pub timestamp_ms: u128,
    pub decision_index: u64,
    pub seat: String,
    pub belief_entropy: [f32; 4],
    pub belief_cache_size: usize,
    pub belief_cache_capacity: usize,
    pub belief_cache_hits: usize,
    pub belief_cache_misses: usize,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub difficulty: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub phase: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub think_limit_ms: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub elapsed_ms: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub timed_out: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub fallback: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub search_stats: Option<SearchTelemetrySnapshot>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub notes: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub controller_bias_delta: Option<i32>,
}

#[derive(Debug, Clone, Serialize)]
pub struct SearchTelemetrySnapshot {
    pub scanned: usize,
    pub scanned_phase_a: usize,
    pub scanned_phase_b: usize,
    pub scanned_phase_c: usize,
    pub tier: Option<String>,
    pub utilization: Option<u8>,
    pub phaseb_topk: Option<u32>,
    pub next_probe_m: Option<u32>,
    pub ab_margin: Option<i32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub continuation_scale_permil: Option<i32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub depth2_samples: Option<usize>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub mix_hint_bias: Option<MixHintBiasSnapshot>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub controller_bias_delta: Option<i32>,
}

#[derive(Debug, Clone, Serialize)]
pub struct MixHintBiasSnapshot {
    pub snnh_feed_bonus_hits: u32,
    pub snnh_capture_guard_hits: u32,
    pub shsh_feed_bonus_hits: u32,
    pub shsh_capture_guard_hits: u32,
}

#[derive(Debug, Clone, Serialize)]
pub struct HardTelemetrySummary {
    pub record_count: usize,
    pub avg_entropy: f32,
    pub cache_hit_rate: f32,
}

/// Bundles post-decision telemetry data to reduce function parameter counts.
pub struct PostDecisionData<'a> {
    pub think_limit_ms: Option<u32>,
    pub elapsed_ms: u32,
    pub timed_out: bool,
    pub fallback: Option<&'a str>,
    pub search_stats: Option<SearchTelemetrySnapshot>,
    pub controller_bias_delta: Option<i32>,
}

impl TelemetrySink {
    pub fn new(retention: usize) -> Self {
        Self {
            retention: retention.max(1),
            records: RwLock::new(Vec::new()),
        }
    }

    pub fn push(&self, record: HardTelemetryRecord) {
        let mut records = self.records.write();
        records.push(record);
        if records.len() > self.retention {
            let overflow = records.len() - self.retention;
            records.drain(0..overflow);
        }
    }

    pub fn snapshot(&self) -> Vec<HardTelemetryRecord> {
        self.records.read().clone()
    }

    pub fn clear(&self) {
        self.records.write().clear();
    }

    #[allow(dead_code)]
    pub fn summarize(&self) -> HardTelemetrySummary {
        HardTelemetrySummary::from_records(&self.snapshot())
    }

    pub fn export_ndjson(
        &self,
        destination: Option<PathBuf>,
    ) -> io::Result<(PathBuf, HardTelemetrySummary)> {
        let records = self.snapshot();
        let summary = HardTelemetrySummary::from_records(&records);
        let target = resolve_destination(destination)?;
        if let Some(parent) = target.parent() {
            fs::create_dir_all(parent)?;
        }
        let mut file = File::create(&target)?;
        for record in &records {
            let json = serde_json::to_string(record).map_err(io::Error::other)?;
            writeln!(file, "{json}")?;
        }
        drop(file);
        if let Some(parent) = target.parent() {
            enforce_retention(parent, self.retention)?;
        }
        Ok((target, summary))
    }
}

impl HardTelemetryRecord {
    pub fn from_tracker(
        seat: PlayerPosition,
        tracker: &UnseenTracker,
        difficulty: Option<crate::bot::BotDifficulty>,
        phase: Option<&str>,
    ) -> Self {
        let timestamp_ms = now_millis();
        let decision_index = DECISION_COUNTER.fetch_add(1, Ordering::Relaxed);
        let entropy = tracker.belief_entropy();
        let metrics = tracker.belief_cache_metrics();
        HardTelemetryRecord {
            timestamp_ms,
            decision_index,
            seat: seat.to_string(),
            belief_entropy: entropy,
            belief_cache_size: metrics.size,
            belief_cache_capacity: metrics.capacity,
            belief_cache_hits: metrics.hits,
            belief_cache_misses: metrics.misses,
            difficulty: difficulty.map(|d| format!("{:?}", d)),
            phase: phase.map(|p| p.to_string()),
            think_limit_ms: None,
            elapsed_ms: None,
            timed_out: None,
            search_stats: None,
            fallback: None,
            notes: None,
            controller_bias_delta: None,
        }
    }
}

impl SearchTelemetrySnapshot {
    pub fn from_stats(stats: &SearchStats) -> Self {
        let tier = Some(format!("{:?}", stats.tier));
        let limits = stats.limits_in_effect;
        SearchTelemetrySnapshot {
            scanned: stats.scanned,
            scanned_phase_a: stats.scanned_phase_a,
            scanned_phase_b: stats.scanned_phase_b,
            scanned_phase_c: stats.scanned_phase_c,
            tier,
            utilization: Some(stats.utilization),
            phaseb_topk: Some(limits.phaseb_topk as u32),
            next_probe_m: Some(limits.next_probe_m as u32),
            ab_margin: Some(limits.ab_margin),
            continuation_scale_permil: Some(stats.continuation_scale_permil),
            depth2_samples: Some(stats.depth2_samples),
            mix_hint_bias: stats.mix_hint_bias.map(MixHintBiasSnapshot::from),
            controller_bias_delta: stats.controller_bias_delta,
        }
    }
}

impl From<crate::bot::play::MixHintBiasStats> for MixHintBiasSnapshot {
    fn from(stats: crate::bot::play::MixHintBiasStats) -> Self {
        Self {
            snnh_feed_bonus_hits: stats.snnh_feed_bonus_hits,
            snnh_capture_guard_hits: stats.snnh_capture_guard_hits,
            shsh_feed_bonus_hits: stats.shsh_feed_bonus_hits,
            shsh_capture_guard_hits: stats.shsh_capture_guard_hits,
        }
    }
}

impl HardTelemetrySummary {
    pub fn from_records(records: &[HardTelemetryRecord]) -> Self {
        if records.is_empty() {
            return Self {
                record_count: 0,
                avg_entropy: 0.0,
                cache_hit_rate: 0.0,
            };
        }
        let record_count = records.len();
        let mut entropy_sum = 0.0f32;
        let mut hit_total: f32 = 0.0;
        let mut miss_total: f32 = 0.0;
        for record in records {
            entropy_sum += record.belief_entropy.iter().copied().sum::<f32>() / 4.0;
            hit_total += record.belief_cache_hits as f32;
            miss_total += record.belief_cache_misses as f32;
        }
        let avg_entropy = entropy_sum / record_count as f32;
        let cache_hit_rate = if (hit_total + miss_total) > f32::EPSILON {
            hit_total / (hit_total + miss_total)
        } else {
            0.0
        };
        Self {
            record_count,
            avg_entropy,
            cache_hit_rate,
        }
    }
}

fn now_millis() -> u128 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis()
}

fn resolve_destination(destination: Option<PathBuf>) -> io::Result<PathBuf> {
    match destination {
        Some(path) => {
            if path.is_dir() || (path.exists() && path.is_dir()) {
                Ok(path.join(default_filename()))
            } else if path.extension().is_some() {
                Ok(path)
            } else {
                let dir = path;
                Ok(dir.join(default_filename()))
            }
        }
        None => {
            let dir = default_output_dir();
            Ok(dir.join(default_filename()))
        }
    }
}

fn default_output_dir() -> PathBuf {
    PathBuf::from("designs").join("tuning").join("telemetry")
}

fn default_filename() -> String {
    format!("hard_{}.ndjson", now_millis())
}

fn enforce_retention(dir: &Path, retention: usize) -> io::Result<()> {
    let mut entries: Vec<_> = fs::read_dir(dir)?
        .filter_map(|entry| {
            entry.ok().and_then(|e| {
                let path = e.path();
                if path.is_file() {
                    e.metadata()
                        .ok()
                        .and_then(|meta| meta.modified().ok())
                        .map(|modified| (modified, path))
                } else {
                    None
                }
            })
        })
        .collect();
    if entries.len() <= retention {
        return Ok(());
    }
    entries.sort_by(|a, b| a.0.cmp(&b.0));
    let excess = entries.len().saturating_sub(retention);
    if excess == 0 {
        return Ok(());
    }
    for (_, path) in entries.into_iter().take(excess) {
        let _ = fs::remove_file(path);
    }
    Ok(())
}

pub mod hard {
    pub use super::SearchTelemetrySnapshot;
    use super::*;
    #[cfg(test)]
    use std::cell::RefCell;

    #[cfg(test)]
    thread_local! {
        static THREAD_LOCAL_SINK: RefCell<Option<TelemetrySink>> = RefCell::new(None);
    }

    pub fn sink() -> &'static TelemetrySink {
        HARD_SINK.get_or_init(|| TelemetrySink::new(default_retention()))
    }

    fn default_retention() -> usize {
        std::env::var("MDH_HARD_TELEMETRY_KEEP")
            .ok()
            .and_then(|raw| raw.trim().parse::<usize>().ok())
            .unwrap_or(20)
    }

    fn with_active_sink<R>(f: impl FnOnce(&TelemetrySink) -> R) -> R {
        #[cfg(test)]
        {
            return THREAD_LOCAL_SINK.with(|cell| {
                if let Some(ref sink) = *cell.borrow() {
                    f(sink)
                } else {
                    f(sink())
                }
            });
        }
        #[cfg(not(test))]
        {
            f(sink())
        }
    }

    pub fn reset() {
        sink().clear();
        DECISION_COUNTER.store(1, Ordering::Relaxed);
    }

    pub fn record_pre_decision(
        seat: PlayerPosition,
        tracker: &UnseenTracker,
        difficulty: crate::bot::BotDifficulty,
    ) {
        let record =
            HardTelemetryRecord::from_tracker(seat, tracker, Some(difficulty), Some("pre"));
        with_active_sink(|sink| sink.push(record));
    }

    #[cfg_attr(not(target_os = "windows"), allow(dead_code))]
    pub fn record_post_decision(
        seat: PlayerPosition,
        tracker: &UnseenTracker,
        difficulty: crate::bot::BotDifficulty,
        data: PostDecisionData<'_>,
    ) {
        let mut record =
            HardTelemetryRecord::from_tracker(seat, tracker, Some(difficulty), Some("post"));
        record.think_limit_ms = data.think_limit_ms;
        record.elapsed_ms = Some(data.elapsed_ms);
        record.timed_out = Some(data.timed_out);
        record.fallback = data.fallback.map(|f| f.to_string());
        record.search_stats = data.search_stats;
        record.controller_bias_delta = data.controller_bias_delta;
        with_active_sink(|sink| sink.push(record));
    }

    pub fn export(destination: Option<PathBuf>) -> io::Result<(PathBuf, HardTelemetrySummary)> {
        sink().export_ndjson(destination)
    }

    #[allow(dead_code)]
    pub fn summary() -> HardTelemetrySummary {
        sink().summarize()
    }

    #[cfg(test)]
    pub fn capture_for_test<F, R>(f: F) -> (R, Vec<HardTelemetryRecord>)
    where
        F: FnOnce() -> R,
    {
        THREAD_LOCAL_SINK.with(|cell| {
            assert!(
                cell.borrow().is_none(),
                "nested telemetry capture not supported"
            );
            *cell.borrow_mut() = Some(TelemetrySink::new(default_retention()));
            let result = f();
            let sink = cell.borrow_mut().take().unwrap();
            let records = sink.snapshot();
            (result, records)
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn summary_handles_empty_records() {
        let summary = HardTelemetrySummary::from_records(&[]);
        assert_eq!(summary.record_count, 0);
        assert_eq!(summary.avg_entropy, 0.0);
        assert_eq!(summary.cache_hit_rate, 0.0);
    }

    #[test]
    fn summary_accumulates_entropy_and_hits() {
        let records = vec![
            HardTelemetryRecord {
                timestamp_ms: 1,
                decision_index: 1,
                seat: "North".to_string(),
                belief_entropy: [1.0, 1.0, 1.0, 1.0],
                belief_cache_size: 2,
                belief_cache_capacity: 4,
                belief_cache_hits: 2,
                belief_cache_misses: 1,
                difficulty: None,
                phase: None,
                think_limit_ms: None,
                elapsed_ms: None,
                timed_out: None,
                search_stats: None,
                fallback: None,
                notes: None,
                controller_bias_delta: None,
            },
            HardTelemetryRecord {
                timestamp_ms: 2,
                decision_index: 2,
                seat: "East".to_string(),
                belief_entropy: [0.5, 0.5, 0.5, 0.5],
                belief_cache_size: 3,
                belief_cache_capacity: 4,
                belief_cache_hits: 1,
                belief_cache_misses: 0,
                difficulty: None,
                phase: None,
                think_limit_ms: None,
                elapsed_ms: None,
                timed_out: None,
                search_stats: None,
                fallback: None,
                notes: None,
                controller_bias_delta: None,
            },
        ];
        let summary = HardTelemetrySummary::from_records(&records);
        assert_eq!(summary.record_count, 2);
        assert!((summary.avg_entropy - 0.75).abs() < 1e-6);
        assert!((summary.cache_hit_rate - (3.0 / 4.0)).abs() < 1e-6);
    }

    #[test]
    fn telemetry_sink_enforces_retention_on_push() {
        let sink = TelemetrySink::new(2);
        let make_record = |idx: u64| HardTelemetryRecord {
            timestamp_ms: idx as u128,
            decision_index: idx,
            seat: "North".to_string(),
            belief_entropy: [0.0; 4],
            belief_cache_size: 0,
            belief_cache_capacity: 0,
            belief_cache_hits: 0,
            belief_cache_misses: 0,
            difficulty: None,
            phase: None,
            think_limit_ms: None,
            elapsed_ms: None,
            timed_out: None,
            fallback: None,
            search_stats: None,
            notes: None,
            controller_bias_delta: None,
        };
        sink.push(make_record(1));
        sink.push(make_record(2));
        sink.push(make_record(3));
        let snapshot = sink.snapshot();
        assert_eq!(snapshot.len(), 2);
        assert_eq!(snapshot[0].decision_index, 2);
        assert_eq!(snapshot[1].decision_index, 3);
    }
}
