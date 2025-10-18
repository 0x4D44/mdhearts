use std::io::Write;
use std::process::{Command, Stdio};
use std::time::Instant;

use hearts_bot::bot::BotDifficulty;
#[cfg(test)]
use hearts_bot::bot::BotFeatures;
#[cfg(test)]
use hearts_bot::bot::UnseenTracker;
use hearts_bot::policy::{HeuristicPolicy, Policy, PolicyContext};
use hearts_core::model::card::Card;
use hearts_core::model::hand::Hand;
use hearts_core::model::player::PlayerPosition;
use hearts_core::model::rank::Rank;
use hearts_core::model::round::RoundState;
use hearts_core::model::score::ScoreBoard;
use hearts_core::model::suit::Suit;
#[cfg(test)]
use hearts_core::model::{hand::Hand as TestHand, passing::PassingDirection, round::RoundPhase};
#[cfg(test)]
use hearts_core::model::{passing::PassingState, round::RoundState as TestRoundState};
use serde::{Deserialize, Serialize};
use serde_json;
use thiserror::Error;
use tracing::{Level, event};

use super::{ExternalFallback, ExternalOptions};

pub struct ExternalPolicy {
    name: String,
    options: ExternalOptions,
    fallback: HeuristicPolicy,
}

impl ExternalPolicy {
    pub fn new(name: String, options: ExternalOptions) -> Self {
        let fallback_difficulty = match &options.fallback {
            ExternalFallback::Heuristic(difficulty) => *difficulty,
            ExternalFallback::Error => BotDifficulty::NormalHeuristic,
        };
        if options.command.is_none() {
            event!(
                target: "hearts_bench::external",
                Level::WARN,
                agent = %name,
                "no external command configured; using fallback heuristic"
            );
        }
        Self {
            name,
            options,
            fallback: HeuristicPolicy::new(fallback_difficulty),
        }
    }

    fn invoke<Request, Response>(
        &self,
        action: &str,
        request: &Request,
    ) -> Result<Response, ExternalInvokeError>
    where
        Request: Serialize,
        Response: for<'de> Deserialize<'de>,
    {
        let command = match &self.options.command {
            Some(cmd) if !cmd.is_empty() => cmd,
            _ => return Err(ExternalInvokeError::NoCommand),
        };

        let mut cmd = Command::new(command);
        if !self.options.args.is_empty() {
            cmd.args(&self.options.args);
        }
        if let Some(dir) = &self.options.working_dir {
            cmd.current_dir(dir);
        }
        cmd.stdin(Stdio::piped()).stdout(Stdio::piped());

        let start = Instant::now();
        let mut child = cmd
            .spawn()
            .map_err(|err| ExternalInvokeError::Spawn(err.to_string()))?;
        {
            let mut stdin = child
                .stdin
                .take()
                .ok_or_else(|| ExternalInvokeError::Io("stdin".into()))?;
            serde_json::to_writer(&mut stdin, request)
                .map_err(|err| ExternalInvokeError::Protocol(err.to_string()))?;
            stdin
                .write_all(b"\n")
                .map_err(|err| ExternalInvokeError::Io(err.to_string()))?;
        }

        let output = child
            .wait_with_output()
            .map_err(|err| ExternalInvokeError::Io(err.to_string()))?;

        if !output.status.success() {
            return Err(ExternalInvokeError::Status(format!(
                "exit status {}",
                output.status
            )));
        }

        let elapsed_ms = start.elapsed().as_secs_f64() * 1000.0;
        if let Some(timeout) = self.options.timeout_ms
            && elapsed_ms > timeout as f64
        {
            event!(
                target: "hearts_bench::external",
                Level::WARN,
                agent = %self.name,
                action,
                elapsed_ms,
                timeout_ms = timeout,
                "external invocation exceeded timeout"
            );
        }

        let response: Response = serde_json::from_slice(&output.stdout)
            .map_err(|err| ExternalInvokeError::Protocol(err.to_string()))?;
        Ok(response)
    }

    fn fallback_pass(&mut self, ctx: &PolicyContext) -> [Card; 3] {
        match self.options.fallback {
            ExternalFallback::Heuristic(_) => self.fallback.choose_pass(ctx),
            ExternalFallback::Error => {
                panic!(
                    "External agent '{}' is misconfigured or failed to respond and no fallback is allowed.",
                    self.name
                );
            }
        }
    }

    fn fallback_play(&mut self, ctx: &PolicyContext) -> Card {
        match self.options.fallback {
            ExternalFallback::Heuristic(_) => self.fallback.choose_play(ctx),
            ExternalFallback::Error => {
                panic!(
                    "External agent '{}' is misconfigured or failed to respond and no fallback is allowed.",
                    self.name
                );
            }
        }
    }

