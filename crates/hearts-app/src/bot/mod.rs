mod adviser;
mod pass;
mod play;
pub mod search;
mod tracker;

pub use adviser::play_bias;
pub use pass::PassPlanner;
pub use play::{PlayPlanner, debug_weights_string};
pub use search::{PlayPlannerHard, debug_hard_weights_string};
pub use tracker::{MoonState, UnseenTracker};

use hearts_core::model::card::Card;
use hearts_core::model::hand::Hand;
use hearts_core::model::passing::PassingDirection;
use hearts_core::model::player::PlayerPosition;
use hearts_core::model::rank::Rank;
use hearts_core::model::round::RoundState;
use hearts_core::model::score::ScoreBoard;
use hearts_core::model::suit::Suit;
use std::sync::OnceLock;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BotDifficulty {
    EasyLegacy,
    NormalHeuristic,
    FutureHard,
    SearchLookahead,
}

impl Default for BotDifficulty {
    fn default() -> Self {
        Self::NormalHeuristic
    }
}

impl BotDifficulty {
    pub fn from_env() -> Self {
        static CACHED: OnceLock<BotDifficulty> = OnceLock::new();
        *CACHED.get_or_init(|| match std::env::var("MDH_BOT_DIFFICULTY") {
            Ok(raw) => match raw.trim().to_ascii_lowercase().as_str() {
                "easy" => BotDifficulty::EasyLegacy,
                "legacy" => BotDifficulty::EasyLegacy,
                "normal" => BotDifficulty::NormalHeuristic,
                "default" => BotDifficulty::NormalHeuristic,
                "hard" => BotDifficulty::FutureHard,
                "future" => BotDifficulty::FutureHard,
                "search" => BotDifficulty::SearchLookahead,
                "lookahead" => BotDifficulty::SearchLookahead,
                _ => BotDifficulty::default(),
            },
            Err(_) => BotDifficulty::default(),
        })
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BotStyle {
    Cautious,
    AggressiveMoon,
    HuntLeader,
}

#[derive(Debug, Clone, Copy)]
pub struct ScoreSnapshot {
    pub min_score: u32,
    pub max_score: u32,
    pub min_player: PlayerPosition,
    pub max_player: PlayerPosition,
    pub leader_gap: u32,
    pub leader_unique: bool,
}

#[derive(Debug, Clone, Copy)]
pub struct BotContext<'a> {
    pub seat: PlayerPosition,
    pub round: &'a RoundState,
    pub scores: &'a ScoreBoard,
    pub passing_direction: PassingDirection,
    pub tracker: &'a UnseenTracker,
    pub difficulty: BotDifficulty,
}

impl<'a> BotContext<'a> {
    pub fn new(
        seat: PlayerPosition,
        round: &'a RoundState,
        scores: &'a ScoreBoard,
        passing_direction: PassingDirection,
        tracker: &'a UnseenTracker,
        difficulty: BotDifficulty,
    ) -> Self {
        Self {
            seat,
            round,
            scores,
            passing_direction,
            tracker,
            difficulty,
        }
    }

    pub fn hand(&self) -> &'a Hand {
        self.round.hand(self.seat)
    }

    pub fn cards_played(&self) -> usize {
        52usize.saturating_sub(self.tracker.unseen_count())
    }
}

pub(crate) fn determine_style(ctx: &BotContext<'_>) -> BotStyle {
    let snapshot = snapshot_scores(ctx.scores);
    let my_score = ctx.scores.score(ctx.seat);
    let hand = ctx.hand();

    // Persisted moon attempt
    if ctx.tracker.moon_state(ctx.seat) == MoonState::Committed {
        return BotStyle::AggressiveMoon;
    }

    if should_try_shoot_moon(hand, my_score, snapshot.min_score, ctx.cards_played()) {
        return BotStyle::AggressiveMoon;
    }

    if matches!(ctx.difficulty, BotDifficulty::FutureHard)
        && snapshot.max_score >= 80
        && snapshot.max_player != ctx.seat
    {
        return BotStyle::HuntLeader;
    }

    if snapshot.max_score >= 90 && snapshot.max_player != ctx.seat {
        return BotStyle::HuntLeader;
    }

    BotStyle::Cautious
}

