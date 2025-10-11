//! Experience collection for offline RL training.
//!
//! This module defines the data format for collecting game experiences
//! that can be used for offline training of neural network policies.

use serde::{Deserialize, Serialize};
use std::fs::{File, OpenOptions};
use std::io::{BufWriter, Write};
use std::path::Path;

/// A single experience tuple for behavioral cloning training
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Experience {
    /// Observation vector (270 features)
    pub observation: Vec<f32>,

    /// Action taken (card ID 0-51)
    pub action: u8,

    /// Reward received (may be 0 for intermediate steps)
    pub reward: f32,

    /// Whether this was a terminal state
    pub done: bool,

    /// Game identifier for batching
    pub game_id: usize,

    /// Step within the game
    pub step_id: usize,

    /// Player seat (0-3)
    pub seat: u8,
}

/// A single experience tuple for PPO/RL training
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RLExperience {
    /// Observation vector (270 features)
    pub observation: Vec<f32>,

    /// Action taken (card ID 0-51)
    pub action: u8,

    /// Reward received (may be 0 for intermediate steps)
    pub reward: f32,

    /// Whether this was a terminal state
    pub done: bool,

    /// Game identifier for batching
    pub game_id: usize,

    /// Step within the game
    pub step_id: usize,

    /// Player seat (0-3)
    pub seat: u8,

    /// Value estimate from critic V(s)
    pub value: f32,

    /// Log probability of action log π(a|s)
    pub log_prob: f32,
}

/// Collector for writing experiences to JSONL format
pub struct ExperienceCollector {
    writer: BufWriter<File>,
    count: usize,
}

impl ExperienceCollector {
    /// Create a new collector writing to a file
    pub fn new<P: AsRef<Path>>(path: P) -> Result<Self, String> {
        let file =
            File::create(path).map_err(|e| format!("Failed to create experience file: {}", e))?;

        Ok(Self {
            writer: BufWriter::new(file),
            count: 0,
        })
    }

    /// Append to an existing file
    #[allow(dead_code)]
    pub fn append<P: AsRef<Path>>(path: P) -> Result<Self, String> {
        let file = OpenOptions::new()
            .create(true)
            .append(true)
            .open(path)
            .map_err(|e| format!("Failed to open experience file: {}", e))?;

        Ok(Self {
            writer: BufWriter::new(file),
            count: 0,
        })
    }

    /// Record a single experience
    pub fn record(&mut self, exp: Experience) -> Result<(), String> {
        let json = serde_json::to_string(&exp)
            .map_err(|e| format!("Failed to serialize experience: {}", e))?;

        writeln!(self.writer, "{}", json)
            .map_err(|e| format!("Failed to write experience: {}", e))?;

        self.count += 1;
        Ok(())
    }

    /// Flush buffered writes
    pub fn flush(&mut self) -> Result<(), String> {
        self.writer
            .flush()
            .map_err(|e| format!("Failed to flush: {}", e))?;
        Ok(())
    }

    /// Get count of recorded experiences
    pub fn count(&self) -> usize {
        self.count
    }
}

impl Drop for ExperienceCollector {
    fn drop(&mut self) {
        let _ = self.flush();
    }
}

/// Collector for writing RL experiences to JSONL format
pub struct RLExperienceCollector {
    writer: BufWriter<File>,
    count: usize,
}

impl RLExperienceCollector {
    /// Create a new collector writing to a file
    pub fn new<P: AsRef<Path>>(path: P) -> Result<Self, String> {
        let file = File::create(path)
            .map_err(|e| format!("Failed to create RL experience file: {}", e))?;

        Ok(Self {
            writer: BufWriter::new(file),
            count: 0,
        })
    }

    /// Record a single RL experience
    pub fn record(&mut self, exp: RLExperience) -> Result<(), String> {
        let json = serde_json::to_string(&exp)
            .map_err(|e| format!("Failed to serialize RL experience: {}", e))?;

        writeln!(self.writer, "{}", json)
            .map_err(|e| format!("Failed to write RL experience: {}", e))?;

        self.count += 1;
        Ok(())
    }

    /// Flush buffered writes
    pub fn flush(&mut self) -> Result<(), String> {
        self.writer
            .flush()
            .map_err(|e| format!("Failed to flush: {}", e))?;
        Ok(())
    }

    /// Get count of recorded experiences
    pub fn count(&self) -> usize {
        self.count
    }
}

