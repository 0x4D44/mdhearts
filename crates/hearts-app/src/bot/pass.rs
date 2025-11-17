use super::{
    BotContext, BotStyle, card_sort_key, count_cards_in_suit, determine_style, snapshot_scores,
};
use hearts_core::model::card::Card;
use hearts_core::model::hand::Hand;
use hearts_core::model::rank::Rank;
use hearts_core::model::suit::Suit;
use std::cmp::Ordering;
use std::sync::OnceLock;

pub struct PassPlanner;

fn debug_enabled() -> bool {
    static ON: std::sync::OnceLock<bool> = std::sync::OnceLock::new();
    *ON.get_or_init(|| {
        std::env::var("MDH_DEBUG_LOGS")
            .map(|v| v == "1" || v.eq_ignore_ascii_case("true") || v.eq_ignore_ascii_case("on"))
            .unwrap_or(false)
    })
}

impl PassPlanner {
    pub fn choose(hand: &Hand, ctx: &BotContext<'_>) -> Option<[Card; 3]> {
        if hand.len() < 3 {
            return None;
        }

        let style = determine_style(ctx);
        let snapshot = snapshot_scores(&ctx.scores);
        let passing_target = ctx.passing_direction.target(ctx.seat);
        // In Hearts: low score = winning/leading, high score = losing/trailing
        // "leader" = person with lowest score (winning the game)
        // "trailing" = person with highest score (losing the game, closer to 100)
        let passing_to_trailing = passing_target == snapshot.max_player;
        let passing_to_leader = passing_target == snapshot.min_player;
        let my_score = ctx.scores.score(ctx.seat);

        let cards: Vec<Card> = hand.iter().copied().collect();
        let suit_counts = suit_tally(hand);

        let mut best: Option<(i32, [Card; 3], [Card; 3])> = None;

        for i in 0..cards.len() - 2 {
            for j in i + 1..cards.len() - 1 {
                for k in j + 1..cards.len() {
                    let triple = [cards[i], cards[j], cards[k]];
                    let score = score_pass_set(
                        &triple,
                        hand,
                        ctx,
                        style,
                        passing_to_trailing,
                        passing_to_leader,
                        my_score,
                        snapshot,
                        &suit_counts,
                    );
                    let mut ordered = triple;
                    ordered.sort_by_key(|card| card_sort_key(*card));

                    match &mut best {
                        None => best = Some((score, triple, ordered)),
                        Some((best_score, best_triple, best_ordered)) => {
                            if score > *best_score
                                || (score == *best_score
                                    && compare_sorted_triples(&ordered, best_ordered)
                                        == std::cmp::Ordering::Less)
                            {
                                *best_score = score;
                                *best_triple = triple;
                                *best_ordered = ordered;
                            }
                        }
                    }
                }
            }
        }

        if debug_enabled() {
            if let Some((score, picks, _)) = &best {
                eprintln!(
                    "mdhearts: pass best score={} cards=[{}, {}, {}]",
                    score, picks[0], picks[1], picks[2]
                );
            }
        }

        best.map(|(_, picks, _)| picks)
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
    let mut parts: Vec<(&'static str, i32)> = Vec::new();
    let suit_len = count_cards_in_suit(hand, card.suit);
    let rank_value = card.rank.value() as i32;
    let card_penalty = card.penalty_value() as i32;

    if card.is_queen_of_spades() {
        score += 18_000;
        parts.push(("qs_priority", 18_000));
    }

    if card.suit == Suit::Spades && !matches!(style, BotStyle::AggressiveMoon) {
        match card.rank {
            Rank::Ace => {
                score += 5_000;
                parts.push(("spade_ace", 5000));
            }
            Rank::King => {
                score += 7_000;
                parts.push(("spade_king", 7000));
            }
            Rank::Queen => {
                score += 18_000;
                parts.push(("spade_queen", 18000));
            }
            Rank::Jack => {
                score += 2_500;
                parts.push(("spade_jack", 2500));
            }
            _ => {}
        }
    }

    if card.suit == Suit::Hearts {
        let d = 6_000 + rank_value * 120;
        score += d;
        parts.push(("hearts_value", d));
    } else if rank_value >= Rank::King.value() as i32 {
        let d = 2_200 + rank_value * 80;
        score += d;
        parts.push(("high_rank_offsuit", d));
    }

    if suit_len <= 2 {
        let d = 4_000 - (suit_len as i32 * 800);
        score += d;
        parts.push(("short_suit_void", d));
    } else if suit_len >= 5 {
        let d = -((suit_len as i32 - 4) * 400);
        score += d;
        parts.push(("long_suit_penalty", d));
    }

    if passing_to_trailing {
        let d = card_penalty * 1_400;
        score += d;
        parts.push(("to_trailing_penalty_bonus", d));
    }

    if passing_to_leader {
        let d = -(card_penalty * pass_weights().to_leader_penalty);
        score += d;
        parts.push(("to_leader_penalty_avoid", d));
        if card.is_queen_of_spades() {
            // Never hand QS to the scoreboard leader; overpower any positive QS heuristics.
            score -= 50_000;
            parts.push(("avoid_qs_to_leader", -50000));
        }
    }

    if my_score >= 75 {
        let d = card_penalty * 1_600;
        score += d;
        parts.push(("high_self_score_shed", d));
    }

    if ctx.tracker.is_unseen(card) {
        score += 90;
        parts.push(("unseen_bonus", 90));
    }

    let two_of_clubs = Card::new(Rank::Two, Suit::Clubs);
    if card == two_of_clubs {
        score -= 4_000;
        parts.push(("two_of_clubs_keep", -4000));
    }

    // Style adjustments
    match style {
        BotStyle::AggressiveMoon => {
            if card.suit == Suit::Hearts {
                score -= 9_000;
                parts.push(("moon_keep_hearts", -9000));
            }
            if card.is_queen_of_spades() {
                score -= 12_000;
                parts.push(("moon_keep_qs", -12000));
            }
            if card.suit == Suit::Spades && card.rank >= Rank::Queen {
                score -= 9_000;
                parts.push(("moon_keep_high_spades", -9000));
            }
            if suit_len == 1 && card.suit != Suit::Hearts {
                score += 2_500;
                parts.push(("moon_void_nonhearts", 2500));
            }
        }
        BotStyle::HuntLeader => {
            if card_penalty > 0 {
                let d = 900 * card_penalty;
                score += d;
                parts.push(("hunt_pass_penalty", d));
                if passing_to_trailing {
                    let d2 = 600 * card_penalty;
                    score += d2;
                    parts.push(("hunt_pass_to_trailing", d2));
                }
            }
        }
        BotStyle::Cautious => {}
    }

    // Late-round adjustment to shed high cards.
    let cards_played = ctx.cards_played() as i32;
    let d = cards_played * 12;
    score += d;
    parts.push(("cards_played_bias", d));

    // Bias towards discarding the very highest ranks when we are well ahead.
    if snapshot.min_player == ctx.seat && snapshot.max_score - snapshot.min_score >= 15 {
        let d = rank_value * 40;
        score += d;
        parts.push(("leader_high_rank_bias", d));
    }
    if debug_enabled() {
        let mut detail = String::new();
        use std::fmt::Write as _;
        for (name, delta) in &parts {
            let _ = write!(&mut detail, " {}={}", name, delta);
        }
        eprintln!("mdhearts: pass {} total={} parts:{}", card, score, detail);
    }
    score
}

fn suit_tally(hand: &Hand) -> [usize; 4] {
    let mut counts = [0usize; 4];
    for card in hand.iter() {
        counts[suit_index(card.suit)] += 1;
    }
    counts
}

fn suit_index(suit: Suit) -> usize {
    match suit {
        Suit::Clubs => 0,
        Suit::Diamonds => 1,
        Suit::Hearts => 2,
        Suit::Spades => 3,
    }
}

fn compare_sorted_triples(a: &[Card; 3], b: &[Card; 3]) -> Ordering {
    for idx in 0..3 {
        let key_a = card_sort_key(a[idx]);
        let key_b = card_sort_key(b[idx]);
        match key_a.cmp(&key_b) {
            Ordering::Equal => continue,
            other => return other,
        }
    }
    Ordering::Equal
}

#[allow(clippy::too_many_arguments)]
fn score_pass_set(
    triple: &[Card; 3],
    hand: &Hand,
    ctx: &BotContext<'_>,
    style: BotStyle,
    passing_to_trailing: bool,
    passing_to_leader: bool,
    my_score: u32,
    snapshot: super::ScoreSnapshot,
    suit_counts: &[usize; 4],
) -> i32 {
    let mut total = 0;
    for card in triple.iter().copied() {
        total += score_card(
            card,
            hand,
            ctx,
            style,
            passing_to_trailing,
            passing_to_leader,
            my_score,
            snapshot,
        );
    }

    let mut removed = [0usize; 4];
    for card in triple.iter() {
        removed[suit_index(card.suit)] += 1;
    }

    for (idx, removed_count) in removed.iter().enumerate() {
        if *removed_count > 0 && suit_counts[idx] == *removed_count {
            total += 1_800;
        }
    }

    let high_club_count = triple
        .iter()
        .filter(|card| card.suit == Suit::Clubs && card.rank >= Rank::Queen)
        .count();
    if high_club_count >= 2 {
        total += 6_000 + (high_club_count as i32 * 800);
    }

    if triple
        .iter()
        .any(|card| card.suit == Suit::Hearts && card.penalty_value() > 0)
        && passing_to_trailing
    {
        total += 1_200;
    }

    total
}

struct PassWeights {
    to_leader_penalty: i32,
}

fn pass_weights() -> &'static PassWeights {
    static CACHED: OnceLock<PassWeights> = OnceLock::new();
    CACHED.get_or_init(|| PassWeights {
        to_leader_penalty: std::env::var("MDH_W_PASS_TO_LEADER_PENALTY")
            .ok()
            .and_then(|s| s.parse::<i32>().ok())
            .unwrap_or(1400),
    })
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
        // Ensure the passing target (Left from North = East) is not the scoreboard leader
        let scores = build_scores([20, 25, 30, 10]);
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
    fn pass_prefers_shedding_multiple_high_clubs() {
        let seat = PlayerPosition::West;
        let passing = PassingDirection::Left;
        let hand = vec![
            Card::new(Rank::Ace, Suit::Clubs),
            Card::new(Rank::King, Suit::Clubs),
            Card::new(Rank::Queen, Suit::Clubs),
            Card::new(Rank::Jack, Suit::Clubs),
            Card::new(Rank::Queen, Suit::Spades),
            Card::new(Rank::Ten, Suit::Spades),
            Card::new(Rank::Ace, Suit::Hearts),
            Card::new(Rank::Five, Suit::Hearts),
            Card::new(Rank::Three, Suit::Hearts),
            Card::new(Rank::Nine, Suit::Diamonds),
            Card::new(Rank::Eight, Suit::Diamonds),
            Card::new(Rank::Six, Suit::Diamonds),
            Card::new(Rank::Four, Suit::Diamonds),
        ];
        let round = build_round(seat, &hand, passing);
        let scores = build_scores([18, 24, 19, 26]);
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
        let high_clubs = [
            Card::new(Rank::Ace, Suit::Clubs),
            Card::new(Rank::King, Suit::Clubs),
            Card::new(Rank::Queen, Suit::Clubs),
        ];
        let shed_high_clubs = picks
            .iter()
            .filter(|card| high_clubs.contains(card))
            .count();
        assert!(
            shed_high_clubs >= 2,
            "expected at least two high clubs to be passed, got {:?}",
            picks
        );
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
        let snapshot = snapshot_scores(&ctx_unseen.scores);
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
        let snapshot_seen = snapshot_scores(&ctx_seen.scores);
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
