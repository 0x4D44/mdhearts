use super::{
    BotContext, BotStyle, Objective, card_sort_key, count_cards_in_suit, determine_style,
    snapshot_scores,
};
use hearts_core::model::card::Card;
use hearts_core::model::hand::Hand;
use hearts_core::model::rank::Rank;
use hearts_core::model::suit::Suit;
use hearts_core::moon::MoonEstimate;
use hearts_core::pass::{
    DirectionProfile, PassCandidate, PassScoreInput, PassWeights, enumerate_pass_triples,
    force_guarded_pass,
};
use tracing::{Level, event};

pub struct PassPlanner;

impl PassPlanner {
    pub fn choose(hand: &Hand, ctx: &BotContext<'_>) -> Option<[Card; 3]> {
        if hand.len() < 3 {
            return None;
        }

        if ctx.features().pass_v2_enabled() {
            return choose_pass_v2(hand, ctx);
        }

        choose_pass_v1(hand, ctx)
    }
}

fn choose_pass_v1(hand: &Hand, ctx: &BotContext<'_>) -> Option<[Card; 3]> {
    let style = determine_style(ctx);
    let snapshot = snapshot_scores(ctx.scores);
    let passing_target = ctx.passing_direction.target(ctx.seat);
    let passing_to_trailing = passing_target == snapshot.max_player;
    let passing_to_leader = passing_target == snapshot.min_player;
    let my_score = ctx.scores.score(ctx.seat);

    let mut scored: Vec<(Card, i32)> = hand
        .iter()
        .copied()
        .map(|card| {
            let score = score_card(
                card,
                hand,
                ctx,
                style,
                passing_to_trailing,
                passing_to_leader,
                my_score,
                snapshot,
            );
            (card, score)
        })
        .collect();

    scored.sort_by(|(card_a, score_a), (card_b, score_b)| {
        score_b
            .cmp(score_a)
            .then_with(|| card_sort_key(*card_a).cmp(&card_sort_key(*card_b)))
    });

    let picks = [scored[0].0, scored[1].0, scored[2].0];
    Some(picks)
}

fn choose_pass_v2(hand: &Hand, ctx: &BotContext<'_>) -> Option<[Card; 3]> {
    if hand.len() < 3 {
        return None;
    }

    let direction_profile = DirectionProfile::from_direction(ctx.passing_direction);
    let belief = ctx.belief().map(|view| view.belief());
    let moon_estimate = ctx.moon_estimate();

    let score_input = PassScoreInput {
        seat: ctx.seat,
        hand,
        round: ctx.round,
        scores: ctx.scores,
        belief,
        weights: PassWeights::default(),
        direction: ctx.passing_direction,
        direction_profile,
        moon_estimate,
    };

    let candidates = enumerate_pass_triples(&score_input);
    if let Some(best) = candidates.first() {
        log_pass_decision(ctx, best, &candidates, moon_estimate);
        return Some(best.cards);
    }

    if let Some(forced) = force_guarded_pass(&score_input) {
        tracing::warn!(
            target: "hearts_bot::pass_decision",
            seat = ?ctx.seat,
            direction = ?ctx.passing_direction,
            reason = "guarded_override",
            message = "forcing guarded pass candidate"
        );
        log_pass_decision(ctx, &forced, std::slice::from_ref(&forced), moon_estimate);
        return Some(forced.cards);
    }

    tracing::warn!(
        target: "hearts_bot::pass_decision",
        seat = ?ctx.seat,
        direction = ?ctx.passing_direction,
        reason = "pass_v2_fallback",
        message = "no candidates after optimizer; using legacy heuristic"
    );

    let legacy = choose_pass_v1(hand, ctx)?;
    let fallback_candidate = PassCandidate {
        cards: legacy,
        score: 0.0,
        void_score: 0.0,
        liability_score: 0.0,
        moon_score: 0.0,
        synergy: 0.0,
        direction_bonus: 0.0,
        moon_liability_penalty: 0.0,
    };
    log_pass_decision(ctx, &fallback_candidate, &[], moon_estimate);
    Some(legacy)
}

