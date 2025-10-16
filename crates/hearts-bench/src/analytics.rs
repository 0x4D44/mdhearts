use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};

use plotters::prelude::*;
use serde::Serialize;
use statrs::distribution::{ContinuousCDF, Normal};
use thiserror::Error;

use crate::config::{AgentConfig, AgentKind, BenchmarkConfig};
use crate::tournament::{DecisionSummary, HandOutcome};

const CONFIDENCE_Z: f64 = 1.96; // 95% CI

#[derive(Debug, Error)]
pub enum AnalyticsError {
    #[error("baseline agent '{0}' not present in tournament results")]
    MissingBaseline(String),
    #[error("agent '{0}' defined in results but missing from configuration")]
    UnknownAgent(String),
    #[error("baseline '{0}' missing for deal {1}")]
    MissingBaselineDeal(String, String),
    #[error("{context}: {source}")]
    Io {
        context: &'static str,
        #[source]
        source: std::io::Error,
    },
    #[error("failed to render plot: {0}")]
    Plot(String),
}

pub struct AnalyticsCollector {
    baseline: String,
    agents: HashMap<String, AgentAccumulator>,
    comparisons: HashMap<String, ComparisonAccumulator>,
    agent_order: Vec<String>,
    latency_budget_ms: u64,
}

impl AnalyticsCollector {
    pub fn new(config: &BenchmarkConfig) -> Result<Self, AnalyticsError> {
        let baseline = config
            .metrics
            .baseline
            .clone()
            .ok_or_else(|| AnalyticsError::MissingBaseline("<unset>".into()))?;

        let mut agents = HashMap::new();
        let mut order = Vec::new();
        for agent in &config.agents {
            agents.insert(
                agent.name.clone(),
                AgentAccumulator::new(
                    agent.name.clone(),
                    agent.clone(),
                    config.metrics.latency_budget_ms,
                ),
            );
            order.push(agent.name.clone());
        }

        Ok(Self {
            baseline,
            agents,
            comparisons: HashMap::new(),
            agent_order: order,
            latency_budget_ms: config.metrics.latency_budget_ms,
        })
    }

    pub fn record_hand(
        &mut self,
        hand_index: usize,
        permutation_index: usize,
        outcome: &HandOutcome,
    ) -> Result<(), AnalyticsError> {
        let deal_id = format!("H{hand_index:05}_P{permutation_index:02}");

        // Determine winner threshold and baseline points
        let winner_points = outcome
            .seat_results
            .iter()
            .map(|seat| seat.points as u32)
            .min()
            .unwrap_or(0);

        let baseline_points = outcome
            .seat_results
            .iter()
            .find(|seat| seat.agent_name == self.baseline)
            .map(|seat| seat.points as f64)
            .ok_or_else(|| {
                AnalyticsError::MissingBaselineDeal(self.baseline.clone(), deal_id.clone())
            })?;

        for seat in &outcome.seat_results {
            let acc = self
                .agents
                .get_mut(&seat.agent_name)
                .ok_or_else(|| AnalyticsError::UnknownAgent(seat.agent_name.clone()))?;

            acc.record_hand(
                seat.points as f64,
                seat.points as u32 == winner_points,
                outcome.moon_shooter == Some(seat.seat),
                &seat.metrics,
            );
        }

        for seat in &outcome.seat_results {
            if seat.agent_name == self.baseline {
                continue;
            }
            let diff = seat.points as f64 - baseline_points;
            self.comparisons
                .entry(seat.agent_name.clone())
                .or_insert_with(ComparisonAccumulator::new)
                .record(diff);
        }

        Ok(())
    }