fn should_try_shoot_moon(
    hand: &Hand,
    my_score: u32,
    leader_score: u32,
    cards_played: usize,
) -> bool {
    if cards_played > 12 {
        return false;
    }
    if my_score >= 70 {
        return false;
    }
    if my_score > leader_score + 15 {
        return false;
    }

    let hearts = count_cards_in_suit(hand, Suit::Hearts);
    if hearts < 7 {
        return false;
    }

    let control_hearts = hand
        .iter()
        .filter(|card| card.suit == Suit::Hearts && card.rank >= Rank::Ten)
        .count();
    let high_spades = hand
        .iter()
        .filter(|card| card.suit == Suit::Spades && card.rank >= Rank::Queen)
        .count();
    let has_ace_spades = hand.contains(Card::new(Rank::Ace, Suit::Spades));
    control_hearts >= 4 && high_spades >= 2 && has_ace_spades
}

pub(crate) fn snapshot_scores(scores: &ScoreBoard) -> ScoreSnapshot {
    let mut min_score = u32::MAX;
    let mut max_score = u32::MIN;
    let mut min_player = PlayerPosition::North;
    let mut max_player = PlayerPosition::North;
    let mut second_highest = 0u32;
    let mut leader_unique = true;

    for seat in PlayerPosition::LOOP.iter().copied() {
        let value = scores.score(seat);
        if value < min_score {
            min_score = value;
            min_player = seat;
        }
        if value > max_score {
            second_highest = max_score;
            max_score = value;
            max_player = seat;
            leader_unique = true;
        } else if value == max_score {
            leader_unique = false;
            second_highest = max_score;
        } else if value > second_highest {
            second_highest = value;
        }
    }

    ScoreSnapshot {
        min_score,
        max_score,
        min_player,
        max_player,
        leader_gap: if leader_unique {
            max_score.saturating_sub(second_highest)
        } else {
            0
        },
        leader_unique,
    }
}

pub(crate) fn count_cards_in_suit(hand: &Hand, suit: Suit) -> usize {
    hand.iter().filter(|card| card.suit == suit).count()
}

pub(crate) fn card_sort_key(card: Card) -> (u8, u8) {
    (card.suit as u8, card.rank.value())
}

#[cfg(test)]
mod tests {
    use super::*;
    use hearts_core::model::round::RoundPhase;

    fn build_round(seat: PlayerPosition, hand_cards: &[Card]) -> RoundState {
        let mut hands = [Hand::new(), Hand::new(), Hand::new(), Hand::new()];
        hands[seat.index()] = Hand::with_cards(hand_cards.to_vec());
        RoundState::from_hands(hands, seat, PassingDirection::Hold, RoundPhase::Playing)
    }

    fn build_scores(values: [u32; 4]) -> ScoreBoard {
        let mut scores = ScoreBoard::new();
        for (idx, value) in values.iter().enumerate() {
            if let Some(pos) = PlayerPosition::from_index(idx) {
                scores.set_score(pos, *value);
            }
        }
        scores
    }

    fn make_tracker(round: &RoundState) -> UnseenTracker {
        let mut tracker = UnseenTracker::new();
        tracker.reset_for_round(round);
        tracker
    }

    #[test]
    fn style_moon_threshold() {
        let seat = PlayerPosition::South;
        let hand = vec![
            Card::new(Rank::Ace, Suit::Hearts),
            Card::new(Rank::King, Suit::Hearts),
            Card::new(Rank::Queen, Suit::Hearts),
            Card::new(Rank::Jack, Suit::Hearts),
            Card::new(Rank::Ten, Suit::Hearts),
            Card::new(Rank::Nine, Suit::Hearts),
            Card::new(Rank::Eight, Suit::Hearts),
            Card::new(Rank::Ace, Suit::Spades),
            Card::new(Rank::King, Suit::Spades),
            Card::new(Rank::Two, Suit::Clubs),
            Card::new(Rank::Three, Suit::Clubs),
            Card::new(Rank::Four, Suit::Diamonds),
            Card::new(Rank::Five, Suit::Diamonds),
        ];
        let round = build_round(seat, &hand);
        let scores = build_scores([18, 24, 22, 27]);
        let tracker = make_tracker(&round);
        let ctx = BotContext::new(
            seat,
            &round,
            &scores,
            PassingDirection::Hold,
            &tracker,
            BotDifficulty::NormalHeuristic,
        );
        assert_eq!(determine_style(&ctx), BotStyle::AggressiveMoon);
    }

