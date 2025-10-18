use super::{
    BotContext, BotStyle, Objective, card_sort_key, count_cards_in_suit, determine_style,
    snapshot_scores,
};
use hearts_core::model::card::Card;
use hearts_core::model::player::PlayerPosition;
use hearts_core::model::round::{PlayOutcome, RoundState};
use hearts_core::model::suit::Suit;
use std::borrow::Cow;
use std::cmp::Ordering;

pub struct PlayPlanner;

impl PlayPlanner {
    pub fn choose(legal: &[Card], ctx: &BotContext<'_>) -> Option<Card> {
        if legal.is_empty() {
            return None;
        }

        let style = determine_style(ctx);
        let snapshot = snapshot_scores(ctx.scores);
        let lead_suit = ctx.round.current_trick().lead_suit();
        let objective = ctx.objective_hint();
        let mut candidates: Cow<'_, [Card]> = Cow::Borrowed(legal);
        if ctx.round.is_first_trick() && lead_suit == Some(Suit::Clubs) {
            let only_hearts = ctx.hand().iter().all(|card| card.suit.is_heart());
            if !only_hearts {
                let filtered: Vec<Card> = candidates
                    .iter()
                    .copied()
                    .filter(|card| !card.is_queen_of_spades() && !card.suit.is_heart())
                    .collect();
                if !filtered.is_empty() {
                    candidates = Cow::Owned(filtered);
                }
            }
        }

        // Compute void inference - use the tracker's knowledge!
        let voids = ctx.void_matrix();

        let mut best: Option<(Card, i32)> = None;

        for &card in candidates.as_ref() {
            let (winner, penalties) = simulate_trick(card, ctx, style, &voids);
            let will_capture = winner == ctx.seat;
            let mut score = base_score(
                ctx,
                card,
                winner,
                will_capture,
                penalties,
                lead_suit,
                style,
                &snapshot,
                objective,
            );

            // Void creation bonus.
            let suit_remaining = count_cards_in_suit(ctx.hand(), card.suit);
            if suit_remaining <= 1 {
                score += ctx.params.play_void_creation_bonus;
            }

            // Prefer dumping high cards when following suit.
            if let Some(lead) = lead_suit {
                if card.suit == lead {
                    score += (card.rank.value() as i32) * ctx.params.play_follow_rank_mult;
                } else {
                    score += card.penalty_value() as i32 * ctx.params.play_slough_penalty_mult;
                }
            } else {
                // We are leading.
                score += (card.rank.value() as i32) * ctx.params.play_lead_rank_mult;
                if card.suit == Suit::Hearts
                    && !ctx.round.hearts_broken()
                    && style != BotStyle::HuntLeader
                {
                    score += ctx.params.play_break_hearts_penalty;
                }
                if style == BotStyle::HuntLeader && card.penalty_value() > 0 {
                    score += ctx.params.play_hunt_lead_penalty_base
                        + (card.penalty_value() as i32 * ctx.params.play_hunt_lead_penalty_mult);
                }
                if style == BotStyle::AggressiveMoon && card.suit == Suit::Hearts {
                    score += ctx.params.play_moon_lead_hearts;
                }
            }

            // Late-round urgency to shed penalties if we are at risk.
            if snapshot.max_player == ctx.seat && snapshot.max_score >= 90 {
                if will_capture {
                    score += penalties as i32 * ctx.params.play_desperate_take_mult;
                } else {
                    score += penalties as i32 * ctx.params.play_desperate_dump_mult;
                }
            }

            // Tracker-based pacing: fewer unseen cards => accelerate shedding points.
            let cards_played = ctx.cards_played() as i32;
            score += cards_played * ctx.params.play_cards_played_mult;
            if ctx.tracker.is_unseen(card) {
                score += ctx.params.play_unseen_bonus;
            }

            match best {
                None => best = Some((card, score)),
                Some((best_card, best_score)) => {
                    if score > best_score
                        || (score == best_score
                            && card_sort_key(card).cmp(&card_sort_key(best_card)) == Ordering::Less)
                    {
                        best = Some((card, score));
                    }
                }
            }
        }

        best.map(|(card, _)| card)
    }
}

