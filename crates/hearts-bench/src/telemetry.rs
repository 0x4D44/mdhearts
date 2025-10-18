use std::collections::BTreeMap;
use std::fs::{File, OpenOptions};
use std::io::{BufRead, BufReader, Write};
use std::path::{Path, PathBuf};

use serde::Serialize;
use serde_json::Value;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum TelemetryError {
    #[error("{context}: {source}")]
    Io {
        context: &'static str,
        #[source]
        source: std::io::Error,
    },
    #[error("failed to parse telemetry JSON: {0}")]
    Json(#[from] serde_json::Error),
}

#[derive(Debug, Default, Serialize)]
pub struct TelemetrySummary {
    pub pass: PassTelemetrySummary,
    pub play: PlayTelemetrySummary,
}

#[derive(Debug, Default, Serialize)]
pub struct PassTelemetrySummary {
    pub count: usize,
    pub avg_total: Option<f64>,
    pub avg_candidates: Option<f64>,
    pub avg_moon_probability: Option<f64>,
    pub avg_best_margin: Option<f64>,
    pub objective_counts: BTreeMap<String, usize>,
}

#[derive(Debug, Default, Serialize)]
pub struct PlayTelemetrySummary {
    pub objective_counts: BTreeMap<String, usize>,
}

#[derive(Debug)]
struct Average {
    sum: f64,
    count: usize,
}

impl Average {
    fn new() -> Self {
        Self { sum: 0.0, count: 0 }
    }

    fn add(&mut self, value: f64) {
        self.sum += value;
        self.count += 1;
    }

    fn mean(&self) -> Option<f64> {
        if self.count == 0 {
            None
        } else {
            Some(self.sum / self.count as f64)
        }
    }
}

/// Aggregate pass/play telemetry emitted by the Stage 2 harness.
pub fn summarise_telemetry(path: &Path) -> Result<TelemetrySummary, TelemetryError> {
    if !path.exists() {
        return Ok(TelemetrySummary::default());
    }

    let file = File::open(path).map_err(|source| TelemetryError::Io {
        context: "opening telemetry log",
        source,
    })?;
    let reader = BufReader::new(file);

    let mut pass_summary = PassTelemetrySummary::default();
    let mut total_avg = Average::new();
    let mut candidate_avg = Average::new();
    let mut moon_prob_avg = Average::new();
    let mut margin_avg = Average::new();

    let mut play_summary = PlayTelemetrySummary::default();

    for line in reader.lines() {
        let line = line.map_err(|source| TelemetryError::Io {
            context: "reading telemetry line",
            source,
        })?;
        if line.trim().is_empty() {
            continue;
        }

        let payload: Value = serde_json::from_str(&line)?;
        let target = payload
            .get("target")
            .and_then(Value::as_str)
            .unwrap_or_default();
        let fields = payload
            .get("fields")
            .and_then(Value::as_object)
            .cloned()
            .unwrap_or_default();

        match target {
            "hearts_bot::pass_decision" => {
                pass_summary.count += 1;

                if let Some(total) = fields.get("total").and_then(Value::as_f64) {
                    total_avg.add(total);
                }
                if let Some(count) = fields
                    .get("candidate_count")
                    .and_then(Value::as_i64)
                    .filter(|v| *v >= 0)
                {
                    candidate_avg.add(count as f64);
                }
                if let Some(prob) = fields.get("moon_probability").and_then(Value::as_f64) {
                    moon_prob_avg.add(prob);
                }

                if let Some(raw_scores) = fields.get("top_scores") {
                    if let Some(delta) = best_margin(raw_scores) {
                        margin_avg.add(delta);
                    }
                }

                let objective = fields
                    .get("moon_objective")
                    .and_then(Value::as_str)
                    .map(|s| s.trim())
                    .filter(|s| !s.is_empty())
                    .unwrap_or("<unset>");
                *pass_summary
                    .objective_counts
                    .entry(objective.to_string())
                    .or_insert(0) += 1;
            }
            "hearts_bot::play" => {
                let objective = fields
                    .get("objective")
                    .and_then(Value::as_str)
                    .map(|s| s.trim())
                    .filter(|s| !s.is_empty())
                    .unwrap_or("<unset>");
                *play_summary
                    .objective_counts
                    .entry(objective.to_string())
                    .or_insert(0) += 1;
            }
            _ => {}
        }
    }

    pass_summary.avg_total = total_avg.mean();
    pass_summary.avg_candidates = candidate_avg.mean();
    pass_summary.avg_moon_probability = moon_prob_avg.mean();
    pass_summary.avg_best_margin = margin_avg.mean();

    Ok(TelemetrySummary {
        pass: pass_summary,
        play: play_summary,
    })
}

fn best_margin(raw_scores: &Value) -> Option<f64> {
    let mut scores = parse_score_list(raw_scores)?;
    if scores.len() < 2 {
        return None;
    }
    scores.sort_by(|a, b| b.partial_cmp(a).unwrap_or(std::cmp::Ordering::Equal));
    Some(scores[0] - scores[1])
}

fn parse_score_list(raw: &Value) -> Option<Vec<f64>> {
    if let Some(array) = raw.as_array() {
        return Some(array.iter().filter_map(Value::as_f64).collect::<Vec<f64>>());
    }

    let text = raw.as_str()?;
    serde_json::from_str::<Vec<f64>>(text).ok()
}

pub fn write_summary_outputs(
    telemetry_path: &Path,
    output_dir: &Path,
) -> Result<Option<TelemetryOutputs>, TelemetryError> {
    if !telemetry_path.exists() {
        return Ok(None);
    }

    let summary = summarise_telemetry(telemetry_path)?;
    let json_path = output_dir.join("telemetry_summary.json");
    let md_path = output_dir.join("telemetry_summary.md");

    std::fs::write(
        &json_path,
        serde_json::to_vec_pretty(&summary).map_err(TelemetryError::from)?,
    )
    .map_err(|source| TelemetryError::Io {
        context: "writing telemetry summary json",
        source,
    })?;

    let markdown = render_markdown(&summary, telemetry_path);
    std::fs::write(&md_path, markdown).map_err(|source| TelemetryError::Io {
        context: "writing telemetry summary markdown",
        source,
    })?;

    Ok(Some(TelemetryOutputs {
        summary,
        json_path,
        markdown_path: md_path,
    }))
}

pub fn append_highlights_to_markdown(
    summary_path: &Path,
    outputs: &TelemetryOutputs,
) -> Result<(), TelemetryError> {
    let mut file = OpenOptions::new()
        .create(true)
        .append(true)
        .open(summary_path)
        .map_err(|source| TelemetryError::Io {
            context: "opening summary markdown for telemetry append",
            source,
        })?;

    let mut section = String::new();
    section.push_str("\n## Telemetry Highlights\n");
    let pass = &outputs.summary.pass;
    section.push_str(&format!("- Pass events captured: {}\n", pass.count));
    if let Some(value) = pass.avg_total {
        section.push_str(&format!("- Avg pass score: {:.2}\n", value));
    }
    if let Some(value) = pass.avg_candidates {
        section.push_str(&format!("- Avg candidates evaluated: {:.2}\n", value));
    }
    if let Some(value) = pass.avg_moon_probability {
        section.push_str(&format!("- Avg moon probability: {:.3}\n", value));
    }
    if let Some(value) = pass.avg_best_margin {
        section.push_str(&format!("- Avg best vs next margin: {:.2}\n", value));
    }
    if !pass.objective_counts.is_empty() {
        section.push_str("- Moon objectives:\n");
        for (label, count) in &pass.objective_counts {
            section.push_str(&format!("  - {}: {}\n", label, count));
        }
    }

    let play = &outputs.summary.play;
    section.push_str("\n### Play Objectives\n");
    if play.objective_counts.is_empty() {
        section.push_str("- <none>\n");
    } else {
        for (label, count) in &play.objective_counts {
            section.push_str(&format!("- {}: {}\n", label, count));
        }
    }

    write!(file, "{section}").map_err(|source| TelemetryError::Io {
        context: "writing telemetry highlights",
        source,
    })?;

    Ok(())
}

fn render_markdown(summary: &TelemetrySummary, telemetry_path: &Path) -> String {
    let mut output = String::new();
    output.push_str("# Telemetry Summary\n\n");
    output.push_str(&format!("- Source: `{}`\n", telemetry_path.display()));
    output.push('\n');

    output.push_str("## Pass Decisions\n");
    output.push_str(&format!("- Events: {}\n", summary.pass.count));
    if let Some(value) = summary.pass.avg_total {
        output.push_str(&format!("- Avg total score: {:.2}\n", value));
    }
    if let Some(value) = summary.pass.avg_candidates {
        output.push_str(&format!("- Avg candidates: {:.2}\n", value));
    }
    if let Some(value) = summary.pass.avg_moon_probability {
        output.push_str(&format!("- Avg moon probability: {:.3}\n", value));
    }
    if let Some(value) = summary.pass.avg_best_margin {
        output.push_str(&format!("- Avg best vs next margin: {:.2}\n", value));
    }
    if !summary.pass.objective_counts.is_empty() {
        output.push_str("- Objectives:\n");
        for (label, count) in &summary.pass.objective_counts {
            output.push_str(&format!("  - {}: {}\n", label, count));
        }
    }
    output.push('\n');

    output.push_str("## Play Objectives\n");
    if summary.play.objective_counts.is_empty() {
        output.push_str("- <none>\n");
    } else {
        for (label, count) in &summary.play.objective_counts {
            output.push_str(&format!("- {}: {}\n", label, count));
        }
    }
    output
}

#[derive(Debug)]
pub struct TelemetryOutputs {
    pub summary: TelemetrySummary,
    pub json_path: PathBuf,
    pub markdown_path: PathBuf,
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::BTreeMap;
    use std::io::Write;

    fn write_temp_file(lines: &[&str]) -> tempfile::NamedTempFile {
        let mut file = tempfile::NamedTempFile::new().expect("create temp file");
        for line in lines {
            writeln!(file, "{line}").expect("write line");
        }
        file
    }

    #[test]
    fn summarises_pass_and_play_events() {
        let lines = vec![
            r#"{"target":"hearts_bot::pass_decision","fields":{"total":100.0,"candidate_count":20,"moon_probability":0.4,"moon_objective":"pph","top_scores":"[100.0, 90.0, 70.0]"}}"#,
            r#"{"target":"hearts_bot::pass_decision","fields":{"total":80.0,"candidate_count":10,"moon_probability":0.2,"moon_objective":"","top_scores":"[80.0]"}}"#,
            r#"{"target":"hearts_bot::play","fields":{"objective":"BlockShooter"}}"#,
            r#"{"target":"hearts_bot::play","fields":{"objective":"MyPointsPerHand"}}"#,
        ];
        let file = write_temp_file(&lines);
        let summary = summarise_telemetry(file.path()).expect("summarise");
        assert_eq!(summary.pass.count, 2);
        assert!(summary.pass.avg_total.unwrap() > 89.9);
        assert!(summary.pass.avg_candidates.unwrap() > 14.9);
        assert!(summary.pass.avg_moon_probability.unwrap() > 0.29);
        assert!(summary.pass.avg_best_margin.unwrap() > 9.9);
        assert_eq!(summary.pass.objective_counts.get("pph"), Some(&1));
        assert_eq!(summary.pass.objective_counts.get("<unset>"), Some(&1));
        assert_eq!(summary.play.objective_counts.get("BlockShooter"), Some(&1));
        assert_eq!(
            summary.play.objective_counts.get("MyPointsPerHand"),
            Some(&1)
        );
    }

    #[test]
    fn handles_missing_file() {
        let path = Path::new("tests/does/not/exist.jsonl");
        let summary = summarise_telemetry(path).expect("summarise missing file");
        assert_eq!(summary.pass.count, 0);
        assert!(summary.pass.avg_total.is_none());
        assert!(summary.play.objective_counts.is_empty());
    }

    #[test]
    fn appends_highlights_to_summary_markdown() {
        let mut summary_file = tempfile::NamedTempFile::new().expect("summary temp file");
        write!(summary_file, "# Tournament Summary\n").expect("seed summary content");
        let telemetry_json = tempfile::NamedTempFile::new().expect("telemetry json temp");
        let telemetry_md = tempfile::NamedTempFile::new().expect("telemetry md temp");

        let mut pass_counts = BTreeMap::new();
        pass_counts.insert("pph".to_string(), 12);
        pass_counts.insert("block_shooter".to_string(), 4);
        let mut play_counts = BTreeMap::new();
        play_counts.insert("BlockShooter".to_string(), 9);

        let outputs = TelemetryOutputs {
            summary: TelemetrySummary {
                pass: PassTelemetrySummary {
                    count: 16,
                    avg_total: Some(104.5),
                    avg_candidates: Some(18.0),
                    avg_moon_probability: Some(0.215),
                    avg_best_margin: Some(6.4),
                    objective_counts: pass_counts,
                },
                play: PlayTelemetrySummary {
                    objective_counts: play_counts,
                },
            },
            json_path: telemetry_json.path().to_path_buf(),
            markdown_path: telemetry_md.path().to_path_buf(),
        };

        append_highlights_to_markdown(summary_file.path(), &outputs).expect("append highlights");

        let contents = std::fs::read_to_string(summary_file.path()).expect("read summary file");
        assert!(contents.contains("## Telemetry Highlights"));
        assert!(contents.contains("Pass events captured: 16"));
        assert!(contents.contains("Avg pass score: 104.50"));
        assert!(contents.contains("pph: 12"));
        assert!(contents.contains("### Play Objectives"));
        assert!(contents.contains("BlockShooter: 9"));
    }
}
