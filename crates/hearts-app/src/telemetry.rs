use crate::bot::UnseenTracker;
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
    pub notes: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct HardTelemetrySummary {
    pub record_count: usize,
    pub avg_entropy: f32,
    pub cache_hit_rate: f32,
}

impl TelemetrySink {
    pub fn new(retention: usize) -> Self {
        Self {
            retention: retention.max(1),
            records: RwLock::new(Vec::new()),
        }
    }

    pub fn push(&self, record: HardTelemetryRecord) {
        self.records.write().push(record);
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
            let json = serde_json::to_string(record)
                .map_err(|err| io::Error::new(io::ErrorKind::Other, err))?;
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
    pub fn from_tracker(seat: PlayerPosition, tracker: &UnseenTracker) -> Self {
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
            notes: None,
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
    use super::*;

    pub fn sink() -> &'static TelemetrySink {
        HARD_SINK.get_or_init(|| {
            let retention = std::env::var("MDH_HARD_TELEMETRY_KEEP")
                .ok()
                .and_then(|raw| raw.trim().parse::<usize>().ok())
                .unwrap_or(20);
            TelemetrySink::new(retention)
        })
    }

    pub fn reset() {
        sink().clear();
        DECISION_COUNTER.store(1, Ordering::Relaxed);
    }

    pub fn record_pre_decision(seat: PlayerPosition, tracker: &UnseenTracker) {
        let record = HardTelemetryRecord::from_tracker(seat, tracker);
        sink().push(record);
    }

    pub fn export(destination: Option<PathBuf>) -> io::Result<(PathBuf, HardTelemetrySummary)> {
        sink().export_ndjson(destination)
    }

    #[allow(dead_code)]
    pub fn summary() -> HardTelemetrySummary {
        sink().summarize()
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
                notes: None,
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
                notes: None,
            },
        ];
        let summary = HardTelemetrySummary::from_records(&records);
        assert_eq!(summary.record_count, 2);
        assert!((summary.avg_entropy - 0.75).abs() < 1e-6);
        assert!((summary.cache_hit_rate - (3.0 / 4.0)).abs() < 1e-6);
    }
}
