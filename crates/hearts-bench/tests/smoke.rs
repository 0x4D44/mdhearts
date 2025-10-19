use std::fs;

use hearts_bench::config::BenchmarkConfig;
use hearts_bench::tournament::TournamentRunner;
use sha2::{Digest, Sha256};
use tempfile::tempdir;

fn load_config(output_dir: &std::path::Path) -> BenchmarkConfig {
    let yaml = format!(
        r#"
run_id: "test_smoke"
deals:
  seed: 4242
  hands: 2
  permutations: 1
agents:
  - name: "baseline"
    kind: "heuristic"
    params:
      difficulty: "normal"
  - name: "easy"
    kind: "heuristic"
    params:
      difficulty: "easy"
  - name: "hard"
    kind: "heuristic"
    params:
      difficulty: "hard"
  - name: "normal_2"
    kind: "heuristic"
    params:
      difficulty: "normal"
outputs:
  jsonl: "{jsonl}"
  summary_md: "{summary}"
  plots_dir: "{plots}"
metrics:
  baseline: "baseline"
logging:
  enable_structured: false
"#,
        jsonl = output_dir.join("deals.jsonl").display(),
        summary = output_dir.join("summary.md").display(),
        plots = output_dir.join("plots").display()
    );

    let mut cfg: BenchmarkConfig = serde_yaml::from_str(&yaml).expect("valid yaml");
    cfg.validate().expect("config validates");
    cfg
}

#[test]
fn tournament_smoke_test_produces_stable_jsonl_hash() {
    let dir = tempdir().expect("temp dir");
    let config = load_config(dir.path());
    let outputs = config.resolved_outputs();

    let runner = TournamentRunner::new(config, outputs).expect("runner created");
    let summary = runner.run().expect("tournament completes");

    assert_eq!(summary.hands_played, 2);
    assert_eq!(summary.permutations, 1);

    let jsonl = fs::read_to_string(&summary.jsonl_path).expect("jsonl readable");
    let mut normalized = String::new();
    for line in jsonl.lines() {
        let mut value: serde_json::Value = serde_json::from_str(line).expect("row decodes to JSON");
        if let Some(obj) = value.as_object_mut() {
            if let Some(speed) = obj.get_mut("speed_ms_turn") {
                *speed = serde_json::Value::Number(
                    serde_json::Number::from_f64(0.0).expect("number for normalized speed"),
                );
            }
        }
        normalized.push_str(&serde_json::to_string(&value).expect("re-serialize normalized row"));
        normalized.push('\n');
    }

    let mut hasher = Sha256::new();
    hasher.update(normalized.as_bytes());
    let digest = hasher.finalize();

    let actual = hex::encode(digest);
    assert_eq!(
        actual, "c12312ffece58c3a93557945fb368194d818a80038254679815b6d25ba56f373",
        "JSONL output hash changed; update expected value if intentional"
    );

    assert!(summary.summary_path.exists(), "summary markdown missing");
    // Plot rendering is optional; ensure any failure surfaces explicitly
    if let Some(plot_path) = summary.plot_path {
        assert!(plot_path.exists(), "plot path reported but missing on disk");
    }
}
