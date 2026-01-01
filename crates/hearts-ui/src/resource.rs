use once_cell::sync::Lazy;
use serde::Deserialize;

#[derive(Debug, Clone, Deserialize, PartialEq, Eq)]
pub struct CardTheme {
    pub key: String,
    pub display_name: String,
    pub asset_prefix: String,
}

#[derive(Debug, Clone, Deserialize, PartialEq, Eq)]
pub struct TableTheme {
    pub key: String,
    pub display_name: String,
    pub texture_path: String,
}

#[derive(Debug, Clone, Deserialize, PartialEq, Eq, Default)]
pub struct AssetManifest {
    pub card_themes: Vec<CardTheme>,
    pub table_themes: Vec<TableTheme>,
}

static MANIFEST: Lazy<AssetManifest> = Lazy::new(load_manifest_from_env);

fn load_manifest_from_env() -> AssetManifest {
    if let Ok(path) = std::env::var("HEARTS_ASSET_MANIFEST") {
        match AssetManifest::load_from_path(&path) {
            Ok(m) => return m,
            Err(e) => {
                eprintln!("Failed to load asset manifest {path}: {e}; falling back to placeholder")
            }
        }
    }
    AssetManifest::placeholder()
}

impl AssetManifest {
    pub fn placeholder() -> Self {
        Self {
            card_themes: vec![CardTheme {
                key: "cards/default".into(),
                display_name: "CLASSIC DECK".into(),
                asset_prefix: "cards/default".into(),
            }],
            table_themes: vec![TableTheme {
                key: "theme/classic_felt".into(),
                display_name: "CLASSIC FELT".into(),
                texture_path: "themes/classic_felt.png".into(),
            }],
        }
    }

    pub fn current() -> &'static AssetManifest {
        &MANIFEST
    }

    pub fn load_from_path(path: &str) -> Result<Self, String> {
        let content = std::fs::read_to_string(path).map_err(|e| format!("read error: {e}"))?;
        serde_json::from_str(&content).map_err(|e| format!("parse error: {e}"))
    }
}

#[cfg(test)]
mod tests {
    use super::AssetManifest;

    #[test]
    fn placeholder_manifest_contains_data() {
        let manifest = AssetManifest::placeholder();
        assert!(!manifest.card_themes.is_empty());
        assert!(!manifest.table_themes.is_empty());
    }

    #[test]
    fn load_from_path_handles_errors() {
        assert!(AssetManifest::load_from_path("non_existent_file.json").is_err());

        let mut temp = std::env::temp_dir();
        temp.push("bad_manifest.json");
        std::fs::write(&temp, "invalid json").unwrap();
        assert!(AssetManifest::load_from_path(temp.to_str().unwrap()).is_err());
        let _ = std::fs::remove_file(temp);
    }

    #[test]
    fn load_from_path_success() {
        let mut temp = std::env::temp_dir();
        temp.push("good_manifest.json");
        let json = r#"{
            "card_themes": [],
            "table_themes": []
        }"#;
        std::fs::write(&temp, json).unwrap();
        let m = AssetManifest::load_from_path(temp.to_str().unwrap()).unwrap();
        assert!(m.card_themes.is_empty());
        let _ = std::fs::remove_file(temp);
    }
}