    #[test]
    fn style_forced_by_committed_moon_state() {
        let seat = PlayerPosition::South;
        let hand = vec![
            Card::new(Rank::Two, Suit::Clubs),
            Card::new(Rank::Three, Suit::Clubs),
            Card::new(Rank::Four, Suit::Diamonds),
            Card::new(Rank::Five, Suit::Diamonds),
            Card::new(Rank::Six, Suit::Spades),
            Card::new(Rank::Seven, Suit::Spades),
            Card::new(Rank::Eight, Suit::Hearts),
            Card::new(Rank::Nine, Suit::Hearts),
            Card::new(Rank::Ten, Suit::Clubs),
            Card::new(Rank::Jack, Suit::Clubs),
            Card::new(Rank::Queen, Suit::Diamonds),
            Card::new(Rank::King, Suit::Diamonds),
            Card::new(Rank::Ace, Suit::Diamonds),
        ];
        let round = build_round(seat, &hand);
        let scores = build_scores([30, 38, 22, 27]);
        let mut tracker = make_tracker(&round);
        tracker.set_moon_state(seat, crate::bot::MoonState::Committed);
        let ctx = BotContext::new(
            seat,
            &round,
            &scores,
            PassingDirection::Hold,
            &tracker,
            BotDifficulty::NormalHeuristic,
        );
        assert_eq!(determine_style(&ctx), BotStyle::AggressiveMoon);
    }

    #[test]
    fn style_hunt_leader_normal() {
        let seat = PlayerPosition::South;
        let hand = vec![
            Card::new(Rank::Ace, Suit::Clubs),
            Card::new(Rank::King, Suit::Diamonds),
            Card::new(Rank::Three, Suit::Spades),
        ];
        let round = build_round(seat, &hand);
        let scores = build_scores([15, 94, 20, 18]);
        let tracker = make_tracker(&round);
        let ctx = BotContext::new(
            seat,
            &round,
            &scores,
            PassingDirection::Hold,
            &tracker,
            BotDifficulty::NormalHeuristic,
        );
        assert_eq!(determine_style(&ctx), BotStyle::HuntLeader);
    }

    #[test]
    fn style_futurehard_bias() {
        let seat = PlayerPosition::East;
        let hand = vec![
            Card::new(Rank::Ace, Suit::Clubs),
            Card::new(Rank::Five, Suit::Diamonds),
            Card::new(Rank::Seven, Suit::Spades),
        ];
        let round = build_round(seat, &hand);
        let scores = build_scores([40, 50, 82, 60]);
        let tracker = make_tracker(&round);
        let ctx = BotContext::new(
            seat,
            &round,
            &scores,
            PassingDirection::Hold,
            &tracker,
            BotDifficulty::FutureHard,
        );
        assert_eq!(determine_style(&ctx), BotStyle::HuntLeader);
    }

    #[test]
    fn moon_aborts_after_penalties() {
        let seat = PlayerPosition::South;
        let hand = vec![
            Card::new(Rank::Ace, Suit::Hearts),
            Card::new(Rank::King, Suit::Hearts),
            Card::new(Rank::Queen, Suit::Hearts),
            Card::new(Rank::Jack, Suit::Hearts),
            Card::new(Rank::Ten, Suit::Hearts),
            Card::new(Rank::Nine, Suit::Hearts),
            Card::new(Rank::Eight, Suit::Hearts),
            Card::new(Rank::Ace, Suit::Spades),
            Card::new(Rank::King, Suit::Spades),
            Card::new(Rank::Two, Suit::Clubs),
            Card::new(Rank::Three, Suit::Clubs),
            Card::new(Rank::Four, Suit::Diamonds),
            Card::new(Rank::Five, Suit::Diamonds),
        ];
        let round = build_round(seat, &hand);
        let scores = build_scores([20, 18, 76, 42]);
        let mut tracker = make_tracker(&round);
        for rank in Rank::ORDERED.iter().take(10) {
            tracker.note_card_revealed(Card::new(*rank, Suit::Clubs));
        }
        let ctx = BotContext::new(
            seat,
            &round,
            &scores,
            PassingDirection::Hold,
            &tracker,
            BotDifficulty::NormalHeuristic,
        );
        assert_eq!(determine_style(&ctx), BotStyle::Cautious);
    }

    #[test]
    fn style_threshold_boundary() {
        let seat = PlayerPosition::North;
        let hand = vec![Card::new(Rank::Ace, Suit::Clubs)];
        let round = build_round(seat, &hand);
        let tracker = make_tracker(&round);

        let style_89 = {
            let scores = build_scores([10, 20, 30, 89]);
            let ctx = BotContext::new(
                seat,
                &round,
                &scores,
                PassingDirection::Hold,
                &tracker,
                BotDifficulty::NormalHeuristic,
            );
            determine_style(&ctx)
        };
        assert_eq!(style_89, BotStyle::Cautious);

        let style_90 = {
            let scores = build_scores([10, 20, 30, 90]);
            let ctx = BotContext::new(
                seat,
                &round,
                &scores,
                PassingDirection::Hold,
                &tracker,
                BotDifficulty::NormalHeuristic,
            );
            determine_style(&ctx)
        };
        assert_eq!(style_90, BotStyle::HuntLeader);
    }
}