fn log_pass_decision(
    ctx: &BotContext<'_>,
    best: &PassCandidate,
    candidates: &[PassCandidate],
    moon_estimate: MoonEstimate,
) {
    if !tracing::enabled!(Level::INFO) || !pass_logging_enabled() {
        return;
    }

    let belief_hash = ctx.belief().map(|view| view.summary_hash()).unwrap_or(0);
    let belief_present = ctx.belief().is_some();
    let moon_objective_label = match moon_estimate.objective {
        Objective::MyPointsPerHand => "pph",
        Objective::BlockShooter => "block_shooter",
    };

    let selected_cards: Vec<String> = best
        .cards
        .iter()
        .map(|card| format!("{:?}", card))
        .collect();

    let mut top_cards: Vec<Vec<String>> = Vec::new();
    let mut top_scores: Vec<f32> = Vec::new();
    let mut top_void_scores: Vec<f32> = Vec::new();
    let mut top_liability_scores: Vec<f32> = Vec::new();
    let mut top_moon_scores: Vec<f32> = Vec::new();
    let mut top_synergy: Vec<f32> = Vec::new();
    let mut top_direction: Vec<f32> = Vec::new();
    let mut top_moon_penalties: Vec<f32> = Vec::new();

    for candidate in candidates.iter().take(5) {
        top_cards.push(
            candidate
                .cards
                .iter()
                .map(|card| format!("{:?}", card))
                .collect(),
        );
        top_scores.push(candidate.score);
        top_void_scores.push(candidate.void_score);
        top_liability_scores.push(candidate.liability_score);
        top_moon_scores.push(candidate.moon_score);
        top_synergy.push(candidate.synergy);
        top_direction.push(candidate.direction_bonus);
        top_moon_penalties.push(candidate.moon_liability_penalty);
    }

    let telemetry = ctx.telemetry();
    let has_run_metadata = telemetry.is_some();
    let run_id = telemetry.map(|meta| meta.run_id).unwrap_or("");
    let hand_index = telemetry.map(|meta| meta.hand_index as i64).unwrap_or(-1);
    let permutation_index = telemetry
        .map(|meta| meta.permutation_index as i64)
        .unwrap_or(-1);

    event!(
        target: "hearts_bot::pass_decision",
        Level::INFO,
        has_run_metadata,
        run_id = %run_id,
        hand_index,
        permutation_index,
        seat = ?ctx.seat,
        direction = ?ctx.passing_direction,
        cards = ?selected_cards,
        total = best.score,
        void_total = best.void_score,
        liability_total = best.liability_score,
        moon_total = best.moon_score,
        synergy = best.synergy,
        direction_bonus = best.direction_bonus,
        moon_penalty = best.moon_liability_penalty,
        belief_present,
        belief_hash,
        moon_probability = moon_estimate.probability,
        moon_objective = %moon_objective_label,
        candidate_count = candidates.len(),
        top_cards = ?top_cards,
        top_scores = ?top_scores,
        top_void_scores = ?top_void_scores,
        top_liability_scores = ?top_liability_scores,
        top_moon_scores = ?top_moon_scores,
        top_synergy = ?top_synergy,
        top_direction = ?top_direction,
        top_moon_penalties = ?top_moon_penalties,
    );
}

fn pass_logging_enabled() -> bool {
    std::env::var("MDH_PASS_DETAILS")
        .map(|raw| matches!(raw.trim(), "1" | "true" | "TRUE" | "on" | "ON"))
        .unwrap_or(false)
}