#[cfg(test)]
pub(crate) fn score_candidate_for_tests(card: Card, ctx: &BotContext<'_>, style: BotStyle) -> i32 {
    let snapshot = snapshot_scores(ctx.scores);
    let lead_suit = ctx.round.current_trick().lead_suit();
    let voids = ctx.void_matrix();
    let (winner, penalties) = simulate_trick(card, ctx, style, &voids);
    let will_capture = winner == ctx.seat;
    let objective = ctx.objective_hint();
    let mut score = base_score(
        ctx,
        card,
        winner,
        will_capture,
        penalties,
        lead_suit,
        style,
        &snapshot,
        objective,
    );

    let suit_remaining = count_cards_in_suit(ctx.hand(), card.suit);
    if suit_remaining <= 1 {
        score += ctx.params.play_void_creation_bonus;
    }

    if let Some(lead) = lead_suit {
        if card.suit == lead {
            score += (card.rank.value() as i32) * ctx.params.play_follow_rank_mult;
        } else {
            score += card.penalty_value() as i32 * ctx.params.play_slough_penalty_mult;
        }
    } else {
        score += (card.rank.value() as i32) * ctx.params.play_lead_rank_mult;
        if card.suit == Suit::Hearts && !ctx.round.hearts_broken() && style != BotStyle::HuntLeader
        {
            score += ctx.params.play_break_hearts_penalty;
        }
        if style == BotStyle::HuntLeader && card.penalty_value() > 0 {
            score += ctx.params.play_hunt_lead_penalty_base
                + (card.penalty_value() as i32 * ctx.params.play_hunt_lead_penalty_mult);
        }
        if style == BotStyle::AggressiveMoon && card.suit == Suit::Hearts {
            score += ctx.params.play_moon_lead_hearts;
        }
    }

    if snapshot.max_player == ctx.seat && snapshot.max_score >= 90 {
        if will_capture {
            score += penalties as i32 * ctx.params.play_desperate_take_mult;
        } else {
            score += penalties as i32 * ctx.params.play_desperate_dump_mult;
        }
    }

    let cards_played = ctx.cards_played() as i32;
    score += cards_played * ctx.params.play_cards_played_mult;
    if ctx.tracker.is_unseen(card) {
        score += ctx.params.play_unseen_bonus;
    }

    score
}

#[allow(clippy::too_many_arguments)]
fn base_score(
    ctx: &BotContext<'_>,
    card: Card,
    winner: PlayerPosition,
    will_capture: bool,
    penalties: u8,
    lead_suit: Option<Suit>,
    style: BotStyle,
    snapshot: &super::ScoreSnapshot,
    objective: Objective,
) -> i32 {
    let penalties_i32 = penalties as i32;
    let mut score: i32 = 0;

    if will_capture {
        score += ctx.params.play_take_trick_penalty;
        score += penalties_i32 * ctx.params.play_take_points_mult;
    } else {
        score += ctx.params.play_avoid_trick_reward;
        score += penalties_i32 * ctx.params.play_dump_points_mult;
    }

    if penalties == 0 && will_capture {
        // Winning a clean trick is still mildly negative to keep low profile.
        score += (card.rank.value() as i32) * ctx.params.play_clean_trick_rank_mult;
    }

    if let Some(lead) = lead_suit
        && card.suit != lead
        && !ctx.round.current_trick().plays().is_empty()
    {
        score += 200;
    }

    match style {
        BotStyle::AggressiveMoon => {
            if will_capture {
                score += ctx.params.play_moon_take_trick
                    + penalties_i32 * ctx.params.play_moon_take_points_mult;
            } else {
                score += penalties_i32 * ctx.params.play_moon_avoid_points_mult;
            }
        }
        BotStyle::HuntLeader => {
            if !will_capture && penalties > 0 && winner == snapshot.max_player {
                score += penalties_i32 * ctx.params.play_hunt_feed_leader_mult;
            }
            if will_capture {
                score += ctx.params.play_hunt_avoid_trick;
            }
        }
        BotStyle::Cautious => {}
    }

    if matches!(objective, Objective::BlockShooter) {
        if will_capture {
            score += 5500;
            score += penalties_i32 * 1400;
            if let Some(lead) = lead_suit {
                if lead == Suit::Hearts {
                    score += 1500;
                }
            } else if card.suit == Suit::Hearts {
                score += 1200;
            }
            if card.is_queen_of_spades() {
                score += 2000;
            }
        } else {
            score -= 2800;
            if penalties > 0 {
                score -= 2500;
            }
        }
    }

    score
}

