use crate::bot::{MoonState, UnseenTracker};
use crate::controller::GameController;
use hearts_core::model::card::Card;
use hearts_core::model::hand::Hand;
use hearts_core::model::passing::PassingDirection;
use hearts_core::model::player::PlayerPosition;
use hearts_core::model::rank::Rank;
use hearts_core::model::round::{RoundPhase, RoundState};
use hearts_core::model::score::ScoreBoard;
use hearts_core::model::suit::Suit;
use hearts_core::model::trick::{Play, Trick};
use serde::{Deserialize, Serialize};
use std::array;
use std::collections::BTreeMap;
use std::fmt;

const fn legacy_version() -> u32 {
    1
}

#[derive(Debug, Serialize, Deserialize)]
pub struct EndgameExport {
    #[serde(default = "legacy_version")]
    pub version: u32,
    #[serde(default)]
    pub seed: Option<u64>,
    pub seat: String,
    #[serde(default)]
    pub round_number: Option<u32>,
    #[serde(default)]
    pub starting_player: Option<String>,
    #[serde(default)]
    pub passing_direction: Option<String>,
    #[serde(default)]
    pub phase: Option<String>,
    pub hearts_broken: bool,
    pub leader: String,
    #[serde(default)]
    pub hands: BTreeMap<String, Vec<String>>,
    #[serde(default)]
    pub current_trick: Option<TrickExport>,
    #[serde(default)]
    pub completed_tricks: Vec<TrickExport>,
    #[serde(default)]
    pub scores: Option<[u32; 4]>,
    #[serde(default)]
    pub penalties: Option<[u8; 4]>,
    #[serde(default)]
    pub next_to_play: Option<String>,
    #[serde(default)]
    pub moon_states: Option<BTreeMap<String, String>>,
    #[serde(default)]
    pub voids: Option<BTreeMap<String, [bool; 4]>>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct TrickExport {
    pub leader: String,
    #[serde(default)]
    pub plays: Vec<PlayExport>,
    #[serde(default)]
    pub penalties: Option<u8>,
    #[serde(default)]
    pub is_complete: Option<bool>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct PlayExport {
    pub seat: String,
    pub card: String,
}

#[cfg_attr(not(test), allow(dead_code))]
#[derive(Debug)]
pub struct EndgameRehydrate {
    pub round: RoundState,
    pub scores: ScoreBoard,
    pub passing_direction: PassingDirection,
    pub tracker: UnseenTracker,
    pub next_to_play: PlayerPosition,
}

#[cfg_attr(not(test), allow(dead_code))]
#[derive(Debug)]
pub enum EndgameLoadError {
    UnknownSeat(String),
    UnknownCard(String),
    UnknownSuit(String),
    UnknownRank(String),
    UnknownPassingDirection(String),
    UnknownMoonState(String),
    MissingField(&'static str),
    TrickOutOfOrder,
}

impl fmt::Display for EndgameLoadError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            EndgameLoadError::UnknownSeat(s) => write!(f, "unknown seat '{s}'"),
            EndgameLoadError::UnknownCard(c) => write!(f, "unknown card '{c}'"),
            EndgameLoadError::UnknownSuit(s) => write!(f, "unknown suit '{s}'"),
            EndgameLoadError::UnknownRank(r) => write!(f, "unknown rank '{r}'"),
            EndgameLoadError::UnknownPassingDirection(p) => {
                write!(f, "unknown passing direction '{p}'")
            }
            EndgameLoadError::UnknownMoonState(m) => write!(f, "unknown moon state '{m}'"),
            EndgameLoadError::MissingField(name) => write!(f, "missing field {name}"),
            EndgameLoadError::TrickOutOfOrder => write!(f, "invalid trick order in export"),
        }
    }
}

impl std::error::Error for EndgameLoadError {}

