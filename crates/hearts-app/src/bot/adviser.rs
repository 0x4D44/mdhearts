use super::BotContext;
use hearts_core::model::card::Card;
use serde::Deserialize;
use std::collections::HashMap;
use std::fs;
use std::sync::OnceLock;

#[derive(Default)]
struct PhaseAdviser {
    card_bias: HashMap<String, i32>,
}

impl PhaseAdviser {
    fn bias_for(&self, card: Card) -> i32 {
        let key = card.to_string();
        *self.card_bias.get(&key).unwrap_or(&0)
    }
}

#[derive(Deserialize)]
struct AdviserFile {
    version: u32,
    #[serde(default)]
    card_bias: HashMap<String, i32>,
}

static PLAY_ADVISER: OnceLock<PhaseAdviser> = OnceLock::new();

fn load_play_adviser() -> PhaseAdviser {
    let path = std::env::var("MDH_ADVISER_PLAY_PATH")
        .unwrap_or_else(|_| "assets/adviser/play.json".to_string());
    match fs::read_to_string(&path) {
        Ok(raw) => {
            if let Ok(parsed) = serde_json::from_str::<AdviserFile>(&raw) {
                if parsed.version == 1 {
                    return PhaseAdviser {
                        card_bias: parsed.card_bias,
                    };
                }
            }
            PhaseAdviser::default()
        }
        Err(_) => PhaseAdviser::default(),
    }
}

fn play_adviser() -> &'static PhaseAdviser {
    PLAY_ADVISER.get_or_init(load_play_adviser)
}

fn play_enabled() -> bool {
    matches!(
        std::env::var("MDH_HARD_ADVISER_PLAY"),
        Ok(value) if matches!(value.trim().to_ascii_lowercase().as_str(), "1" | "true" | "on")
    )
}

pub fn play_bias(card: Card, ctx: &BotContext<'_>) -> i32 {
    if !play_enabled() {
        return 0;
    }
    let _ = ctx; // placeholder for future feature usage
    play_adviser().bias_for(card)
}
