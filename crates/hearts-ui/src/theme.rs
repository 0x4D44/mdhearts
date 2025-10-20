use crate::resource::AssetManifest;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ThemeDescriptor {
    pub name: String,
    pub background_key: String,
    pub card_style_key: String,
}

impl ThemeDescriptor {
    pub fn new(
        name: impl Into<String>,
        background_key: impl Into<String>,
        card_style_key: impl Into<String>,
    ) -> Self {
        Self {
            name: name.into(),
            background_key: background_key.into(),
            card_style_key: card_style_key.into(),
        }
    }
}

impl Default for ThemeDescriptor {
    fn default() -> Self {
        Self::new("Classic Felt", "theme/classic_felt", "cards/default")
    }
}

pub fn built_in_themes() -> Vec<ThemeDescriptor> {
    let manifest = AssetManifest::current();
    if manifest.card_themes.is_empty() || manifest.table_themes.is_empty() {
        return vec![ThemeDescriptor::default()];
    }

    manifest
        .card_themes
        .iter()
        .zip(manifest.table_themes.iter().cycle())
        .map(|(card, table)| ThemeDescriptor::new(&card.display_name, &table.key, &card.key))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::{ThemeDescriptor, built_in_themes};

    #[test]
    fn default_theme_is_classic_felt() {
        let theme = ThemeDescriptor::default();
        assert_eq!(theme.name, "Classic Felt");
    }

    #[test]
    fn built_in_themes_never_empty() {
        assert!(!built_in_themes().is_empty());
    }
}
