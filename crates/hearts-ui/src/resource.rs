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

static MANIFEST: Lazy<AssetManifest> = Lazy::new(load_manifest);

fn load_manifest() -> AssetManifest {
    if let Ok(path) = std::env::var("HEARTS_ASSET_MANIFEST") {
        if let Ok(content) = std::fs::read_to_string(&path) {
            if let Ok(manifest) = serde_json::from_str::<AssetManifest>(&content) {
                return manifest;
            } else {
                eprintln!("Failed to parse asset manifest {path}; falling back to placeholder");
            }
        } else {
            eprintln!("Failed to read asset manifest {path}; falling back to placeholder");
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
}
