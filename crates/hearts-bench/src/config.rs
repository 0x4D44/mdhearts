use serde::{Deserialize, Serialize};
use std::collections::HashSet;
use std::fs::File;
use std::io::BufReader;
use std::path::{Path, PathBuf};
use thiserror::Error;
use tracing::Level;

const DEFAULT_SEAT_PERMUTATIONS: usize = 14;
const DEFAULT_LATENCY_BUDGET_MS: u64 = 1_200;
const RUN_ID_ALLOWED: &str = "abcdefghijklmnopqrstuvwxyzABCDEFGHIJKLMNOPQRSTUVWXYZ0123456789._-";

/// Root benchmark configuration loaded from YAML.
#[derive(Debug, Clone, Deserialize, PartialEq)]
pub struct BenchmarkConfig {
    pub run_id: String,
    pub deals: DealConfig,
    pub agents: Vec<AgentConfig>,
    pub outputs: OutputsConfig,
    #[serde(default)]
    pub metrics: MetricsConfig,
    #[serde(default)]
    pub logging: LoggingConfig,
}

impl BenchmarkConfig {
    /// Load configuration from a YAML file on disk.
    pub fn from_path(path: impl AsRef<Path>) -> Result<Self, ConfigError> {
        let path = path.as_ref();
        let path_buf = path.to_path_buf();
        let file = File::open(path).map_err(|source| ConfigError::Read {
            source,
            path: path_buf.clone(),
        })?;
        let reader = BufReader::new(file);
        let mut cfg: BenchmarkConfig =
            serde_yaml::from_reader(reader).map_err(|source| ConfigError::Parse {
                source,
                path: path_buf.clone(),
            })?;
        cfg.validate().map_err(|source| ConfigError::Invalid {
            path: path_buf,
            source,
        })?;
        Ok(cfg)
    }

    /// Validate the configuration without performing I/O.
    pub fn validate(&mut self) -> Result<(), ValidationError> {
        validate_run_id(&self.run_id)?;
        self.deals.validate()?;
        self.outputs.validate(&self.run_id)?;
        self.metrics.validate(&self.agents)?;
        self.logging.normalize();
        validate_agents(&mut self.agents)?;
        Ok(())
    }

    /// Resolve output templates (e.g., `{run_id}` placeholders) into concrete paths.
    pub fn resolved_outputs(&self) -> ResolvedOutputs {
        ResolvedOutputs {
            jsonl: resolve_template(&self.run_id, &self.outputs.jsonl),
            summary_md: resolve_template(&self.run_id, &self.outputs.summary_md),
            plots_dir: resolve_template(&self.run_id, &self.outputs.plots_dir),
        }
    }
}

/// Deal sampling configuration block.
#[derive(Debug, Clone, Deserialize, PartialEq)]
pub struct DealConfig {
    pub seed: Option<u64>,
    pub hands: usize,
    #[serde(default = "default_permutations")]
    pub permutations: usize,
}

impl DealConfig {
    fn validate(&self) -> Result<(), ValidationError> {
        if self.hands == 0 {
            return Err(ValidationError::InvalidField {
                field: "deals.hands".to_string(),
                message: "number of hands must be greater than zero".to_string(),
            });
        }

        if self.permutations == 0 {
            return Err(ValidationError::InvalidField {
                field: "deals.permutations".to_string(),
                message: "permutations must be at least 1".to_string(),
            });
        }

        Ok(())
    }
}

fn default_permutations() -> usize {
    DEFAULT_SEAT_PERMUTATIONS
}

/// Definition of a tournament participant.
#[derive(Debug, Clone, Deserialize, PartialEq)]
pub struct AgentConfig {
    pub name: String,
    pub kind: AgentKind,
    #[serde(default)]
    pub params: serde_yaml::Value,
}

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum AgentKind {
    Heuristic,
    External,
    Embedded,
}

/// Output artifact configuration.
#[derive(Debug, Clone, Deserialize, PartialEq)]
pub struct OutputsConfig {
    pub jsonl: String,
    pub summary_md: String,
    pub plots_dir: String,
}

impl OutputsConfig {
    fn validate(&self, run_id: &str) -> Result<(), ValidationError> {
        for (label, value) in [
            ("outputs.jsonl", &self.jsonl),
            ("outputs.summary_md", &self.summary_md),
            ("outputs.plots_dir", &self.plots_dir),
        ] {
            if value.trim().is_empty() {
                return Err(ValidationError::InvalidField {
                    field: label.to_string(),
                    message: "path must not be empty".to_string(),
                });
            }

            let resolved = resolve_template(run_id, value);
            if resolved.components().count() == 0 {
                return Err(ValidationError::InvalidField {
                    field: label.to_string(),
                    message: "resolved path is invalid".to_string(),
                });
            }
        }
        Ok(())
    }
}