impl EndgameExport {
    pub fn capture(controller: &GameController, focus: PlayerPosition, seed: Option<u64>) -> Self {
        let ctx = controller.bot_context(focus);
        let round = ctx.round;
        let tracker = ctx.tracker;
        let passing = controller.passing_direction();
        let next_to_play = controller.expected_to_play();
        let mut hands = BTreeMap::new();
        for seat in PlayerPosition::LOOP.iter().copied() {
            let cards: Vec<String> = round
                .hand(seat)
                .iter()
                .map(|card| card.to_string())
                .collect();
            hands.insert(seat_key(seat).to_string(), cards);
        }

        let current_trick = trick_to_export(round.current_trick());
        let completed_tricks = round.trick_history().iter().map(trick_to_export).collect();

        let mut moon_states = BTreeMap::new();
        let mut voids = BTreeMap::new();
        for seat in PlayerPosition::LOOP.iter().copied() {
            moon_states.insert(
                seat_key(seat).to_string(),
                format!("{:?}", tracker.moon_state(seat)),
            );
            let mut suit_voids = [false; 4];
            for (idx, suit) in Suit::ALL.iter().enumerate() {
                suit_voids[idx] = tracker.is_void(seat, *suit);
            }
            voids.insert(seat_key(seat).to_string(), suit_voids);
        }

        Self {
            version: 2,
            seed,
            seat: format!("{:?}", focus),
            round_number: Some(controller.round_number()),
            starting_player: Some(format!("{:?}", round.starting_player())),
            passing_direction: Some(passing.as_str().to_string()),
            phase: Some(match round.phase() {
                RoundPhase::Playing => "Playing".to_string(),
                RoundPhase::Passing(_) => "Passing".to_string(),
            }),
            hearts_broken: round.hearts_broken(),
            leader: format!("{:?}", round.current_trick().leader()),
            hands,
            current_trick: Some(current_trick),
            completed_tricks,
            scores: Some(controller.standings()),
            penalties: Some(controller.penalties_this_round()),
            next_to_play: Some(format!("{:?}", next_to_play)),
            moon_states: Some(moon_states),
            voids: Some(voids),
        }
    }

    #[cfg_attr(not(test), allow(dead_code))]
    pub fn rehydrate(&self) -> Result<EndgameRehydrate, EndgameLoadError> {
        let passing_direction = if let Some(direction) = &self.passing_direction {
            direction
                .parse::<PassingDirection>()
                .map_err(|_| EndgameLoadError::UnknownPassingDirection(direction.clone()))?
        } else {
            PassingDirection::Hold
        };

        let starting_player = if let Some(starting) = &self.starting_player {
            parse_seat(starting)?
        } else {
            parse_seat(&self.leader)?
        };

        if self.hands.is_empty() {
            return Err(EndgameLoadError::MissingField("hands"));
        }

        let mut parsed_cards: [Vec<Card>; 4] = array::from_fn(|_| Vec::new());
        for seat in PlayerPosition::LOOP.iter().copied() {
            let key = seat_key(seat);
            let texts = self
                .hands
                .get(key)
                .ok_or(EndgameLoadError::MissingField("hands"))?;
            let cards = texts
                .iter()
                .map(|text| parse_card(text))
                .collect::<Result<Vec<_>, _>>()?;
            parsed_cards[seat.index()] = cards;
        }
        let hands_array: [Hand; 4] = parsed_cards.map(Hand::with_cards);

        let current_trick = if let Some(trick) = &self.current_trick {
            trick_from_export(trick)?
        } else {
            Trick::new(parse_seat(&self.leader)?)
        };

        let completed_tricks: Vec<Trick> = self
            .completed_tricks
            .iter()
            .map(trick_from_export)
            .collect::<Result<_, _>>()?;

        let round = RoundState::from_hands_with_state(
            hands_array,
            starting_player,
            passing_direction,
            RoundPhase::Playing,
            current_trick,
            completed_tricks,
            self.hearts_broken,
        );

        let mut scores = ScoreBoard::new();
        if let Some(totals) = self.scores {
            scores.set_totals(totals);
        }

        let mut tracker = UnseenTracker::new();
        tracker.reset_for_round(&round);

        if let Some(voids) = &self.voids {
            for seat in PlayerPosition::LOOP.iter().copied() {
                if let Some(flags) = voids.get(seat_key(seat)) {
                    for (idx, flag) in flags.iter().copied().enumerate() {
                        if flag && let Some(suit) = Suit::from_index(idx) {
                            tracker.note_void(seat, suit);
                        }
                    }
                }
            }
        }

        if let Some(moon_states) = &self.moon_states {
            for seat in PlayerPosition::LOOP.iter().copied() {
                if let Some(label) = moon_states.get(seat_key(seat)) {
                    let state = parse_moon_state(label)?;
                    tracker.set_moon_state(seat, state);
                }
            }
        }

        let next_to_play = if let Some(next_seat) = &self.next_to_play {
            parse_seat(next_seat)?
        } else {
            expected_player(round.current_trick())
        };

        Ok(EndgameRehydrate {
            round,
            scores,
            passing_direction,
            tracker,
            next_to_play,
        })
    }
}