    fn build_pass_request(ctx: &PolicyContext) -> PassRequest {
        PassRequest {
            action: "pass",
            seat: seat_label(ctx.seat).to_string(),
            hand: map_cards(ctx.hand),
            scores: score_array(ctx.scores),
            passing_direction: ctx.passing_direction.as_str().to_string(),
        }
    }

    fn build_play_request(ctx: &PolicyContext, legal_moves: &[Card]) -> PlayRequest {
        let current_trick = ctx.round.current_trick();
        PlayRequest {
            action: "play",
            seat: seat_label(ctx.seat).to_string(),
            hand: map_cards(ctx.hand),
            scores: score_array(ctx.scores),
            legal_moves: map_cards_slice(legal_moves),
            trick: current_trick
                .plays()
                .iter()
                .map(|play| TrickCard {
                    seat: seat_label(play.position).to_string(),
                    card: card_to_string(play.card),
                })
                .collect(),
            trick_leader: seat_label(current_trick.leader()).to_string(),
        }
    }
}

impl Policy for ExternalPolicy {
    fn choose_pass(&mut self, ctx: &PolicyContext) -> [Card; 3] {
        let request = ExternalPolicy::build_pass_request(ctx);
        match self.invoke::<_, PassResponse>("pass", &request) {
            Ok(response) => match parse_card_list(&response.cards) {
                Some(cards) if cards.len() == 3 => [cards[0], cards[1], cards[2]],
                _ => {
                    event!(
                        target: "hearts_bench::external",
                        Level::WARN,
                        agent = %self.name,
                        "invalid pass response; falling back"
                    );
                    self.fallback_pass(ctx)
                }
            },
            Err(err) => {
                event!(
                    target: "hearts_bench::external",
                    Level::WARN,
                    agent = %self.name,
                    error = %err,
                    "external pass request failed; falling back"
                );
                self.fallback_pass(ctx)
            }
        }
    }

    fn choose_play(&mut self, ctx: &PolicyContext) -> Card {
        let legal_moves = compute_legal_moves(ctx.seat, ctx.hand, ctx.round);
        if legal_moves.is_empty() {
            return self.fallback_play(ctx);
        }

        let request = ExternalPolicy::build_play_request(ctx, &legal_moves);
        match self.invoke::<_, PlayResponse>("play", &request) {
            Ok(response) => match card_from_string(&response.card) {
                Some(card) if legal_moves.contains(&card) => card,
                _ => {
                    event!(
                        target: "hearts_bench::external",
                        Level::WARN,
                        agent = %self.name,
                        "invalid play response; falling back"
                    );
                    self.fallback_play(ctx)
                }
            },
            Err(err) => {
                event!(
                    target: "hearts_bench::external",
                    Level::WARN,
                    agent = %self.name,
                    error = %err,
                    "external play request failed; falling back"
                );
                self.fallback_play(ctx)
            }
        }
    }
}

#[derive(Debug, Error)]
enum ExternalInvokeError {
    #[error("no command configured")]
    NoCommand,
    #[error("failed to spawn process: {0}")]
    Spawn(String),
    #[error("I/O error: {0}")]
    Io(String),
    #[error("protocol error: {0}")]
    Protocol(String),
    #[error("non-zero exit status: {0}")]
    Status(String),
}

#[derive(Serialize)]
struct PassRequest {
    action: &'static str,
    seat: String,
    hand: Vec<String>,
    scores: [u32; 4],
    passing_direction: String,
}

#[derive(Deserialize)]
struct PassResponse {
    cards: Vec<String>,
}

#[derive(Serialize)]
struct PlayRequest {
    action: &'static str,
    seat: String,
    hand: Vec<String>,
    scores: [u32; 4],
    legal_moves: Vec<String>,
    trick: Vec<TrickCard>,
    trick_leader: String,
}

#[derive(Deserialize)]
struct PlayResponse {
    card: String,
}

#[derive(Serialize)]
struct TrickCard {
    seat: String,
    card: String,
}

fn map_cards(hand: &Hand) -> Vec<String> {
    hand.iter().map(|card| card_to_string(*card)).collect()
}

fn map_cards_slice(cards: &[Card]) -> Vec<String> {
    cards.iter().map(|card| card_to_string(*card)).collect()
}

fn score_array(scores: &ScoreBoard) -> [u32; 4] {
    let mut arr = [0u32; 4];
    for seat in PlayerPosition::LOOP.iter().copied() {
        arr[seat.index()] = scores.score(seat);
    }
    arr
}

fn seat_label(seat: PlayerPosition) -> &'static str {
    super::seat_label(seat)
}

