use super::match_state::MatchState;
use crate::model::card::Card;
use crate::model::hand::Hand;
use crate::model::passing::{PassingDirection, PassingState};
use crate::model::player::PlayerPosition;
use crate::model::rank::Rank;
use crate::model::round::{RoundPhase, RoundState};
use crate::model::suit::Suit;
use crate::model::trick::Trick;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct MatchSnapshot {
    pub seed: u64,
    pub round_number: u32,
    pub passing_direction: String,
    pub scores: [u32; 4],
    pub round_starting_player: PlayerPosition,
    #[serde(default)]
    pub round: Option<RoundSnapshot>,
    #[serde(default)]
    pub passing_index: Option<usize>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct RoundSnapshot {
    pub hands: [Vec<String>; 4],
    pub current_trick: TrickSnapshot,
    pub trick_history: Vec<TrickSnapshot>,
    pub phase: RoundPhaseSnapshot,
    pub hearts_broken: bool,
    pub starting_player: PlayerPosition,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum RoundPhaseSnapshot {
    Passing {
        submissions: [Option<[String; 3]>; 4],
    },
    Playing,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct TrickSnapshot {
    pub leader: PlayerPosition,
    pub plays: Vec<PlaySnapshot>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct PlaySnapshot {
    pub position: PlayerPosition,
    pub card: String,
}

impl MatchSnapshot {
    pub fn capture(state: &MatchState) -> Self {
        MatchSnapshot {
            seed: state.seed(),
            round_number: state.round_number(),
            passing_direction: state.passing_direction().as_str().to_string(),
            scores: *state.scores().standings(),
            round_starting_player: state.round().starting_player(),
            round: None,
            passing_index: None,
        }
    }

    pub fn capture_full(state: &MatchState) -> Self {
        MatchSnapshot {
            seed: state.seed(),
            round_number: state.round_number(),
            passing_direction: state.passing_direction().as_str().to_string(),
            scores: *state.scores().standings(),
            round_starting_player: state.round().starting_player(),
            round: Some(RoundSnapshot::capture(state.round())),
            passing_index: Some(state.passing_index()),
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

    pub fn restore_full(self) -> MatchState {
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

        if let Some(round_snapshot) = self.round.clone() {
            let restored_round = round_snapshot
                .restore(direction)
                .expect("snapshot must be valid");
            state.set_round(restored_round);
        }

        if let Some(idx) = self.passing_index {
            state.set_passing_index(idx);
        } else {
            if let Some(idx) = PassingDirection::CYCLE
                .iter()
                .position(|d| d.as_str() == direction.as_str())
            {
                state.set_passing_index(idx);
            }
        }

        state
    }

    pub fn to_json(state: &MatchState) -> serde_json::Result<String> {
        let snapshot = Self::capture(state);
        serde_json::to_string_pretty(&snapshot)
    }

    pub fn to_json_full(state: &MatchState) -> serde_json::Result<String> {
        let snapshot = Self::capture_full(state);
        serde_json::to_string_pretty(&snapshot)
    }

    pub fn from_json(json: &str) -> serde_json::Result<Self> {
        serde_json::from_str(json)
    }
}

impl RoundSnapshot {
    pub fn capture(round: &RoundState) -> Self {
        let mut hands: [Vec<String>; 4] = std::array::from_fn(|_| Vec::new());
        for seat in PlayerPosition::LOOP.iter().copied() {
            let mut cards: Vec<Card> = round.hand(seat).cards().to_vec();
            sort_cards(&mut cards);
            hands[seat.index()] = cards.iter().copied().map(card_to_string).collect();
        }

        let current_trick = TrickSnapshot::capture(round.current_trick());
        let trick_history = round
            .trick_history()
            .iter()
            .map(TrickSnapshot::capture)
            .collect();

        let phase = match round.phase() {
            RoundPhase::Playing => RoundPhaseSnapshot::Playing,
            RoundPhase::Passing(state) => {
                let mut submissions: [Option<[String; 3]>; 4] =
                    std::array::from_fn(|_| None);
                for seat in PlayerPosition::LOOP.iter().copied() {
                    if let Some(cards) = state.submissions()[seat.index()] {
                        submissions[seat.index()] = Some(cards.map(card_to_string));
                    }
                }
                RoundPhaseSnapshot::Passing { submissions }
            }
        };

        RoundSnapshot {
            hands,
            current_trick,
            trick_history,
            phase,
            hearts_broken: round.hearts_broken(),
            starting_player: round.starting_player(),
        }
    }

    pub fn restore(
        self,
        passing_direction: PassingDirection,
    ) -> Result<RoundState, String> {
        let hands_cards: [Vec<Card>; 4] = std::array::from_fn(|idx| {
            self.hands[idx]
                .iter()
                .map(|s| parse_card(s))
                .collect::<Option<Vec<_>>>()
                .unwrap_or_default()
        });

        let hands: [Hand; 4] = std::array::from_fn(|idx| {
            let mut cards = hands_cards[idx].clone();
            sort_cards(&mut cards);
            Hand::with_cards(cards)
        });

        let current_trick = self
            .current_trick
            .restore()
            .map_err(|e| format!("current trick restore failed: {e}"))?;
        let trick_history = self
            .trick_history
            .into_iter()
            .map(|t| t.restore())
            .collect::<Result<Vec<_>, _>>()
            .map_err(|e| format!("trick history restore failed: {e}"))?;

        let phase = match self.phase {
            RoundPhaseSnapshot::Playing => RoundPhase::Playing,
            RoundPhaseSnapshot::Passing { submissions } => {
                let state = PassingState::with_submissions(
                    passing_direction,
                    submissions.map(|opt| opt.map(|arr| arr.map(|s| parse_card(&s).unwrap()))),
                );
                // ensure submissions with None stay None; already set via map
                RoundPhase::Passing(state.clone())
            }
        };

        let round = RoundState::from_hands_with_state(
            hands,
            self.starting_player,
            passing_direction,
            phase,
            current_trick.clone(),
            trick_history,
            self.hearts_broken,
        );

        Ok(round)
    }
}

impl TrickSnapshot {
    pub fn capture(trick: &Trick) -> Self {
        TrickSnapshot {
            leader: trick.leader(),
            plays: trick
                .plays()
                .iter()
                .map(|p| PlaySnapshot {
                    position: p.position,
                    card: card_to_string(p.card),
                })
                .collect(),
        }
    }

    pub fn restore(self) -> Result<Trick, String> {
        let mut trick = Trick::new(self.leader);
        for play in self.plays {
            let card = parse_card(&play.card).ok_or_else(|| "invalid card".to_string())?;
            trick
                .play(play.position, card)
                .map_err(|e| format!("trick play invalid: {e}"))?;
        }
        Ok(trick)
    }
}

fn card_to_string(card: Card) -> String {
    card.to_string()
}

fn parse_card(code: &str) -> Option<Card> {
    if code.len() < 2 {
        return None;
    }
    let (rank_str, suit_char) = code.split_at(code.len() - 1);
    let suit = match suit_char.chars().next()? {
        'C' | 'c' => Suit::Clubs,
        'D' | 'd' => Suit::Diamonds,
        'S' | 's' => Suit::Spades,
        'H' | 'h' => Suit::Hearts,
        _ => return None,
    };
    let rank = match rank_str {
        "A" | "a" => Rank::Ace,
        "K" | "k" => Rank::King,
        "Q" | "q" => Rank::Queen,
        "J" | "j" => Rank::Jack,
        "10" => Rank::Ten,
        "9" | "09" => Rank::Nine,
        "8" | "08" => Rank::Eight,
        "7" | "07" => Rank::Seven,
        "6" | "06" => Rank::Six,
        "5" | "05" => Rank::Five,
        "4" | "04" => Rank::Four,
        "3" | "03" => Rank::Three,
        "2" | "02" => Rank::Two,
        _ => return None,
    };
    Some(Card::new(rank, suit))
}

fn sort_cards(cards: &mut Vec<Card>) {
    cards.sort_by(|a, b| a.suit.cmp(&b.suit).then(a.rank.cmp(&b.rank)));
}

#[cfg(test)]
mod tests {
    use super::MatchSnapshot;
    use crate::game::match_state::MatchState;
    use crate::model::card::Card;
    use crate::model::hand::Hand;
    use crate::model::passing::{PassingDirection, PassingState};
    use crate::model::player::PlayerPosition;
    use crate::model::rank::Rank;
    use crate::model::round::{RoundPhase, RoundState};
    use crate::model::suit::Suit;
    use crate::model::trick::Trick;

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

    #[test]
    fn full_snapshot_roundtrip_preserves_state() {
        // Build a small custom round: passing phase with one submission.
        let hands = [
            Hand::with_cards(vec![Card::new(Rank::Two, Suit::Clubs)]),
            Hand::with_cards(vec![Card::new(Rank::Queen, Suit::Spades)]),
            Hand::with_cards(vec![Card::new(Rank::Three, Suit::Clubs)]),
            Hand::with_cards(vec![Card::new(Rank::Four, Suit::Clubs)]),
        ];
        let mut round = RoundState::from_hands(
            hands,
            PlayerPosition::North,
            PassingDirection::Left,
            RoundPhase::Passing(PassingState::new(PassingDirection::Left)),
        );
        // submit one pass to ensure submissions captured
        round
            .submit_pass(
                PlayerPosition::North,
                [
                    Card::new(Rank::Two, Suit::Clubs),
                    Card::new(Rank::Three, Suit::Clubs),
                    Card::new(Rank::Four, Suit::Clubs),
                ],
            )
            .ok();

        let mut state =
            MatchState::with_seed_round_direction(7, 2, PassingDirection::Left, PlayerPosition::North);
        state.scores_mut().set_totals([1, 2, 3, 4]);
        state.set_round(round.clone());

        let snap = MatchSnapshot::capture_full(&state);
        let json = serde_json::to_string(&snap).unwrap();
        let parsed: MatchSnapshot = serde_json::from_str(&json).unwrap();
        let restored = parsed.restore_full();

        assert_eq!(restored.seed(), 7);
        assert_eq!(restored.round_number(), 2);
        assert_eq!(restored.scores().standings(), &[1, 2, 3, 4]);

        let restored_round = restored.round();
        assert!(matches!(restored_round.phase(), RoundPhase::Passing(_)));
        assert!(!restored_round.trick_history().is_empty() || restored_round.current_trick().plays().len() == round.current_trick().plays().len());
        assert_eq!(restored_round.starting_player(), PlayerPosition::North);
    }

    #[test]
    fn trick_snapshot_roundtrip() {
        let mut trick = Trick::new(PlayerPosition::North);
        trick
            .play(PlayerPosition::North, Card::new(Rank::Ten, Suit::Clubs))
            .unwrap();
        trick
            .play(PlayerPosition::East, Card::new(Rank::Queen, Suit::Clubs))
            .unwrap();
        let snap = super::TrickSnapshot::capture(&trick);
        let restored = snap.restore().unwrap();
        assert_eq!(restored.plays().len(), 2);
        assert_eq!(restored.leader(), PlayerPosition::North);
    }
}