fn trick_to_export(trick: &Trick) -> TrickExport {
    TrickExport {
        leader: format!("{:?}", trick.leader()),
        plays: trick
            .plays()
            .iter()
            .map(|Play { position, card }| PlayExport {
                seat: format!("{:?}", position),
                card: card.to_string(),
            })
            .collect(),
        penalties: Some(trick.penalty_total()),
        is_complete: Some(trick.is_complete()),
    }
}

#[cfg_attr(not(test), allow(dead_code))]
fn trick_from_export(export: &TrickExport) -> Result<Trick, EndgameLoadError> {
    let leader = parse_seat(&export.leader)?;
    let mut trick = Trick::new(leader);
    for play in &export.plays {
        let seat = parse_seat(&play.seat)?;
        let card = parse_card(&play.card)?;
        trick
            .play(seat, card)
            .map_err(|_| EndgameLoadError::TrickOutOfOrder)?;
    }
    Ok(trick)
}

#[cfg_attr(not(test), allow(dead_code))]
fn parse_card(text: &str) -> Result<Card, EndgameLoadError> {
    if text.len() < 2 {
        return Err(EndgameLoadError::UnknownCard(text.to_string()));
    }
    let (rank_part, suit_part) = text.split_at(text.len() - 1);
    let suit = match suit_part {
        "C" => Suit::Clubs,
        "D" => Suit::Diamonds,
        "S" => Suit::Spades,
        "H" => Suit::Hearts,
        other => return Err(EndgameLoadError::UnknownSuit(other.to_string())),
    };
    let rank = match rank_part {
        "A" => Rank::Ace,
        "K" => Rank::King,
        "Q" => Rank::Queen,
        "J" => Rank::Jack,
        "10" => Rank::Ten,
        "9" => Rank::Nine,
        "8" => Rank::Eight,
        "7" => Rank::Seven,
        "6" => Rank::Six,
        "5" => Rank::Five,
        "4" => Rank::Four,
        "3" => Rank::Three,
        "2" => Rank::Two,
        other => return Err(EndgameLoadError::UnknownRank(other.to_string())),
    };
    Ok(Card::new(rank, suit))
}

#[cfg_attr(not(test), allow(dead_code))]
fn parse_seat(text: &str) -> Result<PlayerPosition, EndgameLoadError> {
    match text.trim().to_ascii_lowercase().as_str() {
        "n" | "north" => Ok(PlayerPosition::North),
        "e" | "east" => Ok(PlayerPosition::East),
        "s" | "south" => Ok(PlayerPosition::South),
        "w" | "west" => Ok(PlayerPosition::West),
        other => Err(EndgameLoadError::UnknownSeat(other.to_string())),
    }
}

#[cfg_attr(not(test), allow(dead_code))]
fn parse_moon_state(text: &str) -> Result<MoonState, EndgameLoadError> {
    match text.trim().to_ascii_lowercase().as_str() {
        "inactive" => Ok(MoonState::Inactive),
        "considering" => Ok(MoonState::Considering),
        "committed" => Ok(MoonState::Committed),
        other => Err(EndgameLoadError::UnknownMoonState(other.to_string())),
    }
}

fn seat_key(seat: PlayerPosition) -> &'static str {
    match seat {
        PlayerPosition::North => "N",
        PlayerPosition::East => "E",
        PlayerPosition::South => "S",
        PlayerPosition::West => "W",
    }
}

#[cfg_attr(not(test), allow(dead_code))]
fn expected_player(trick: &Trick) -> PlayerPosition {
    trick
        .plays()
        .last()
        .map(|Play { position, .. }| position.next())
        .unwrap_or_else(|| trick.leader())
}