    pub fn finalize(mut self) -> Result<AnalyticsSummary, AnalyticsError> {
        let mut reports = Vec::new();
        for name in &self.agent_order {
            if let Some(acc) = self.agents.remove(name) {
                reports.push(acc.into_report());
            }
        }

        let mut comparisons = Vec::new();
        for report in &reports {
            if report.name == self.baseline {
                comparisons.push(ComparisonReport {
                    agent: report.name.clone(),
                    p_value: 1.0,
                    sample_size: report.hands,
                });
                continue;
            }
            if let Some(comp) = self.comparisons.remove(&report.name) {
                let (p_value, sample_size) = comp.wilcoxon_signed_rank();
                comparisons.push(ComparisonReport {
                    agent: report.name.clone(),
                    p_value,
                    sample_size,
                });
            } else {
                comparisons.push(ComparisonReport {
                    agent: report.name.clone(),
                    p_value: 1.0,
                    sample_size: 0,
                });
            }
        }

        Ok(AnalyticsSummary {
            baseline: self.baseline,
            agents: reports,
            comparisons,
            latency_budget_ms: self.latency_budget_ms,
        }
        .enrich())
    }
}

struct AgentAccumulator {
    name: String,
    config: AgentConfig,
    total_points: f64,
    hands: u32,
    wins: u32,
    moon_shots: u32,
    per_hand_points: Vec<f64>,
    total_latency_ms: f64,
    total_decisions: u64,
    latency_budget_ms: u64,
}

impl AgentAccumulator {
    fn new(name: String, config: AgentConfig, latency_budget_ms: u64) -> Self {
        Self {
            name,
            config,
            total_points: 0.0,
            hands: 0,
            wins: 0,
            moon_shots: 0,
            per_hand_points: Vec::new(),
            total_latency_ms: 0.0,
            total_decisions: 0,
            latency_budget_ms,
        }
    }

    fn record_hand(
        &mut self,
        points: f64,
        is_winner: bool,
        is_moon_shooter: bool,
        metrics: &DecisionSummary,
    ) {
        self.total_points += points;
        self.hands += 1;
        self.per_hand_points.push(points);
        if is_winner {
            self.wins += 1;
        }
        if is_moon_shooter {
            self.moon_shots += 1;
        }
        self.total_latency_ms += metrics.total_ms;
        self.total_decisions += metrics.decisions as u64;
    }

    fn into_report(self) -> AgentReport {
        let avg_pph = if self.hands == 0 {
            0.0
        } else {
            self.total_points / self.hands as f64
        };

        let (ci_low, ci_high) = confidence_interval(&self.per_hand_points);

        let avg_latency = if self.total_decisions == 0 {
            0.0
        } else {
            self.total_latency_ms / self.total_decisions as f64
        };

        AgentReport {
            name: self.name.clone(),
            kind: self.config.kind.clone(),
            params: self.config.params.clone(),
            hands: self.hands as usize,
            avg_pph,
            ci95: (ci_low, ci_high),
            wins: self.wins as usize,
            moon_shots: self.moon_shots as usize,
            average_ms_per_decision: avg_latency,
            delta_vs_baseline: 0.0, // Filled later once we know baseline report
            over_budget: avg_latency > self.latency_budget_ms as f64,
        }
    }
}

#[derive(Clone)]
struct ComparisonAccumulator {
    diffs: Vec<f64>,
}

impl ComparisonAccumulator {
    fn new() -> Self {
        Self { diffs: Vec::new() }
    }

    fn record(&mut self, diff: f64) {
        self.diffs.push(diff);
    }

    fn wilcoxon_signed_rank(self) -> (f64, usize) {
        let diffs: Vec<f64> = self
            .diffs
            .into_iter()
            .filter(|d| d.abs() > f64::EPSILON)
            .collect();
        let n = diffs.len();
        if n == 0 {
            return (1.0, 0);
        }

        let mut paired: Vec<(f64, f64)> =
            diffs.into_iter().map(|d| (d.abs(), d.signum())).collect();
        paired.sort_by(|a, b| a.0.partial_cmp(&b.0).unwrap());

        // Rank handling with ties
        let mut ranks = Vec::with_capacity(n);
        let mut tie_sizes = Vec::new();
        let mut i = 0;
        while i < paired.len() {
            let mut j = i;
            while j + 1 < paired.len() && (paired[j + 1].0 - paired[i].0).abs() < 1e-12 {
                j += 1;
            }
            let rank = (i + j + 2) as f64 / 2.0;
            for k in i..=j {
                ranks.push((rank, paired[k].1));
            }
            if j > i {
                tie_sizes.push(j - i + 1);
            }
            i = j + 1;
        }

        let w_plus: f64 = ranks
            .iter()
            .filter(|(_, sign)| *sign > 0.0)
            .map(|(rank, _)| *rank)
            .sum();
        let w_minus: f64 = ranks
            .iter()
            .filter(|(_, sign)| *sign < 0.0)
            .map(|(rank, _)| *rank)
            .sum();

        let w = w_plus.min(w_minus);
        let n_f = n as f64;
        let mean_w = n_f * (n_f + 1.0) / 4.0;

        // Variance with tie correction
        let tie_adjustment: f64 = tie_sizes
            .into_iter()
            .map(|count| {
                let c = count as f64;
                (c.powi(3) - c) / 48.0
            })
            .sum();
        let variance_w = n_f * (n_f + 1.0) * (2.0 * n_f + 1.0) / 24.0 - tie_adjustment;
        if variance_w <= 0.0 {
            return (1.0, n);
        }

        let z = ((w - mean_w).abs() - 0.5) / variance_w.sqrt();
        let normal = Normal::new(0.0, 1.0).unwrap();
        let p = 2.0 * (1.0 - normal.cdf(z));
        (p.min(1.0).max(0.0), n)
    }
}