fn card_to_string(card: Card) -> String {
    let rank = match card.rank {
        Rank::Two => "2",
        Rank::Three => "3",
        Rank::Four => "4",
        Rank::Five => "5",
        Rank::Six => "6",
        Rank::Seven => "7",
        Rank::Eight => "8",
        Rank::Nine => "9",
        Rank::Ten => "T",
        Rank::Jack => "J",
        Rank::Queen => "Q",
        Rank::King => "K",
        Rank::Ace => "A",
    };
    let suit = match card.suit {
        Suit::Clubs => 'C',
        Suit::Diamonds => 'D',
        Suit::Spades => 'S',
        Suit::Hearts => 'H',
    };
    format!("{rank}{suit}")
}

fn card_from_string(value: &str) -> Option<Card> {
    if value.len() != 2 {
        return None;
    }
    let rank = match &value[..1] {
        "2" => Rank::Two,
        "3" => Rank::Three,
        "4" => Rank::Four,
        "5" => Rank::Five,
        "6" => Rank::Six,
        "7" => Rank::Seven,
        "8" => Rank::Eight,
        "9" => Rank::Nine,
        "T" | "t" => Rank::Ten,
        "J" | "j" => Rank::Jack,
        "Q" | "q" => Rank::Queen,
        "K" | "k" => Rank::King,
        "A" | "a" => Rank::Ace,
        _ => return None,
    };
    let suit = match &value[1..2] {
        "C" | "c" => Suit::Clubs,
        "D" | "d" => Suit::Diamonds,
        "S" | "s" => Suit::Spades,
        "H" | "h" => Suit::Hearts,
        _ => return None,
    };
    Some(Card::new(rank, suit))
}

fn parse_card_list(values: &[String]) -> Option<Vec<Card>> {
    values.iter().map(|v| card_from_string(v)).collect()
}

fn compute_legal_moves(seat: PlayerPosition, hand: &Hand, round: &RoundState) -> Vec<Card> {
    hand.iter()
        .copied()
        .filter(|card| {
            let mut probe = round.clone();
            probe.play_card(seat, *card).is_ok()
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::iter::FromIterator;

    #[test]
    fn fallback_invoked_when_command_missing() {
        let params = serde_yaml::Mapping::from_iter([
            (
                serde_yaml::Value::String("command".into()),
                serde_yaml::Value::String("__hearts_bench_missing__".into()),
            ),
            (
                serde_yaml::Value::String("fallback".into()),
                serde_yaml::Value::String("heuristic_easy".into()),
            ),
        ]);
        let options = ExternalOptions::from_params("xinxin", &serde_yaml::Value::Mapping(params))
            .expect("parse options");
        let mut policy = ExternalPolicy::new("xinxin".into(), options);

        let mut hands: [TestHand; 4] = std::array::from_fn(|_| TestHand::new());
        hands[PlayerPosition::North.index()] = TestHand::with_cards(vec![
            Card::new(Rank::Two, Suit::Clubs),
            Card::new(Rank::Three, Suit::Clubs),
            Card::new(Rank::Four, Suit::Clubs),
            Card::new(Rank::Five, Suit::Diamonds),
        ]);
        let round = TestRoundState::from_hands(
            hands,
            PlayerPosition::North,
            PassingDirection::Left,
            RoundPhase::Passing(PassingState::new(PassingDirection::Left)),
        );
        let scores = ScoreBoard::new();
        let tracker = UnseenTracker::new();
        let ctx = PolicyContext {
            seat: PlayerPosition::North,
            hand: round.hand(PlayerPosition::North),
            round: &round,
            scores: &scores,
            passing_direction: round.passing_direction(),
            tracker: &tracker,
            belief: None,
            features: BotFeatures::default(),
        };
        let pass = policy.choose_pass(&ctx);
        assert_eq!(pass.len(), 3);

        let play_round = TestRoundState::from_hands(
            [
                ctx.hand.clone(),
                TestHand::new(),
                TestHand::new(),
                TestHand::new(),
            ],
            PlayerPosition::North,
            PassingDirection::Hold,
            RoundPhase::Playing,
        );
        let play_scores = ScoreBoard::new();
        let play_tracker = UnseenTracker::new();
        let play_ctx = PolicyContext {
            seat: PlayerPosition::North,
            hand: play_round.hand(PlayerPosition::North),
            round: &play_round,
            scores: &play_scores,
            passing_direction: play_round.passing_direction(),
            tracker: &play_tracker,
            belief: None,
            features: BotFeatures::default(),
        };
        let card = policy.choose_play(&play_ctx);
        assert!(play_ctx.hand.contains(card));
    }
}