impl Drop for RLExperienceCollector {
    fn drop(&mut self) {
        let _ = self.flush();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::{BufRead, BufReader};

    #[test]
    fn experience_serialization_roundtrip() {
        let exp = Experience {
            observation: vec![0.1, 0.2, 0.3],
            action: 5,
            reward: 1.5,
            done: false,
            game_id: 42,
            step_id: 10,
            seat: 2,
        };

        let json = serde_json::to_string(&exp).unwrap();
        let deserialized: Experience = serde_json::from_str(&json).unwrap();

        assert_eq!(deserialized.observation, exp.observation);
        assert_eq!(deserialized.action, exp.action);
        assert_eq!(deserialized.reward, exp.reward);
        assert_eq!(deserialized.done, exp.done);
        assert_eq!(deserialized.game_id, exp.game_id);
        assert_eq!(deserialized.step_id, exp.step_id);
        assert_eq!(deserialized.seat, exp.seat);
    }

    #[test]
    fn collector_writes_jsonl_format() {
        let temp_dir = std::env::temp_dir();
        let path = temp_dir.join("test_experiences.jsonl");

        // Write experiences
        {
            let mut collector = ExperienceCollector::new(&path).unwrap();

            for i in 0..5 {
                let exp = Experience {
                    observation: vec![i as f32],
                    action: i,
                    reward: i as f32 * 0.5,
                    done: i == 4,
                    game_id: 0,
                    step_id: i as usize,
                    seat: 0,
                };
                collector.record(exp).unwrap();
            }

            assert_eq!(collector.count(), 5);
        }

        // Read back and verify
        let file = File::open(&path).unwrap();
        let reader = BufReader::new(file);
        let lines: Vec<String> = reader.lines().map(|l| l.unwrap()).collect();

        assert_eq!(lines.len(), 5);

        for (i, line) in lines.iter().enumerate() {
            let exp: Experience = serde_json::from_str(line).unwrap();
            assert_eq!(exp.step_id, i);
            assert_eq!(exp.action, i as u8);
        }

        // Clean up
        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn collector_can_append() {
        let temp_dir = std::env::temp_dir();
        let path = temp_dir.join("test_append.jsonl");

        // Write initial experiences
        {
            let mut collector = ExperienceCollector::new(&path).unwrap();
            let exp = Experience {
                observation: vec![1.0],
                action: 0,
                reward: 0.0,
                done: false,
                game_id: 0,
                step_id: 0,
                seat: 0,
            };
            collector.record(exp).unwrap();
        }

        // Append more
        {
            let mut collector = ExperienceCollector::append(&path).unwrap();
            let exp = Experience {
                observation: vec![2.0],
                action: 1,
                reward: 0.0,
                done: false,
                game_id: 0,
                step_id: 1,
                seat: 0,
            };
            collector.record(exp).unwrap();
        }

        // Verify both are present
        let file = File::open(&path).unwrap();
        let reader = BufReader::new(file);
        let lines: Vec<String> = reader.lines().map(|l| l.unwrap()).collect();

        assert_eq!(lines.len(), 2);

        // Clean up
        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn rl_experience_serialization_roundtrip() {
        let exp = RLExperience {
            observation: vec![0.1, 0.2, 0.3],
            action: 5,
            reward: 1.5,
            done: false,
            game_id: 42,
            step_id: 10,
            seat: 2,
            value: 0.75,
            log_prob: -1.23,
        };

        let json = serde_json::to_string(&exp).unwrap();
        let deserialized: RLExperience = serde_json::from_str(&json).unwrap();

        assert_eq!(deserialized.observation, exp.observation);
        assert_eq!(deserialized.action, exp.action);
        assert_eq!(deserialized.reward, exp.reward);
        assert_eq!(deserialized.done, exp.done);
        assert_eq!(deserialized.game_id, exp.game_id);
        assert_eq!(deserialized.step_id, exp.step_id);
        assert_eq!(deserialized.seat, exp.seat);
        assert_eq!(deserialized.value, exp.value);
        assert_eq!(deserialized.log_prob, exp.log_prob);
    }

    #[test]
    fn rl_collector_writes_jsonl_format() {
        let temp_dir = std::env::temp_dir();
        let path = temp_dir.join("test_rl_experiences.jsonl");

        // Write experiences
        {
            let mut collector = RLExperienceCollector::new(&path).unwrap();

            for i in 0..5 {
                let exp = RLExperience {
                    observation: vec![i as f32],
                    action: i,
                    reward: i as f32 * 0.5,
                    done: i == 4,
                    game_id: 0,
                    step_id: i as usize,
                    seat: 0,
                    value: i as f32 * 0.1,
                    log_prob: -(i as f32),
                };
                collector.record(exp).unwrap();
            }

            assert_eq!(collector.count(), 5);
        }

        // Read back and verify
        let file = File::open(&path).unwrap();
        let reader = BufReader::new(file);
        let lines: Vec<String> = reader.lines().map(|l| l.unwrap()).collect();

        assert_eq!(lines.len(), 5);

        for (i, line) in lines.iter().enumerate() {
            let exp: RLExperience = serde_json::from_str(line).unwrap();
            assert_eq!(exp.step_id, i);
            assert_eq!(exp.action, i as u8);
            assert_eq!(exp.value, i as f32 * 0.1);
            assert!((exp.log_prob - (-(i as f32))).abs() < 1e-6);
        }

        // Clean up
        let _ = std::fs::remove_file(&path);
    }
}