/// Metrics configuration block.
#[derive(Debug, Clone, Deserialize, PartialEq)]
pub struct MetricsConfig {
    #[serde(default)]
    pub baseline: Option<String>,
    #[serde(default = "default_latency_budget_ms")]
    pub latency_budget_ms: u64,
}

impl Default for MetricsConfig {
    fn default() -> Self {
        Self {
            baseline: None,
            latency_budget_ms: DEFAULT_LATENCY_BUDGET_MS,
        }
    }
}

impl MetricsConfig {
    fn validate(&self, agents: &[AgentConfig]) -> Result<(), ValidationError> {
        let Some(baseline) = self.baseline.as_ref() else {
            return Err(ValidationError::InvalidField {
                field: "metrics.baseline".to_string(),
                message: "baseline agent must be specified".to_string(),
            });
        };

        if !agents.iter().any(|a| &a.name == baseline) {
            return Err(ValidationError::InvalidField {
                field: "metrics.baseline".to_string(),
                message: format!("baseline agent '{baseline}' is not defined in agents list"),
            });
        }

        if self.latency_budget_ms == 0 {
            return Err(ValidationError::InvalidField {
                field: "metrics.latency_budget_ms".to_string(),
                message: "latency budget must be greater than zero".to_string(),
            });
        }

        Ok(())
    }
}

fn default_latency_budget_ms() -> u64 {
    DEFAULT_LATENCY_BUDGET_MS
}

/// Logging configuration defaults to disabled structured logs.
#[derive(Debug, Clone, Deserialize, PartialEq)]
pub struct LoggingConfig {
    #[serde(default)]
    pub enable_structured: bool,
    #[serde(default = "default_tracing_level")]
    pub tracing_level: String,
    #[serde(default)]
    pub pass_details: bool,
    #[serde(default)]
    pub moon_details: bool,
}

impl Default for LoggingConfig {
    fn default() -> Self {
        Self {
            enable_structured: false,
            tracing_level: default_tracing_level(),
            pass_details: false,
            moon_details: false,
        }
    }
}

impl LoggingConfig {
    fn normalize(&mut self) {
        if self.tracing_level.trim().is_empty() {
            self.tracing_level = default_tracing_level();
        }
    }

    pub fn level(&self) -> Option<Level> {
        match self.tracing_level.to_ascii_lowercase().as_str() {
            "trace" => Some(Level::TRACE),
            "debug" => Some(Level::DEBUG),
            "info" => Some(Level::INFO),
            "warn" | "warning" => Some(Level::WARN),
            "error" => Some(Level::ERROR),
            _ => None,
        }
    }
}

fn default_tracing_level() -> String {
    "info".to_string()
}

fn validate_run_id(run_id: &str) -> Result<(), ValidationError> {
    if run_id.trim().is_empty() {
        return Err(ValidationError::InvalidField {
            field: "run_id".to_string(),
            message: "run_id must not be empty".to_string(),
        });
    }

    if !run_id.chars().all(|c| RUN_ID_ALLOWED.contains(c)) {
        return Err(ValidationError::InvalidField {
            field: "run_id".to_string(),
            message: "run_id may only contain alphanumeric characters, '.', '_' or '-'".to_string(),
        });
    }

    Ok(())
}

fn validate_agents(agents: &mut [AgentConfig]) -> Result<(), ValidationError> {
    if agents.is_empty() {
        return Err(ValidationError::InvalidField {
            field: "agents".to_string(),
            message: "at least one agent must be specified".to_string(),
        });
    }

    let mut seen = HashSet::new();
    for agent in agents.iter_mut() {
        if agent.name.trim().is_empty() {
            return Err(ValidationError::InvalidField {
                field: "agents.name".to_string(),
                message: "agent name must not be empty".to_string(),
            });
        }

        if !agent
            .name
            .chars()
            .all(|c| RUN_ID_ALLOWED.contains(c) || c == '/')
        {
            return Err(ValidationError::InvalidField {
                field: format!("agents[{}].name", agent.name),
                message: "agent name contains invalid characters".to_string(),
            });
        }

        if !seen.insert(agent.name.clone()) {
            return Err(ValidationError::InvalidField {
                field: "agents".to_string(),
                message: format!("agent name '{}' defined more than once", agent.name),
            });
        }

        if agent.params.is_null() {
            agent.params = serde_yaml::Value::Mapping(Default::default());
        }
    }

    Ok(())
}

fn resolve_template(run_id: &str, template: &str) -> PathBuf {
    let replaced = template.replace("{run_id}", run_id);
    PathBuf::from(replaced)
}

