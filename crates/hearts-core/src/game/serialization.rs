use super::match_state::MatchState;
use crate::model::passing::PassingDirection;
use crate::model::player::PlayerPosition;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct MatchSnapshot {
    pub seed: u64,
    pub round_number: u32,
    pub passing_direction: String,
    pub scores: [u32; 4],
    pub round_starting_player: PlayerPosition,
}

impl MatchSnapshot {
    pub fn capture(state: &MatchState) -> Self {
        MatchSnapshot {
            seed: state.seed(),
            round_number: state.round_number(),
            passing_direction: state.passing_direction().as_str().to_string(),
            scores: *state.scores().standings(),
            round_starting_player: state.round().starting_player(),
        }
    }

    pub fn restore(self) -> MatchState {
        let direction = self
            .passing_direction
            .parse::<PassingDirection>()
            .unwrap_or(PassingDirection::Left);
        let mut state = MatchState::with_seed_round_direction(
            self.seed,
            self.round_number,
            direction,
            self.round_starting_player,
        );
        state.scores_mut().set_totals(self.scores);
        state
    }

    pub fn to_json(state: &MatchState) -> serde_json::Result<String> {
        let snapshot = Self::capture(state);
        serde_json::to_string_pretty(&snapshot)
    }

    pub fn from_json(json: &str) -> serde_json::Result<Self> {
        serde_json::from_str(json)
    }
}

#[cfg(test)]
mod tests {
    use super::MatchSnapshot;
    use crate::game::match_state::MatchState;
    use crate::model::player::PlayerPosition;

    #[test]
    fn snapshot_serializes_to_json() {
        let state = MatchState::with_seed(PlayerPosition::North, 99);
        let json = MatchSnapshot::to_json(&state).unwrap();
        assert!(json.contains("\"seed\": 99"));
        assert!(json.contains("\"round_number\": 1"));
        assert!(!json.contains("\"penalties\""));
    }

    #[test]
    fn snapshot_roundtrip_restores_seed_and_scores() {
        let mut state = MatchState::with_seed(PlayerPosition::North, 123);
        state.scores_mut().set_totals([10, 20, 30, 40]);
        let snapshot = MatchSnapshot::capture(&state);
        let restored = snapshot.clone().restore();
        assert_eq!(restored.seed(), 123);
        assert_eq!(restored.scores().standings(), &snapshot.scores);
        assert_eq!(
            restored.passing_direction().as_str(),
            snapshot.passing_direction
        );
    }

    #[test]
    fn snapshot_from_json_ignores_legacy_penalties_field() {
        let legacy = r#"{
            "seed": 7,
            "round_number": 2,
            "passing_direction": "Left",
            "scores": [0, 1, 2, 3],
            "penalties": [0, 0, 0, 0],
            "round_starting_player": "North"
        }"#;

        let snapshot = MatchSnapshot::from_json(legacy).unwrap();
        assert_eq!(snapshot.round_number, 2);
        assert_eq!(snapshot.scores, [0, 1, 2, 3]);
        assert_eq!(snapshot.passing_direction, "Left");
    }
}