fn simulate_trick(
    card: Card,
    ctx: &BotContext<'_>,
    style: BotStyle,
    voids: &[[bool; 4]; 4],
) -> (PlayerPosition, u8) {
    let mut sim = ctx.round.clone();
    let seat = ctx.seat;
    let mut outcome = match sim.play_card(seat, card) {
        Ok(result) => result,
        Err(_) => return (seat, 0),
    };

    while !matches!(outcome, PlayOutcome::TrickCompleted { .. }) {
        let next_seat = next_to_play(&sim);
        let response = choose_followup_card(&sim, next_seat, style, voids);
        outcome = match sim.play_card(next_seat, response) {
            Ok(result) => result,
            Err(_) => break,
        };
    }

    match outcome {
        PlayOutcome::TrickCompleted { winner, penalties } => (winner, penalties),
        _ => (seat, 0),
    }
}

fn next_to_play(round: &RoundState) -> PlayerPosition {
    let trick = round.current_trick();
    trick
        .plays()
        .last()
        .map(|play| play.position.next())
        .unwrap_or(trick.leader())
}

fn choose_followup_card(
    round: &RoundState,
    seat: PlayerPosition,
    _style: BotStyle,
    voids: &[[bool; 4]; 4],
) -> Card {
    let legal = legal_moves_for(round, seat);
    if legal.is_empty() {
        if let Some(card) = round.hand(seat).iter().copied().next() {
            return card;
        }
        panic!("simulation expected at least one legal card");
    }
    let lead_suit = round.current_trick().lead_suit();

    if let Some(lead) = lead_suit {
        // Check if this player is void in the led suit
        let is_void = voids[seat.index()][lead as usize];

        if !is_void {
            // Player can follow suit - play lowest card in suit
            if let Some(card) = legal
                .iter()
                .copied()
                .filter(|c| c.suit == lead)
                .min_by_key(|card| card.rank.value())
            {
                return card;
            }
        }
        // If void (or couldn't find card in suit), fall through to dump logic
    }

    // Dump highest penalty card
    legal
        .into_iter()
        .max_by(|a, b| compare_penalty_dump(*a, *b))
        .expect("at least one legal card")
}

fn legal_moves_for(round: &RoundState, seat: PlayerPosition) -> Vec<Card> {
    round
        .hand(seat)
        .iter()
        .copied()
        .filter(|card| {
            let mut probe = round.clone();
            probe.play_card(seat, *card).is_ok()
        })
        .collect()
}