#[allow(clippy::too_many_arguments)]
fn score_card(
    card: Card,
    hand: &Hand,
    ctx: &BotContext<'_>,
    style: BotStyle,
    passing_to_trailing: bool,
    passing_to_leader: bool,
    my_score: u32,
    snapshot: super::ScoreSnapshot,
) -> i32 {
    let mut score: i32 = 0;
    let suit_len = count_cards_in_suit(hand, card.suit);
    let rank_value = card.rank.value() as i32;
    let card_penalty = card.penalty_value() as i32;

    if card.is_queen_of_spades() {
        score += ctx.params.pass_queen_spades;
    }

    if card.suit == Suit::Spades && !matches!(style, BotStyle::AggressiveMoon) {
        match card.rank {
            Rank::Ace => score += ctx.params.pass_ace_spades,
            Rank::King => score += ctx.params.pass_king_spades,
            Rank::Queen => score += ctx.params.pass_queen_spades,
            Rank::Jack => score += ctx.params.pass_jack_spades,
            _ => {}
        }
    }

    if card.suit == Suit::Hearts {
        score += ctx.params.pass_hearts_base + rank_value * ctx.params.pass_hearts_rank_mult;
    } else if rank_value >= Rank::King.value() as i32 {
        score +=
            ctx.params.pass_high_cards_base + rank_value * ctx.params.pass_high_cards_rank_mult;
    }

    if suit_len <= 2 {
        score += ctx.params.pass_void_creation_base
            - (suit_len as i32 * ctx.params.pass_void_creation_mult);
    } else if suit_len >= 5 {
        score -= (suit_len as i32 - 4) * ctx.params.pass_long_suit_penalty;
    }

    if passing_to_trailing {
        score += card_penalty * ctx.params.pass_to_trailing_mult;
    }

    if passing_to_leader {
        score += card_penalty * ctx.params.pass_to_leader_mult;
    }

    if my_score >= 75 {
        score += card_penalty * ctx.params.pass_desperate_mult;
    }

    if ctx.tracker.is_unseen(card) {
        score += ctx.params.pass_unseen_bonus;
    }

    let two_of_clubs = Card::new(Rank::Two, Suit::Clubs);
    if card == two_of_clubs {
        score += ctx.params.pass_two_clubs_penalty;
    }

    // Style adjustments
    match style {
        BotStyle::AggressiveMoon => {
            if card.suit == Suit::Hearts {
                score += ctx.params.pass_moon_keep_hearts;
            }
            if card.is_queen_of_spades() {
                score += ctx.params.pass_moon_keep_queen;
            }
            if card.suit == Suit::Spades && card.rank >= Rank::Queen {
                score += ctx.params.pass_moon_keep_spades;
            }
            if suit_len == 1 && card.suit != Suit::Hearts {
                score += ctx.params.pass_moon_void_bonus;
            }
        }
        BotStyle::HuntLeader => {
            if card_penalty > 0 {
                score += ctx.params.pass_hunt_penalty_mult * card_penalty;
                if passing_to_trailing {
                    score += ctx.params.pass_hunt_trailing_mult * card_penalty;
                }
            }
        }
        BotStyle::Cautious => {}
    }

    // Late-round adjustment to shed high cards.
    let cards_played = ctx.cards_played() as i32;
    score += cards_played * ctx.params.pass_cards_played_mult;

    // Bias towards discarding the very highest ranks when we are well ahead.
    if snapshot.min_player == ctx.seat && snapshot.max_score - snapshot.min_score >= 15 {
        score += rank_value * ctx.params.pass_leader_rank_mult;
    }

    score
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::BotFeatures;
    use crate::bot::tracker::UnseenTracker;
    use crate::bot::{BeliefView, BotContext, BotDifficulty, BotParams};
    use hearts_core::belief::Belief;
    use hearts_core::model::card::Card;
    use hearts_core::model::hand::Hand;
    use hearts_core::model::passing::{PassingDirection, PassingState};
    use hearts_core::model::player::PlayerPosition;
    use hearts_core::model::rank::Rank;
    use hearts_core::model::round::{RoundPhase, RoundState};
    use hearts_core::model::score::ScoreBoard;
    use hearts_core::model::suit::Suit;
    use std::sync::Mutex;

    static PASS_ENV_GUARD: Mutex<()> = Mutex::new(());

    fn build_round(
        seat: PlayerPosition,
        hand_cards: &[Card],
        passing_direction: PassingDirection,
    ) -> RoundState {
        let mut hands = [Hand::new(), Hand::new(), Hand::new(), Hand::new()];
        hands[seat.index()] = Hand::with_cards(hand_cards.to_vec());
        let phase = RoundPhase::Passing(PassingState::new(passing_direction));
        RoundState::from_hands(hands, seat, passing_direction, phase)
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

    #[test]
    fn pass_logging_disabled_without_env() {
        let _guard = PASS_ENV_GUARD.lock().unwrap();
        unsafe {
            std::env::remove_var("MDH_PASS_DETAILS");
        }
        assert!(!super::pass_logging_enabled());
    }

    #[test]
    fn pass_logging_enabled_with_env_flag() {
        let _guard = PASS_ENV_GUARD.lock().unwrap();
        unsafe {
            std::env::set_var("MDH_PASS_DETAILS", "true");
        }
        assert!(super::pass_logging_enabled());
        unsafe {
            std::env::remove_var("MDH_PASS_DETAILS");
        }
    }

    #[test]
    fn cautious_pass_drops_queen_of_spades() {
        let seat = PlayerPosition::North;
        let passing = PassingDirection::Left;
        let hand = vec![
            Card::new(Rank::Queen, Suit::Spades),
            Card::new(Rank::Ace, Suit::Hearts),
            Card::new(Rank::King, Suit::Hearts),
            Card::new(Rank::Ten, Suit::Hearts),
            Card::new(Rank::Five, Suit::Spades),
            Card::new(Rank::Six, Suit::Clubs),
            Card::new(Rank::Seven, Suit::Clubs),
            Card::new(Rank::Eight, Suit::Clubs),
            Card::new(Rank::Nine, Suit::Clubs),
            Card::new(Rank::Two, Suit::Diamonds),
            Card::new(Rank::Three, Suit::Diamonds),
            Card::new(Rank::Four, Suit::Diamonds),
            Card::new(Rank::Five, Suit::Diamonds),
        ];
        let round = build_round(seat, &hand, passing);
        let scores = build_scores([20, 10, 30, 25]);
        let mut tracker = UnseenTracker::new();
        tracker.reset_for_round(&round);
        let params = BotParams::default();
        let ctx = BotContext::new(
            seat,
            &round,
            &scores,
            passing,
            &tracker,
            None,
            BotFeatures::default(),
            BotDifficulty::NormalHeuristic,
            &params,
            None,
        );

        let picks = PassPlanner::choose(round.hand(seat), &ctx).unwrap();
        assert!(picks.contains(&Card::new(Rank::Queen, Suit::Spades)));
    }

    #[test]
    fn aggressive_pass_keeps_control_cards() {
        let seat = PlayerPosition::South;
        let passing = PassingDirection::Right;
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
            Card::new(Rank::Four, Suit::Clubs),
            Card::new(Rank::Five, Suit::Clubs),
        ];
        let round = build_round(seat, &hand, passing);
        let scores = build_scores([35, 36, 40, 38]);
        let mut tracker = UnseenTracker::new();
        tracker.reset_for_round(&round);
        let params = BotParams::default();
        let ctx = BotContext::new(
            seat,
            &round,
            &scores,
            passing,
            &tracker,
            None,
            BotFeatures::default(),
            BotDifficulty::NormalHeuristic,
            &params,
            None,
        );

        let picks = PassPlanner::choose(round.hand(seat), &ctx).unwrap();
        for card in picks.iter() {
            assert_ne!(
                card.suit,
                Suit::Hearts,
                "should keep hearts when attempting moon"
            );
            assert!(
                !(card.suit == Suit::Spades
                    && (*card == Card::new(Rank::Ace, Suit::Spades)
                        || *card == Card::new(Rank::King, Suit::Spades))),
                "should retain spade control cards"
            );
        }
    }

    #[test]
    fn pass_prefers_void_creation_for_short_suit() {
        let seat = PlayerPosition::East;
        let passing = PassingDirection::Across;
        let hand = vec![
            Card::new(Rank::Two, Suit::Clubs),
            Card::new(Rank::Five, Suit::Clubs),
            Card::new(Rank::Four, Suit::Diamonds),
            Card::new(Rank::Six, Suit::Diamonds),
            Card::new(Rank::Eight, Suit::Diamonds),
            Card::new(Rank::Ten, Suit::Diamonds),
            Card::new(Rank::Three, Suit::Spades),
            Card::new(Rank::Four, Suit::Spades),
            Card::new(Rank::Five, Suit::Spades),
            Card::new(Rank::Seven, Suit::Spades),
            Card::new(Rank::Nine, Suit::Spades),
            Card::new(Rank::Jack, Suit::Spades),
            Card::new(Rank::Three, Suit::Hearts),
        ];
        let round = build_round(seat, &hand, passing);
        let scores = build_scores([15, 22, 18, 20]);
        let mut tracker = UnseenTracker::new();
        tracker.reset_for_round(&round);
        let params = BotParams::default();
        let ctx = BotContext::new(
            seat,
            &round,
            &scores,
            passing,
            &tracker,
            None,
            BotFeatures::default(),
            BotDifficulty::NormalHeuristic,
            &params,
            None,
        );

        let picks = PassPlanner::choose(round.hand(seat), &ctx).unwrap();
        let club_passes = picks.iter().filter(|card| card.suit == Suit::Clubs).count();
        assert!(club_passes >= 1, "picks {:?}", picks);
    }

    #[test]
    fn pass_v2_prefers_high_liability_cards() {
        let seat = PlayerPosition::East;
        let passing = PassingDirection::Left;
        let hand = vec![
            Card::new(Rank::Queen, Suit::Spades),
            Card::new(Rank::Ace, Suit::Hearts),
            Card::new(Rank::King, Suit::Spades),
            Card::new(Rank::Two, Suit::Clubs),
            Card::new(Rank::Three, Suit::Clubs),
            Card::new(Rank::Four, Suit::Diamonds),
            Card::new(Rank::Five, Suit::Diamonds),
        ];
        let round = build_round(seat, &hand, passing);
        let belief = Belief::from_state(&round, seat);
        let scores = build_scores([0, 0, 0, 0]);
        let mut tracker = UnseenTracker::new();
        tracker.reset_for_round(&round);
        let params = BotParams::default();
        let features = BotFeatures::default().with_pass_v2(true);
        let belief_view = BeliefView::new(&belief, features.void_threshold());
        let ctx = BotContext::new(
            seat,
            &round,
            &scores,
            passing,
            &tracker,
            Some(belief_view),
            features,
            BotDifficulty::NormalHeuristic,
            &params,
            None,
        );

        let picks = PassPlanner::choose(round.hand(seat), &ctx).unwrap();
        assert!(picks.contains(&Card::new(Rank::Queen, Suit::Spades)));
        assert!(picks.contains(&Card::new(Rank::King, Suit::Spades)));
        assert!(
            !picks.contains(&Card::new(Rank::Ace, Suit::Hearts)),
            "expected A♥ to be retained, picks={picks:?}"
        );
    }

    #[test]
    fn pass_v2_without_belief_still_targets_penalties() {
        let seat = PlayerPosition::East;
        let passing = PassingDirection::Left;
        let hand = vec![
            Card::new(Rank::Queen, Suit::Spades),
            Card::new(Rank::Ace, Suit::Hearts),
            Card::new(Rank::King, Suit::Spades),
            Card::new(Rank::Two, Suit::Clubs),
            Card::new(Rank::Three, Suit::Clubs),
            Card::new(Rank::Four, Suit::Diamonds),
            Card::new(Rank::Five, Suit::Diamonds),
        ];
        let round = build_round(seat, &hand, passing);
        let scores = build_scores([0, 0, 0, 0]);
        let mut tracker = UnseenTracker::new();
        tracker.reset_for_round(&round);
        let params = BotParams::default();
        let features = BotFeatures::default().with_pass_v2(true);
        let ctx = BotContext::new(
            seat,
            &round,
            &scores,
            passing,
            &tracker,
            None,
            features,
            BotDifficulty::NormalHeuristic,
            &params,
            None,
        );

        let picks = PassPlanner::choose(round.hand(seat), &ctx).unwrap();
        assert!(picks.contains(&Card::new(Rank::Queen, Suit::Spades)));
        assert!(picks.contains(&Card::new(Rank::King, Suit::Spades)));
        assert!(
            !picks.contains(&Card::new(Rank::Ace, Suit::Hearts)),
            "expected A♥ to be retained, picks={picks:?}"
        );
    }
    #[test]
    fn pass_tracker_respects_seen_queen() {
        let seat = PlayerPosition::North;
        let passing = PassingDirection::Left;
        let hand = vec![
            Card::new(Rank::Queen, Suit::Spades),
            Card::new(Rank::Ace, Suit::Hearts),
            Card::new(Rank::King, Suit::Hearts),
            Card::new(Rank::Ten, Suit::Hearts),
            Card::new(Rank::Five, Suit::Spades),
            Card::new(Rank::Six, Suit::Clubs),
            Card::new(Rank::Seven, Suit::Clubs),
            Card::new(Rank::Eight, Suit::Clubs),
            Card::new(Rank::Nine, Suit::Clubs),
            Card::new(Rank::Two, Suit::Diamonds),
            Card::new(Rank::Three, Suit::Diamonds),
            Card::new(Rank::Four, Suit::Diamonds),
            Card::new(Rank::Five, Suit::Diamonds),
        ];
        let round = build_round(seat, &hand, passing);
        let scores = build_scores([20, 10, 30, 25]);

        let mut tracker_unseen = UnseenTracker::new();
        tracker_unseen.reset_for_round(&round);
        let params = BotParams::default();
        let ctx_unseen = BotContext::new(
            seat,
            &round,
            &scores,
            passing,
            &tracker_unseen,
            None,
            BotFeatures::default(),
            BotDifficulty::NormalHeuristic,
            &params,
            None,
        );
        let style = determine_style(&ctx_unseen);
        let snapshot = snapshot_scores(ctx_unseen.scores);
        let passing_target = ctx_unseen.passing_direction.target(ctx_unseen.seat);
        let passing_to_trailing = passing_target == snapshot.max_player;
        let passing_to_leader = passing_target == snapshot.min_player;
        let my_score = ctx_unseen.scores.score(ctx_unseen.seat);
        let hand_ref = round.hand(seat);
        let queen = Card::new(Rank::Queen, Suit::Spades);

        let score_unseen = super::score_card(
            queen,
            hand_ref,
            &ctx_unseen,
            style,
            passing_to_trailing,
            passing_to_leader,
            my_score,
            snapshot,
        );

        let mut tracker_seen = tracker_unseen.clone();
        tracker_seen.note_card_revealed(queen);
        let ctx_seen = BotContext::new(
            seat,
            &round,
            &scores,
            passing,
            &tracker_seen,
            None,
            BotFeatures::default(),
            BotDifficulty::NormalHeuristic,
            &params,
            None,
        );
        let snapshot_seen = snapshot_scores(ctx_seen.scores);
        let score_seen = super::score_card(
            queen,
            hand_ref,
            &ctx_seen,
            style,
            passing_to_trailing,
            passing_to_leader,
            my_score,
            snapshot_seen,
        );

        assert!(score_seen < score_unseen);
    }
}