#[derive(Debug, Serialize)]
pub struct AnalyticsSummary {
    pub baseline: String,
    pub agents: Vec<AgentReport>,
    pub comparisons: Vec<ComparisonReport>,
    pub latency_budget_ms: u64,
}

impl AnalyticsSummary {
    pub fn enrich(mut self) -> Self {
        let baseline_avg = self
            .agents
            .iter()
            .find(|agent| agent.name == self.baseline)
            .map(|agent| agent.avg_pph)
            .unwrap_or(0.0);

        for agent in &mut self.agents {
            agent.delta_vs_baseline = agent.avg_pph - baseline_avg;
        }

        self
    }

    pub fn write_markdown(&self, path: impl AsRef<Path>) -> Result<(), AnalyticsError> {
        let mut rows = String::new();
        rows.push_str("# Tournament Summary\n\n");
        rows.push_str(&format!(
            "Latency budget: {} ms average per decision\n\n",
            self.latency_budget_ms
        ));
        rows.push_str("| Agent | Kind | Hands | Avg PPH | Δ vs baseline | 95% CI | Win % | Moon % | Avg ms/decision | Over Budget | p-value |\n");
        rows.push_str("|-------|------|-------|---------|----------------|--------|-------|--------|------------------|-------------|---------|\n");

        for agent in &self.agents {
            let comparison = self
                .comparisons
                .iter()
                .find(|c| c.agent == agent.name)
                .map(|c| c.p_value)
                .unwrap_or(1.0);
            let win_rate = if agent.hands == 0 {
                0.0
            } else {
                agent.wins as f64 / agent.hands as f64
            };
            let moon_rate = if agent.hands == 0 {
                0.0
            } else {
                agent.moon_shots as f64 / agent.hands as f64
            };

            rows.push_str(&format!(
                "| {name} | {kind:?} | {hands} | {avg:.3} | {delta:+.3} | [{ci_low:.3}, {ci_high:.3}] | {win:.1}% | {moon:.1}% | {latency:.2} | {over_budget} | {pval:.3} |\n",
                name = agent.name,
                kind = agent.kind,
                hands = agent.hands,
                avg = agent.avg_pph,
                delta = agent.delta_vs_baseline,
                ci_low = agent.ci95.0,
                ci_high = agent.ci95.1,
                win = win_rate * 100.0,
                moon = moon_rate * 100.0,
                latency = agent.average_ms_per_decision,
                over_budget = if agent.over_budget { "Yes" } else { "No" },
                pval = comparison,
            ));
        }

        fs::write(path.as_ref(), rows).map_err(|e| AnalyticsError::Io {
            context: "writing summary markdown",
            source: e,
        })?;
        Ok(())
    }

