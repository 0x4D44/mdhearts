//! Weight loading from JSON files for custom trained models.
//!
//! This module allows loading neural network weights from JSON files,
//! enabling the use of externally trained models instead of the
//! compiled-in dummy weights.

use serde::{Deserialize, Serialize};
use std::fs;
use std::path::Path;

/// JSON format for model weights
#[derive(Debug, Serialize, Deserialize)]
pub struct WeightManifest {
    pub schema_version: String,
    pub schema_hash: String,
    pub layer1: LayerWeights,
    pub layer2: LayerWeights,
    pub layer3: LayerWeights,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct LayerWeights {
    pub weights: Vec<f32>,
    pub biases: Vec<f32>,
}

impl WeightManifest {
    /// Load weights from a JSON file
    pub fn from_file<P: AsRef<Path>>(path: P) -> Result<Self, String> {
        let contents =
            fs::read_to_string(path).map_err(|e| format!("Failed to read weight file: {}", e))?;

        let manifest: WeightManifest = serde_json::from_str(&contents)
            .map_err(|e| format!("Failed to parse weight JSON: {}", e))?;

        Ok(manifest)
    }

    /// Save weights to a JSON file
    #[allow(dead_code)]
    pub fn to_file<P: AsRef<Path>>(&self, path: P) -> Result<(), String> {
        let json = serde_json::to_string_pretty(self)
            .map_err(|e| format!("Failed to serialize weights: {}", e))?;

        fs::write(path, json).map_err(|e| format!("Failed to write weight file: {}", e))?;

        Ok(())
    }

    /// Validate weight dimensions match expected architecture
    pub fn validate(&self) -> Result<(), String> {
        // Layer 1: 270 -> 256
        if self.layer1.weights.len() != 270 * 256 {
            return Err(format!(
                "Layer 1 weights wrong size: expected {}, got {}",
                270 * 256,
                self.layer1.weights.len()
            ));
        }
        if self.layer1.biases.len() != 256 {
            return Err(format!(
                "Layer 1 biases wrong size: expected 256, got {}",
                self.layer1.biases.len()
            ));
        }

        // Layer 2: 256 -> 128
        if self.layer2.weights.len() != 256 * 128 {
            return Err(format!(
                "Layer 2 weights wrong size: expected {}, got {}",
                256 * 128,
                self.layer2.weights.len()
            ));
        }
        if self.layer2.biases.len() != 128 {
            return Err(format!(
                "Layer 2 biases wrong size: expected 128, got {}",
                self.layer2.biases.len()
            ));
        }

        // Layer 3: 128 -> 52
        if self.layer3.weights.len() != 128 * 52 {
            return Err(format!(
                "Layer 3 weights wrong size: expected {}, got {}",
                128 * 52,
                self.layer3.weights.len()
            ));
        }
        if self.layer3.biases.len() != 52 {
            return Err(format!(
                "Layer 3 biases wrong size: expected 52, got {}",
                self.layer3.biases.len()
            ));
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn weight_manifest_validation_passes_for_correct_sizes() {
        let manifest = WeightManifest {
            schema_version: "1.1.0".to_string(),
            schema_hash: "test".to_string(),
            layer1: LayerWeights {
                weights: vec![0.0; 270 * 256],
                biases: vec![0.0; 256],
            },
            layer2: LayerWeights {
                weights: vec![0.0; 256 * 128],
                biases: vec![0.0; 128],
            },
            layer3: LayerWeights {
                weights: vec![0.0; 128 * 52],
                biases: vec![0.0; 52],
            },
        };

        assert!(manifest.validate().is_ok());
    }

    #[test]
    fn weight_manifest_validation_fails_for_wrong_sizes() {
        let manifest = WeightManifest {
            schema_version: "1.1.0".to_string(),
            schema_hash: "test".to_string(),
            layer1: LayerWeights {
                weights: vec![0.0; 100], // Wrong size
                biases: vec![0.0; 256],
            },
            layer2: LayerWeights {
                weights: vec![0.0; 256 * 128],
                biases: vec![0.0; 128],
            },
            layer3: LayerWeights {
                weights: vec![0.0; 128 * 52],
                biases: vec![0.0; 52],
            },
        };

        assert!(manifest.validate().is_err());
    }
}