fn compare_penalty_dump(a: Card, b: Card) -> Ordering {
    let weight = |card: Card| -> (i32, i32) {
        let penalty = card.penalty_value() as i32;
        let rank = card.rank.value() as i32;
        (penalty * 100 + rank, rank)
    };
    weight(a).cmp(&weight(b))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::BotFeatures;
    use crate::bot::tracker::UnseenTracker;
    use crate::bot::{BotContext, BotDifficulty, BotParams};
    use hearts_core::model::card::Card;
    use hearts_core::model::hand::Hand;
    use hearts_core::model::passing::PassingDirection;
    use hearts_core::model::player::PlayerPosition;
    use hearts_core::model::rank::Rank;
    use hearts_core::model::round::{RoundPhase, RoundState};
    use hearts_core::model::score::ScoreBoard;
    use hearts_core::model::suit::Suit;
    use hearts_core::moon::MoonEstimate;

    fn build_round(
        starting: PlayerPosition,
        hands_vec: [Vec<Card>; 4],
        plays: &[(PlayerPosition, Card)],
        hearts_broken: bool,
    ) -> RoundState {
        let mut hands = [Hand::new(), Hand::new(), Hand::new(), Hand::new()];
        for (idx, cards) in hands_vec.into_iter().enumerate() {
            hands[idx] = Hand::with_cards(cards);
        }
        let mut seed_trick = hearts_core::model::trick::Trick::new(starting);
        let mut seat_iter = starting;
        let seed_cards = [
            Card::new(Rank::Two, Suit::Clubs),
            Card::new(Rank::Three, Suit::Clubs),
            Card::new(Rank::Four, Suit::Clubs),
            Card::new(Rank::Five, Suit::Clubs),
        ];
        for card in seed_cards {
            seed_trick.play(seat_iter, card).unwrap();
            seat_iter = seat_iter.next();
        }
        let mut current_trick = hearts_core::model::trick::Trick::new(starting);
        for &(seat, card) in plays {
            current_trick.play(seat, card).unwrap();
        }
        RoundState::from_hands_with_state(
            hands,
            starting,
            PassingDirection::Hold,
            RoundPhase::Playing,
            current_trick,
            vec![seed_trick],
            hearts_broken,
        )
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

    fn make_ctx<'a>(
        seat: PlayerPosition,
        round: &'a RoundState,
        scores: &'a ScoreBoard,
        tracker: &'a UnseenTracker,
        difficulty: BotDifficulty,
        params: &'a BotParams,
    ) -> BotContext<'a> {
        BotContext::new(
            seat,
            round,
            scores,
            PassingDirection::Hold,
            tracker,
            None,
            BotFeatures::default(),
            difficulty,
            params,
        )
    }

    fn block_objective_hand() -> Vec<Card> {
        vec![
            Card::new(Rank::Ace, Suit::Hearts),
            Card::new(Rank::King, Suit::Hearts),
            Card::new(Rank::Queen, Suit::Hearts),
            Card::new(Rank::Jack, Suit::Hearts),
            Card::new(Rank::Ten, Suit::Hearts),
            Card::new(Rank::Nine, Suit::Hearts),
            Card::new(Rank::Eight, Suit::Hearts),
            Card::new(Rank::Seven, Suit::Hearts),
            Card::new(Rank::Six, Suit::Hearts),
            Card::new(Rank::Five, Suit::Hearts),
            Card::new(Rank::Four, Suit::Clubs),
            Card::new(Rank::Three, Suit::Clubs),
            Card::new(Rank::Two, Suit::Clubs),
        ]
    }

    fn cautious_objective_hand() -> Vec<Card> {
        vec![
            Card::new(Rank::Queen, Suit::Hearts),
            Card::new(Rank::Seven, Suit::Hearts),
            Card::new(Rank::Ace, Suit::Clubs),
            Card::new(Rank::King, Suit::Clubs),
            Card::new(Rank::Jack, Suit::Clubs),
            Card::new(Rank::Ten, Suit::Clubs),
            Card::new(Rank::Nine, Suit::Spades),
            Card::new(Rank::Eight, Suit::Diamonds),
            Card::new(Rank::Six, Suit::Clubs),
            Card::new(Rank::Five, Suit::Diamonds),
            Card::new(Rank::Four, Suit::Spades),
            Card::new(Rank::Three, Suit::Clubs),
            Card::new(Rank::Two, Suit::Clubs),
        ]
    }

    #[test]
    fn block_objective_increases_capture_score() {
        let seat = PlayerPosition::South;
        let heart = Card::new(Rank::Queen, Suit::Hearts);

        let mut params = BotParams::default();
        params.play_take_trick_penalty = -600;
        params.play_take_points_mult = -80;
        params.play_avoid_trick_reward = 0;
        params.play_dump_points_mult = 0;
        params.play_void_creation_bonus = 0;
        params.play_break_hearts_penalty = 0;
        params.play_lead_rank_mult = 0;

        let round_block = build_round(
            PlayerPosition::North,
            [
                vec![Card::new(Rank::Two, Suit::Spades)],
                vec![Card::new(Rank::Three, Suit::Diamonds)],
                block_objective_hand(),
                vec![Card::new(Rank::Four, Suit::Clubs)],
            ],
            &[],
            false,
        );
        let scores_block = build_scores([20, 18, 21, 25]);
        let mut tracker_block = UnseenTracker::new();
        tracker_block.reset_for_round(&round_block);
        let mut ctx_block = make_ctx(
            seat,
            &round_block,
            &scores_block,
            &tracker_block,
            BotDifficulty::NormalHeuristic,
            &params,
        );
        ctx_block.moon_estimate = MoonEstimate {
            probability: 0.85,
            raw_score: 2.4,
            objective: Objective::BlockShooter,
        };

        let round_cautious = build_round(
            PlayerPosition::North,
            [
                vec![Card::new(Rank::Five, Suit::Spades)],
                vec![Card::new(Rank::Six, Suit::Diamonds)],
                cautious_objective_hand(),
                vec![Card::new(Rank::Seven, Suit::Clubs)],
            ],
            &[],
            false,
        );
        let scores_cautious = build_scores([12, 14, 10, 16]);
        let mut tracker_cautious = UnseenTracker::new();
        tracker_cautious.reset_for_round(&round_cautious);
        let mut ctx_cautious = make_ctx(
            seat,
            &round_cautious,
            &scores_cautious,
            &tracker_cautious,
            BotDifficulty::NormalHeuristic,
            &params,
        );
        ctx_cautious.moon_estimate = MoonEstimate {
            probability: 0.1,
            raw_score: -1.0,
            objective: Objective::MyPointsPerHand,
        };

        let score_block = super::score_candidate_for_tests(heart, &ctx_block, BotStyle::Cautious);
        let score_cautious =
            super::score_candidate_for_tests(heart, &ctx_cautious, BotStyle::Cautious);

        assert!(
            score_block > score_cautious,
            "expected block objective ({score_block}) to outscore cautious objective ({score_cautious}) for capturing heart"
        );
    }

    #[test]
    fn block_objective_prefers_capturing_move() {
        let seat = PlayerPosition::South;
        let heart = Card::new(Rank::Queen, Suit::Hearts);
        let dump = Card::new(Rank::Two, Suit::Clubs);

        let mut params = BotParams::default();
        params.play_take_trick_penalty = -600;
        params.play_take_points_mult = -80;
        params.play_avoid_trick_reward = 0;
        params.play_dump_points_mult = 0;
        params.play_void_creation_bonus = 0;
        params.play_break_hearts_penalty = 0;
        params.play_lead_rank_mult = 0;

        let round_block = build_round(
            PlayerPosition::North,
            [
                vec![Card::new(Rank::Ten, Suit::Spades)],
                vec![Card::new(Rank::Three, Suit::Diamonds)],
                block_objective_hand(),
                vec![Card::new(Rank::Four, Suit::Clubs)],
            ],
            &[],
            false,
        );
        let scores_block = build_scores([18, 19, 23, 24]);
        let mut tracker_block = UnseenTracker::new();
        tracker_block.reset_for_round(&round_block);
        let mut ctx_block = make_ctx(
            seat,
            &round_block,
            &scores_block,
            &tracker_block,
            BotDifficulty::NormalHeuristic,
            &params,
        );
        ctx_block.moon_estimate = MoonEstimate {
            probability: 0.85,
            raw_score: 2.4,
            objective: Objective::BlockShooter,
        };

        let legal = vec![heart, dump];
        let heart_score = super::score_candidate_for_tests(heart, &ctx_block, BotStyle::Cautious);
        let dump_score = super::score_candidate_for_tests(dump, &ctx_block, BotStyle::Cautious);
        assert!(
            heart_score > dump_score,
            "block objective scoring heart={} dump={}",
            heart_score,
            dump_score
        );

        let chosen_block = PlayPlanner::choose(&legal, &ctx_block).unwrap();
        assert_eq!(
            chosen_block, heart,
            "block shooter objective should choose to capture with {:?}",
            heart
        );

        let round_cautious = build_round(
            PlayerPosition::North,
            [
                vec![Card::new(Rank::Four, Suit::Spades)],
                vec![Card::new(Rank::Five, Suit::Diamonds)],
                cautious_objective_hand(),
                vec![Card::new(Rank::Six, Suit::Clubs)],
            ],
            &[],
            false,
        );
        let scores_cautious = build_scores([10, 12, 8, 16]);
        let mut tracker_cautious = UnseenTracker::new();
        tracker_cautious.reset_for_round(&round_cautious);
        let mut ctx_cautious = make_ctx(
            seat,
            &round_cautious,
            &scores_cautious,
            &tracker_cautious,
            BotDifficulty::NormalHeuristic,
            &params,
        );
        ctx_cautious.moon_estimate = MoonEstimate {
            probability: 0.05,
            raw_score: -1.5,
            objective: Objective::MyPointsPerHand,
        };

        let chosen_cautious = PlayPlanner::choose(&legal, &ctx_cautious).unwrap();
        assert_eq!(
            chosen_cautious, dump,
            "points objective should prefer dumping {:?}",
            dump
        );
    }

    #[test]
    fn cautious_dumps_points_when_safe() {
        let seat = PlayerPosition::South;
        let round = build_round(
            PlayerPosition::North,
            [
                vec![Card::new(Rank::King, Suit::Clubs)],
                vec![Card::new(Rank::Ace, Suit::Clubs)],
                vec![
                    Card::new(Rank::Queen, Suit::Hearts),
                    Card::new(Rank::Ten, Suit::Hearts),
                    Card::new(Rank::Three, Suit::Spades),
                ],
                vec![Card::new(Rank::Two, Suit::Clubs)],
            ],
            &[
                (PlayerPosition::North, Card::new(Rank::King, Suit::Clubs)),
                (PlayerPosition::East, Card::new(Rank::Ace, Suit::Clubs)),
            ],
            false,
        );

        let scores = build_scores([20, 18, 22, 19]);
        let mut tracker = UnseenTracker::new();
        tracker.reset_for_round(&round);
        let params = BotParams::default();
        let ctx = make_ctx(
            seat,
            &round,
            &scores,
            &tracker,
            BotDifficulty::NormalHeuristic,
            &params,
        );
        let legal = legal_moves_for(&round, seat);

        let choice = PlayPlanner::choose(&legal, &ctx).unwrap();
        assert!(legal.len() >= 2);
        assert_eq!(choice.suit, Suit::Hearts);
        assert!(choice.rank >= Rank::Ten);
        assert!(choice.penalty_value() > 0);
    }

    #[test]
    fn cautious_avoids_capturing_points_when_possible() {
        let seat = PlayerPosition::South;
        let round = build_round(
            PlayerPosition::North,
            [
                vec![Card::new(Rank::Ten, Suit::Hearts)],
                vec![Card::new(Rank::Queen, Suit::Hearts)],
                vec![
                    Card::new(Rank::King, Suit::Hearts),
                    Card::new(Rank::Two, Suit::Hearts),
                    Card::new(Rank::Four, Suit::Diamonds),
                ],
                vec![Card::new(Rank::Three, Suit::Hearts)],
            ],
            &[
                (PlayerPosition::North, Card::new(Rank::Ten, Suit::Hearts)),
                (PlayerPosition::East, Card::new(Rank::Queen, Suit::Hearts)),
            ],
            false,
        );

        let scores = build_scores([14, 25, 20, 22]);
        let mut tracker = UnseenTracker::new();
        tracker.reset_for_round(&round);
        let params = BotParams::default();
        let ctx = make_ctx(
            seat,
            &round,
            &scores,
            &tracker,
            BotDifficulty::NormalHeuristic,
            &params,
        );
        let legal = legal_moves_for(&round, seat);

        let choice = PlayPlanner::choose(&legal, &ctx).unwrap();
        assert_ne!(choice, Card::new(Rank::King, Suit::Hearts));
        assert!(choice.suit == Suit::Hearts);
    }

    #[test]
    fn aggressive_player_grabs_trick_for_moon_attempt() {
        let seat = PlayerPosition::South;
        let round = build_round(
            PlayerPosition::West,
            [
                vec![Card::new(Rank::Six, Suit::Hearts)],
                vec![Card::new(Rank::Four, Suit::Hearts)],
                vec![
                    Card::new(Rank::Ace, Suit::Hearts),
                    Card::new(Rank::King, Suit::Hearts),
                    Card::new(Rank::Queen, Suit::Hearts),
                    Card::new(Rank::Jack, Suit::Hearts),
                    Card::new(Rank::Ten, Suit::Hearts),
                    Card::new(Rank::Nine, Suit::Hearts),
                    Card::new(Rank::Eight, Suit::Hearts),
                    Card::new(Rank::Ace, Suit::Spades),
                    Card::new(Rank::King, Suit::Spades),
                ],
                vec![Card::new(Rank::Three, Suit::Hearts)],
            ],
            &[
                (PlayerPosition::West, Card::new(Rank::Three, Suit::Hearts)),
                (PlayerPosition::North, Card::new(Rank::Six, Suit::Hearts)),
                (PlayerPosition::East, Card::new(Rank::Four, Suit::Hearts)),
            ],
            false,
        );

        let scores = build_scores([32, 34, 38, 35]);
        let mut tracker = UnseenTracker::new();
        tracker.reset_for_round(&round);
        let params = BotParams::default();
        let ctx = make_ctx(
            seat,
            &round,
            &scores,
            &tracker,
            BotDifficulty::NormalHeuristic,
            &params,
        );
        let legal = legal_moves_for(&round, seat);

        let choice = PlayPlanner::choose(&legal, &ctx).unwrap();
        assert_eq!(choice.suit, Suit::Hearts);
        assert!(choice.rank.value() > Rank::Six.value());
    }

    #[test]
    fn play_tracker_considers_unseen() {
        let seat = PlayerPosition::South;
        let round = build_round(
            seat,
            [
                vec![Card::new(Rank::Two, Suit::Clubs)],
                vec![Card::new(Rank::Three, Suit::Diamonds)],
                vec![
                    Card::new(Rank::Seven, Suit::Hearts),
                    Card::new(Rank::Four, Suit::Clubs),
                ],
                vec![Card::new(Rank::Five, Suit::Spades)],
            ],
            &[],
            false,
        );
        let scores = build_scores([20, 18, 22, 16]);
        let mut tracker = UnseenTracker::new();
        tracker.reset_for_round(&round);
        let params = BotParams::default();
        let ctx_unseen = make_ctx(
            seat,
            &round,
            &scores,
            &tracker,
            BotDifficulty::NormalHeuristic,
            &params,
        );
        let style = determine_style(&ctx_unseen);
        let heart = Card::new(Rank::Seven, Suit::Hearts);

        let score_unseen = super::score_candidate_for_tests(heart, &ctx_unseen, style);

        let mut tracker_seen = tracker.clone();
        tracker_seen.note_card_revealed(heart);
        let ctx_seen = make_ctx(
            seat,
            &round,
            &scores,
            &tracker_seen,
            BotDifficulty::NormalHeuristic,
            &params,
        );
        let score_seen = super::score_candidate_for_tests(heart, &ctx_seen, style);

        assert!(score_unseen > score_seen);
    }

    #[test]
    fn play_lead_unbroken_hearts() {
        let seat = PlayerPosition::South;
        let round = build_round(
            seat,
            [
                vec![Card::new(Rank::Two, Suit::Clubs)],
                vec![Card::new(Rank::Three, Suit::Diamonds)],
                vec![
                    Card::new(Rank::Queen, Suit::Hearts),
                    Card::new(Rank::Four, Suit::Clubs),
                ],
                vec![Card::new(Rank::Six, Suit::Spades)],
            ],
            &[],
            false,
        );
        let scores = build_scores([20, 18, 24, 19]);
        let mut tracker = UnseenTracker::new();
        tracker.reset_for_round(&round);
        let params = BotParams::default();
        let ctx = make_ctx(
            seat,
            &round,
            &scores,
            &tracker,
            BotDifficulty::NormalHeuristic,
            &params,
        );
        let legal = legal_moves_for(&round, seat);
        let choice = PlayPlanner::choose(&legal, &ctx).unwrap();
        assert_ne!(choice.suit, Suit::Hearts);
    }

    #[test]
    fn play_lead_moon_mode() {
        let seat = PlayerPosition::South;
        let round = build_round(
            seat,
            [
                vec![Card::new(Rank::Two, Suit::Clubs)],
                vec![Card::new(Rank::Three, Suit::Diamonds)],
                vec![
                    Card::new(Rank::Ace, Suit::Hearts),
                    Card::new(Rank::King, Suit::Hearts),
                ],
                vec![Card::new(Rank::Six, Suit::Spades)],
            ],
            &[],
            false,
        );
        let scores = build_scores([18, 22, 20, 21]);
        let mut tracker = UnseenTracker::new();
        tracker.reset_for_round(&round);
        let params = BotParams::default();
        let ctx = make_ctx(
            seat,
            &round,
            &scores,
            &tracker,
            BotDifficulty::NormalHeuristic,
            &params,
        );
        let legal = legal_moves_for(&round, seat);
        let choice = PlayPlanner::choose(&legal, &ctx).unwrap();
        assert!(legal.len() >= 2);
        assert_eq!(choice.suit, Suit::Hearts);
        assert!(choice.rank >= Rank::King);
    }

    #[test]
    fn play_hunt_avoids_self_dump() {
        let seat = PlayerPosition::East;
        let round = build_round(
            PlayerPosition::North,
            [
                vec![Card::new(Rank::Ten, Suit::Hearts)],
                vec![
                    Card::new(Rank::Queen, Suit::Hearts),
                    Card::new(Rank::Two, Suit::Hearts),
                ],
                vec![Card::new(Rank::Five, Suit::Clubs)],
                vec![Card::new(Rank::Three, Suit::Hearts)],
            ],
            &[(PlayerPosition::North, Card::new(Rank::Ten, Suit::Hearts))],
            true,
        );
        let scores = build_scores([20, 18, 22, 95]);
        let mut tracker = UnseenTracker::new();
        tracker.reset_for_round(&round);
        let params = BotParams::default();
        let ctx = make_ctx(
            seat,
            &round,
            &scores,
            &tracker,
            BotDifficulty::NormalHeuristic,
            &params,
        );
        let legal = legal_moves_for(&round, seat);
        assert!(legal.contains(&Card::new(Rank::Queen, Suit::Hearts)));
        let choice = PlayPlanner::choose(&legal, &ctx).unwrap();
        assert_eq!(choice, Card::new(Rank::Two, Suit::Hearts));
    }

    #[test]
    fn hunt_leader_feeds_points_to_player_near_target() {
        let seat = PlayerPosition::East;
        let round = build_round(
            seat,
            [
                vec![Card::new(Rank::Ace, Suit::Hearts)],
                vec![
                    Card::new(Rank::Queen, Suit::Hearts),
                    Card::new(Rank::Two, Suit::Clubs),
                    Card::new(Rank::Four, Suit::Clubs),
                ],
                vec![Card::new(Rank::Ten, Suit::Hearts)],
                vec![Card::new(Rank::Five, Suit::Hearts)],
            ],
            &[],
            true,
        );
        let scores = build_scores([95, 40, 45, 60]);
        let mut tracker = UnseenTracker::new();
        tracker.reset_for_round(&round);
        let params = BotParams::default();
        let ctx = make_ctx(
            seat,
            &round,
            &scores,
            &tracker,
            BotDifficulty::NormalHeuristic,
            &params,
        );
        let legal = legal_moves_for(&round, seat);

        let choice = PlayPlanner::choose(&legal, &ctx).unwrap();
        assert_eq!(choice, Card::new(Rank::Queen, Suit::Hearts));
    }
}
