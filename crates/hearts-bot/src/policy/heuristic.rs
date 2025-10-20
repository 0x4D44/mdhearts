use super::{Policy, PolicyContext};
use crate::bot::{
    BeliefView, BotContext, BotDifficulty, BotFeatures, BotStyle, Objective, PassPlanner,
    PlayPlanner, determine_style,
};
use hearts_core::model::card::Card;
use hearts_core::model::hand::Hand;
use hearts_core::model::player::PlayerPosition;
use hearts_core::model::rank::Rank;
use hearts_core::model::round::RoundState;
use hearts_core::model::suit::Suit;
use tracing::{Level, event};

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
        let features = ctx.features;
        let belief_view = belief_view_from_ctx(ctx, features);
        let bot_ctx = BotContext::new(
            ctx.seat,
            ctx.round,
            ctx.scores,
            ctx.passing_direction,
            ctx.tracker,
            belief_view,
            features,
            self.difficulty,
            &params,
            ctx.telemetry,
        );

        // For EasyLegacy, just return first 3 cards
        if matches!(self.difficulty, BotDifficulty::EasyLegacy) && ctx.hand.len() >= 3 {
            let cards = ctx.hand.cards();
            let selection = [cards[0], cards[1], cards[2]];
            log_pass_decision(ctx, self.difficulty, &selection, "easy_first_three");
            return selection;
        }

        let selection =
            PassPlanner::choose(ctx.hand, &bot_ctx).expect("PassPlanner returns valid pass");
        log_pass_decision(ctx, self.difficulty, &selection, "heuristic_pass");
        selection
    }

    fn choose_play(&mut self, ctx: &PolicyContext) -> Card {
        let params = crate::bot::BotParams::default();
        let features = ctx.features;
        let belief_view = belief_view_from_ctx(ctx, features);
        let bot_ctx = BotContext::new(
            ctx.seat,
            ctx.round,
            ctx.scores,
            ctx.passing_direction,
            ctx.tracker,
            belief_view,
            features,
            self.difficulty,
            &params,
            ctx.telemetry,
        );

        // Compute legal moves
        let legal_moves = compute_legal_moves(ctx.seat, ctx.hand, ctx.round);
        let objective = bot_ctx.objective_hint();
        if legal_moves.is_empty() {
            if let Some(card) = ctx
                .round
                .hand(ctx.seat)
                .iter()
                .copied()
                .next()
                .or_else(|| ctx.hand.iter().copied().next())
            {
                log_play_decision(
                    ctx,
                    self.difficulty,
                    BotStyle::Cautious,
                    objective,
                    &[],
                    card,
                    "fallback_empty_legal",
                );
                return card;
            }
            panic!("heuristic policy expected at least one legal card");
        }

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
            let chosen = legal_moves
                .first()
                .copied()
                .expect("At least one legal move exists");
            log_play_decision(
                ctx,
                self.difficulty,
                BotStyle::Cautious,
                objective,
                &legal_moves,
                chosen,
                "easy_first_legal",
            );
            return chosen;
        }

        let style = determine_style(&bot_ctx);
        let chosen = PlayPlanner::choose(&legal_moves, &bot_ctx)
            .or_else(|| legal_moves.first().copied())
            .expect("PlayPlanner returns valid card");
        log_play_decision(
            ctx,
            self.difficulty,
            style,
            objective,
            &legal_moves,
            chosen,
            "heuristic_play",
        );
        chosen
    }
}

fn belief_view_from_ctx<'a>(
    ctx: &PolicyContext<'a>,
    features: BotFeatures,
) -> Option<BeliefView<'a>> {
    if !features.belief_enabled() {
        return None;
    }
    ctx.belief
        .map(|belief| BeliefView::new(belief, features.void_threshold()))
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

fn log_pass_decision(
    ctx: &PolicyContext,
    difficulty: BotDifficulty,
    selection: &[Card; 3],
    reason: &str,
) {
    if !tracing::enabled!(Level::INFO) {
        return;
    }

    if !moon_logging_enabled() {
        return;
    }

    let cards = selection
        .iter()
        .map(|card| format!("{:?}", card))
        .collect::<Vec<_>>()
        .join(",");

    event!(
        target: "hearts_bot::pass",
        Level::INFO,
        seat = ?ctx.seat,
        difficulty = ?difficulty,
        hand_size = ctx.hand.len(),
        unseen = ctx.tracker.unseen_count(),
        reason,
        cards = %cards
    );
}

fn moon_logging_enabled() -> bool {
    std::env::var("MDH_MOON_DETAILS")
        .map(|raw| matches!(raw.trim(), "1" | "true" | "TRUE" | "on" | "ON"))
        .unwrap_or(false)
}

fn log_play_decision(
    ctx: &PolicyContext,
    difficulty: BotDifficulty,
    style: BotStyle,
    objective: Objective,
    legal_moves: &[Card],
    chosen: Card,
    reason: &str,
) {
    if !tracing::enabled!(Level::INFO) {
        return;
    }

    let choice = format!("{chosen:?}");
    let legal_preview = if legal_moves.len() <= 6 {
        legal_moves
            .iter()
            .map(|card| format!("{:?}", card))
            .collect::<Vec<_>>()
            .join(",")
    } else {
        format!("{} moves", legal_moves.len())
    };

    event!(
        target: "hearts_bot::play",
        Level::INFO,
        seat = ?ctx.seat,
        difficulty = ?difficulty,
        style = ?style,
        objective = ?objective,
        legal_count = legal_moves.len(),
        legal_moves = %legal_preview,
        chosen = %choice,
        hearts_broken = ctx.round.hearts_broken(),
        trick_cards = ctx.round.current_trick().plays().len(),
        reason,
    );
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
            belief: None,
            features: BotFeatures::default(),
            telemetry: None,
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
            belief: None,
            features: BotFeatures::default(),
            telemetry: None,
        };

        let mut policy = HeuristicPolicy::normal();
        let result = policy.choose_pass(&ctx);

        // PassPlanner should prefer to pass away high hearts and Queen of Spades
        assert!(result.contains(&Card::new(Rank::Ace, Suit::Hearts)));
        assert!(result.contains(&Card::new(Rank::King, Suit::Hearts)));
        assert!(result.contains(&Card::new(Rank::Queen, Suit::Spades)));
    }

    #[test]
    fn moon_logging_disabled_without_env() {
        unsafe {
            std::env::remove_var("MDH_MOON_DETAILS");
        }
        assert!(!super::moon_logging_enabled());
    }

    #[test]
    fn moon_logging_enabled_with_env_flag() {
        unsafe {
            std::env::set_var("MDH_MOON_DETAILS", "on");
        }
        assert!(super::moon_logging_enabled());
        unsafe {
            std::env::remove_var("MDH_MOON_DETAILS");
        }
    }
}
