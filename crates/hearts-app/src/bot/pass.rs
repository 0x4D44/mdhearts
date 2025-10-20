use super::{
    BotContext, BotStyle, card_sort_key, count_cards_in_suit, determine_style, snapshot_scores,
};
use hearts_core::model::card::Card;
use hearts_core::model::hand::Hand;
use hearts_core::model::rank::Rank;
use hearts_core::model::suit::Suit;

pub struct PassPlanner;

impl PassPlanner {
    pub fn choose(hand: &Hand, ctx: &BotContext<'_>) -> Option<[Card; 3]> {
        if hand.len() < 3 {
            return None;
        }

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
        score += 18_000;
    }

    if card.suit == Suit::Spades && !matches!(style, BotStyle::AggressiveMoon) {
        match card.rank {
            Rank::Ace => score += 5_000,
            Rank::King => score += 7_000,
            Rank::Queen => score += 18_000,
            Rank::Jack => score += 2_500,
            _ => {}
        }
    }

    if card.suit == Suit::Hearts {
        score += 6_000 + rank_value * 120;
    } else if rank_value >= Rank::King.value() as i32 {
        score += 2_200 + rank_value * 80;
    }

    if suit_len <= 2 {
        score += 4_000 - (suit_len as i32 * 800);
    } else if suit_len >= 5 {
        score -= (suit_len as i32 - 4) * 400;
    }

    if passing_to_trailing {
        score += card_penalty * 1_400;
    }

    if passing_to_leader {
        score -= card_penalty * 1_200;
    }

    if my_score >= 75 {
        score += card_penalty * 1_600;
    }

    if ctx.tracker.is_unseen(card) {
        score += 90;
    }

    let two_of_clubs = Card::new(Rank::Two, Suit::Clubs);
    if card == two_of_clubs {
        score -= 4_000;
    }

    // Style adjustments
    match style {
        BotStyle::AggressiveMoon => {
            if card.suit == Suit::Hearts {
                score -= 9_000;
            }
            if card.is_queen_of_spades() {
                score -= 12_000;
            }
            if card.suit == Suit::Spades && card.rank >= Rank::Queen {
                score -= 9_000;
            }
            if suit_len == 1 && card.suit != Suit::Hearts {
                score += 2_500;
            }
        }
        BotStyle::HuntLeader => {
            if card_penalty > 0 {
                score += 900 * card_penalty;
                if passing_to_trailing {
                    score += 600 * card_penalty;
                }
            }
        }
        BotStyle::Cautious => {}
    }

    // Late-round adjustment to shed high cards.
    let cards_played = ctx.cards_played() as i32;
    score += cards_played * 12;

    // Bias towards discarding the very highest ranks when we are well ahead.
    if snapshot.min_player == ctx.seat && snapshot.max_score - snapshot.min_score >= 15 {
        score += rank_value * 40;
    }

    score
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::bot::tracker::UnseenTracker;
    use crate::bot::{BotContext, BotDifficulty};
    use hearts_core::model::card::Card;
    use hearts_core::model::hand::Hand;
    use hearts_core::model::passing::{PassingDirection, PassingState};
    use hearts_core::model::player::PlayerPosition;
    use hearts_core::model::rank::Rank;
    use hearts_core::model::round::{RoundPhase, RoundState};
    use hearts_core::model::score::ScoreBoard;
    use hearts_core::model::suit::Suit;

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
        let ctx = BotContext::new(
            seat,
            &round,
            &scores,
            passing,
            &tracker,
            BotDifficulty::NormalHeuristic,
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
        let ctx = BotContext::new(
            seat,
            &round,
            &scores,
            passing,
            &tracker,
            BotDifficulty::NormalHeuristic,
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
        let ctx = BotContext::new(
            seat,
            &round,
            &scores,
            passing,
            &tracker,
            BotDifficulty::NormalHeuristic,
        );

        let picks = PassPlanner::choose(round.hand(seat), &ctx).unwrap();
        let club_passes = picks.iter().filter(|card| card.suit == Suit::Clubs).count();
        assert!(club_passes >= 1, "picks {:?}", picks);
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
        let ctx_unseen = BotContext::new(
            seat,
            &round,
            &scores,
            passing,
            &tracker_unseen,
            BotDifficulty::NormalHeuristic,
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
            BotDifficulty::NormalHeuristic,
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
