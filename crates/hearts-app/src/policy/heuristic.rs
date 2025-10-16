use super::{Policy, PolicyContext};
use crate::bot::{BotContext, BotDifficulty, PassPlanner, PlayPlanner};
use hearts_core::model::card::Card;
use hearts_core::model::hand::Hand;
use hearts_core::model::player::PlayerPosition;
use hearts_core::model::rank::Rank;
use hearts_core::model::round::RoundState;
use hearts_core::model::suit::Suit;

/// Adapter that wraps existing PassPlanner/PlayPlanner to implement the Policy trait
pub struct HeuristicPolicy {
    difficulty: BotDifficulty,
}

impl HeuristicPolicy {
    pub fn new(difficulty: BotDifficulty) -> Self {
        Self { difficulty }
    }

    pub fn easy() -> Self {
        Self::new(BotDifficulty::EasyLegacy)
    }

    pub fn normal() -> Self {
        Self::new(BotDifficulty::NormalHeuristic)
    }

    #[allow(dead_code)]
    pub fn hard() -> Self {
        Self::new(BotDifficulty::FutureHard)
    }
}

impl Policy for HeuristicPolicy {
    fn choose_pass(&mut self, ctx: &PolicyContext) -> [Card; 3] {
        let params = crate::bot::BotParams::default();
        let bot_ctx = BotContext::new(
            ctx.seat,
            ctx.round,
            ctx.scores,
            ctx.passing_direction,
            ctx.tracker,
            self.difficulty,
            &params,
        );

        // For EasyLegacy, just return first 3 cards
        if matches!(self.difficulty, BotDifficulty::EasyLegacy) && ctx.hand.len() >= 3 {
            let cards = ctx.hand.cards();
            return [cards[0], cards[1], cards[2]];
        }

        PassPlanner::choose(ctx.hand, &bot_ctx).expect("PassPlanner returns valid pass")
    }

    fn choose_play(&mut self, ctx: &PolicyContext) -> Card {
        let params = crate::bot::BotParams::default();
        let bot_ctx = BotContext::new(
            ctx.seat,
            ctx.round,
            ctx.scores,
            ctx.passing_direction,
            ctx.tracker,
            self.difficulty,
            &params,
        );

        // Compute legal moves
        let legal_moves = compute_legal_moves(ctx.seat, ctx.hand, ctx.round);

        // Check if this is the first trick and we're the leader
        let enforce_two =
            ctx.round.is_first_trick() && ctx.round.current_trick().leader() == ctx.seat;

        if enforce_two {
            let two = Card::new(Rank::Two, Suit::Clubs);
            if legal_moves.contains(&two) {
                return two;
            }
        }

        // For EasyLegacy, just return first legal card
        if matches!(self.difficulty, BotDifficulty::EasyLegacy) {
            return legal_moves
                .first()
                .copied()
                .expect("At least one legal move exists");
        }

        PlayPlanner::choose(&legal_moves, &bot_ctx)
            .or_else(|| legal_moves.first().copied())
            .expect("PlayPlanner returns valid card")
    }
}

/// Compute legal moves for a given hand and round state
fn compute_legal_moves(seat: PlayerPosition, hand: &Hand, round: &RoundState) -> Vec<Card> {
    hand.iter()
        .copied()
        .filter(|&card| {
            let mut probe = round.clone();
            probe.play_card(seat, card).is_ok()
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::bot::UnseenTracker;
    use hearts_core::model::passing::PassingDirection;
    use hearts_core::model::round::RoundPhase;
    use hearts_core::model::score::ScoreBoard;

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
    fn easy_policy_returns_first_three_cards() {
        let seat = PlayerPosition::South;
        let hand_cards = vec![
            Card::new(Rank::Two, Suit::Clubs),
            Card::new(Rank::Three, Suit::Clubs),
            Card::new(Rank::Four, Suit::Clubs),
            Card::new(Rank::Five, Suit::Diamonds),
        ];
        let round = build_round(seat, &hand_cards);
        let hand = round.hand(seat);
        let scores = build_scores([0, 0, 0, 0]);
        let tracker = make_tracker(&round);

        let ctx = PolicyContext {
            seat,
            hand,
            round: &round,
            scores: &scores,
            passing_direction: PassingDirection::Left,
            tracker: &tracker,
        };

        let mut policy = HeuristicPolicy::easy();
        let result = policy.choose_pass(&ctx);

        assert_eq!(result[0], hand_cards[0]);
        assert_eq!(result[1], hand_cards[1]);
        assert_eq!(result[2], hand_cards[2]);
    }

    #[test]
    fn normal_policy_uses_pass_planner() {
        let seat = PlayerPosition::South;
        let hand_cards = vec![
            Card::new(Rank::Ace, Suit::Hearts),
            Card::new(Rank::King, Suit::Hearts),
            Card::new(Rank::Queen, Suit::Spades),
            Card::new(Rank::Two, Suit::Clubs),
        ];
        let round = build_round(seat, &hand_cards);
        let hand = round.hand(seat);
        let scores = build_scores([0, 0, 0, 0]);
        let tracker = make_tracker(&round);

        let ctx = PolicyContext {
            seat,
            hand,
            round: &round,
            scores: &scores,
            passing_direction: PassingDirection::Left,
            tracker: &tracker,
        };

        let mut policy = HeuristicPolicy::normal();
        let result = policy.choose_pass(&ctx);

        // PassPlanner should prefer to pass away high hearts and Queen of Spades
        assert!(result.contains(&Card::new(Rank::Ace, Suit::Hearts)));
        assert!(result.contains(&Card::new(Rank::King, Suit::Hearts)));
        assert!(result.contains(&Card::new(Rank::Queen, Suit::Spades)));
    }
}