/// Fully resolved output paths.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ResolvedOutputs {
    pub jsonl: PathBuf,
    pub summary_md: PathBuf,
    pub plots_dir: PathBuf,
}

/// Errors surfaced when loading configuration files.
#[derive(Debug, Error)]
pub enum ConfigError {
    #[error("failed to read config {path:?}: {source}")]
    Read {
        #[source]
        source: std::io::Error,
        path: PathBuf,
    },
    #[error("failed to parse config {path:?}: {source}")]
    Parse {
        #[source]
        source: serde_yaml::Error,
        path: PathBuf,
    },
    #[error("invalid configuration in {path:?}: {source}")]
    Invalid {
        path: PathBuf,
        source: ValidationError,
    },
}

impl ConfigError {
    pub fn path(&self) -> &Path {
        match self {
            ConfigError::Read { path, .. }
            | ConfigError::Parse { path, .. }
            | ConfigError::Invalid { path, .. } => path.as_path(),
        }
    }
}

/// Validation failures captured with contextual metadata.
#[derive(Debug, Error)]
pub enum ValidationError {
    #[error("{field}: {message}")]
    InvalidField { field: String, message: String },
}

#[cfg(test)]
mod tests {
    use super::*;

    const BASIC_YAML: &str = r#"
run_id: "stage0_smoke"
deals:
  seed: 123
  hands: 16
agents:
  - name: "cautious"
    kind: "heuristic"
    params:
      style: "Cautious"
  - name: "xinxin"
    kind: "external"
    params:
      command: "./tools/xinxin_runner --config openspiel.yaml"
outputs:
  jsonl: "bench/out/{run_id}/deals.jsonl"
  summary_md: "bench/out/{run_id}/summary.md"
  plots_dir: "bench/out/{run_id}/plots"
metrics:
  baseline: "cautious"
logging:
  enable_structured: true
  tracing_level: "debug"
"#;

    #[test]
    fn loads_and_validates_basic_config() {
        let mut cfg: BenchmarkConfig = serde_yaml::from_str(BASIC_YAML).expect("parse yaml");
        cfg.validate().expect("validate");

        assert_eq!(cfg.deals.permutations, DEFAULT_SEAT_PERMUTATIONS);
        assert_eq!(cfg.metrics.latency_budget_ms, DEFAULT_LATENCY_BUDGET_MS);
        assert!(cfg.logging.enable_structured);

        let outputs = cfg.resolved_outputs();
        assert_eq!(
            outputs.jsonl,
            PathBuf::from("bench/out/stage0_smoke/deals.jsonl")
        );
    }

    #[test]
    fn rejects_missing_baseline() {
        let yaml = BASIC_YAML.replace("baseline: \"cautious\"\n", "");
        let mut cfg: BenchmarkConfig = serde_yaml::from_str(&yaml).expect("parse");
        let err = cfg.validate().expect_err("should fail");
        assert!(matches!(
            err,
            ValidationError::InvalidField { field, .. } if field == "metrics.baseline"
        ));
    }

    #[test]
    fn rejects_duplicate_agents() {
        let yaml = BASIC_YAML.replace("- name: \"xinxin\"\n    kind: \"external\"\n    params:\n      command: \"./tools/xinxin_runner --config openspiel.yaml\"\n", "- name: \"cautious\"\n    kind: \"heuristic\"\n");
        let mut cfg: BenchmarkConfig = serde_yaml::from_str(&yaml).expect("parse");
        let err = cfg.validate().expect_err("duplicate agents should fail");
        assert!(matches!(
            err,
            ValidationError::InvalidField { field, .. } if field == "agents"
        ));
    }

    #[test]
    fn rejects_invalid_run_id() {
        let yaml = BASIC_YAML.replace("stage0_smoke", "stage 0 smoke");
        let mut cfg: BenchmarkConfig = serde_yaml::from_str(&yaml).expect("parse");
        let err = cfg.validate().expect_err("invalid run id");
        assert!(matches!(
            err,
            ValidationError::InvalidField { field, .. } if field == "run_id"
        ));
    }

    #[test]
    fn outputs_resolve_template_multiple_occurrences() {
        let yaml = BASIC_YAML.replace(
            "bench/out/{run_id}/plots",
            "bench/out/{run_id}/{run_id}/plots",
        );
        let mut cfg: BenchmarkConfig = serde_yaml::from_str(&yaml).expect("parse");
        cfg.validate().expect("valid");
        let outputs = cfg.resolved_outputs();
        assert_eq!(
            outputs.plots_dir,
            PathBuf::from("bench/out/stage0_smoke/stage0_smoke/plots")
        );
    }
}