    pub fn render_plot(&self, dir: impl AsRef<Path>) -> Result<PathBuf, AnalyticsError> {
        let dir = dir.as_ref();
        if !dir.as_os_str().is_empty() {
            fs::create_dir_all(dir).map_err(|e| AnalyticsError::Io {
                context: "creating plots directory",
                source: e,
            })?;
        }

        let output_path = dir.join("delta_pph.png");
        let baseline = self.baseline.clone();
        let agents_snapshot = self.agents.clone();

        let prev_hook = std::panic::take_hook();
        std::panic::set_hook(Box::new(|_| {}));

        let plot_attempt = std::panic::catch_unwind(move || {
            let root = BitMapBackend::new(&output_path, (800, 480)).into_drawing_area();
            root.fill(&WHITE)
                .map_err(|e| AnalyticsError::Plot(e.to_string()))?;

            let mut agents = agents_snapshot;
            agents.sort_by(|a, b| {
                a.delta_vs_baseline
                    .partial_cmp(&b.delta_vs_baseline)
                    .unwrap()
            });

            let y_range_min = agents
                .iter()
                .map(|a| a.delta_vs_baseline)
                .fold(0.0f64, |acc, v| acc.min(v));
            let y_range_max = agents
                .iter()
                .map(|a| a.delta_vs_baseline)
                .fold(0.0f64, |acc, v| acc.max(v));
            let margin = ((y_range_max - y_range_min).abs() * 0.1).max(0.2);

            let mut chart = ChartBuilder::on(&root)
                .margin(20)
                .caption(
                    "PPH delta vs baseline (lower is better)",
                    ("sans-serif", 22),
                )
                .set_label_area_size(LabelAreaPosition::Left, 50)
                .set_label_area_size(LabelAreaPosition::Bottom, 60)
                .build_cartesian_2d(
                    0..agents.len(),
                    (y_range_min - margin)..(y_range_max + margin),
                )
                .map_err(|e| AnalyticsError::Plot(e.to_string()))?;

            chart
                .configure_mesh()
                .disable_mesh()
                .y_desc("Δ PPH vs baseline")
                .x_desc("Agent")
                .x_label_formatter(&|idx| {
                    agents
                        .get(*idx)
                        .map(|agent| agent.name.clone())
                        .unwrap_or_default()
                })
                .draw()
                .map_err(|e| AnalyticsError::Plot(e.to_string()))?;

            chart
                .draw_series(agents.iter().enumerate().map(|(idx, agent)| {
                    let color = if agent.name == baseline {
                        &BLUE
                    } else if agent.delta_vs_baseline <= 0.0 {
                        &GREEN
                    } else {
                        &RED
                    };
                    Rectangle::new(
                        [(idx, 0.0), (idx + 1, agent.delta_vs_baseline)],
                        color.filled(),
                    )
                }))
                .map_err(|e| AnalyticsError::Plot(e.to_string()))?;

            drop(chart);

            root.present()
                .map_err(|e| AnalyticsError::Plot(e.to_string()))?;

            drop(root);

            Ok(output_path)
        });

        std::panic::set_hook(prev_hook);

        match plot_attempt {
            Ok(result) => result,
            Err(_) => Err(AnalyticsError::Plot(
                "plotters panicked while rendering (missing font support?)".into(),
            )),
        }
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct AgentReport {
    pub name: String,
    pub kind: AgentKind,
    pub params: serde_yaml::Value,
    pub hands: usize,
    pub avg_pph: f64,
    pub ci95: (f64, f64),
    pub wins: usize,
    pub moon_shots: usize,
    pub average_ms_per_decision: f64,
    #[serde(skip)]
    pub delta_vs_baseline: f64,
    #[serde(skip)]
    pub over_budget: bool,
}

#[derive(Debug, Clone, Serialize)]
pub struct ComparisonReport {
    pub agent: String,
    pub p_value: f64,
    pub sample_size: usize,
}

fn confidence_interval(points: &[f64]) -> (f64, f64) {
    if points.is_empty() {
        return (0.0, 0.0);
    }
    let mean = points.iter().sum::<f64>() / points.len() as f64;
    if points.len() == 1 {
        return (mean, mean);
    }
    let variance = points
        .iter()
        .map(|value| (value - mean).powi(2))
        .sum::<f64>()
        / (points.len() as f64 - 1.0);
    let std_error = (variance / points.len() as f64).sqrt();
    let margin = CONFIDENCE_Z * std_error;
    (mean - margin, mean + margin)
}
