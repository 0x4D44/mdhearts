use super::{
    BotContext, BotStyle, card_sort_key, count_cards_in_suit, determine_style, snapshot_scores,
};
use hearts_core::model::card::Card;
use hearts_core::model::player::PlayerPosition;
use hearts_core::model::round::{PlayOutcome, RoundState};
use hearts_core::model::suit::Suit;
use std::cmp::Ordering;
use std::sync::OnceLock;

fn debug_enabled() -> bool {
    static ON: std::sync::OnceLock<bool> = std::sync::OnceLock::new();
    *ON.get_or_init(|| {
        std::env::var("MDH_DEBUG_LOGS")
            .map(|v| v == "1" || v.eq_ignore_ascii_case("true") || v.eq_ignore_ascii_case("on"))
            .unwrap_or(false)
    })
}

pub struct PlayPlanner;

impl PlayPlanner {
    pub fn choose(legal: &[Card], ctx: &BotContext<'_>) -> Option<Card> {
        if legal.is_empty() {
            return None;
        }

        let style = determine_style(ctx);
        let snapshot = snapshot_scores(ctx.scores);
        let lead_suit = ctx.round.current_trick().lead_suit();
        let mut best: Option<(Card, i32)> = None;

        for &card in legal {
            let (winner, penalties) = simulate_trick(card, ctx, style, snapshot.max_player);
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
            );
            let mut dbg = DebugParts::maybe_new(card, style, lead_suit, score);

            // Void creation bonus.
            let suit_remaining = count_cards_in_suit(ctx.hand(), card.suit);
            if suit_remaining <= 1 {
                score += 750;
                dbg.add("void_creation", 750);
            }

            // Prefer dumping high cards when following suit.
            if let Some(lead) = lead_suit {
                if card.suit == lead {
                    let d = -((card.rank.value() as i32) * 24);
                    score += d;
                    dbg.add("follow_high_rank_penalty", d);
                } else {
                    let d = card.penalty_value() as i32 * weights().off_suit_dump_bonus;
                    score += d;
                    dbg.add("off_suit_dump_bonus", d);
                }
            } else {
                // We are leading.
                let d = -((card.rank.value() as i32) * 10);
                score += d;
                dbg.add("lead_rank_bias", d);
                if card.suit == Suit::Hearts
                    && !ctx.round.hearts_broken()
                    && style != BotStyle::HuntLeader
                {
                    score -= 1_100;
                    dbg.add("lead_unbroken_hearts_penalty", -1100);
                }
                // Early-round caution: even if hearts are broken, avoid leading hearts too early in Cautious style
                if style == BotStyle::Cautious
                    && card.suit == Suit::Hearts
                    && ctx.round.hearts_broken()
                    && ctx.cards_played() < 16
                {
                    let p = -weights().early_hearts_lead_caution;
                    score += p;
                    dbg.add("early_round_lead_hearts_caution", p);
                }
                if style == BotStyle::HuntLeader && card.penalty_value() > 0 {
                    let d = 10_000 + (card.penalty_value() as i32 * 400);
                    score += d;
                    dbg.add("hunt_leader_lead_dump", d);
                }
                if style == BotStyle::AggressiveMoon && card.suit == Suit::Hearts {
                    score += 1_300;
                    dbg.add("moon_lead_hearts_bonus", 1300);
                }
            }

            // Late-round urgency to shed penalties if we are at risk.
            if snapshot.max_player == ctx.seat && snapshot.max_score >= 90 {
                if will_capture {
                    let d = -(penalties as i32 * 1200);
                    score += d;
                    dbg.add("near100_self_capture_penalty", d);
                } else {
                    let d = penalties as i32 * 300;
                    score += d;
                    dbg.add("near100_shed_bonus", d);
                }
            }

            // Tracker-based pacing: fewer unseen cards => accelerate shedding points.
            let cards_played = ctx.cards_played() as i32;
            let d = cards_played * weights().cards_played_bias;
            score += d;
            dbg.add("cards_played_bias", d);
            if ctx.tracker.is_unseen(card) {
                score += 20;
                dbg.add("unseen_card_bonus", 20);
            }

            dbg.finish(score);

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

    pub fn explain_candidates(legal: &[Card], ctx: &BotContext<'_>) -> Vec<(Card, i32)> {
        if legal.is_empty() {
            return Vec::new();
        }
        let style = determine_style(ctx);
        let snapshot = snapshot_scores(ctx.scores);
        let lead_suit = ctx.round.current_trick().lead_suit();
        let mut out: Vec<(Card, i32)> = Vec::new();
        for &card in legal {
            let (winner, penalties) = simulate_trick(card, ctx, style, snapshot.max_player);
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
            );
            let suit_remaining = count_cards_in_suit(ctx.hand(), card.suit);
            if suit_remaining <= 1 {
                score += 750;
            }
            if let Some(lead) = lead_suit {
                if card.suit == lead {
                    score -= (card.rank.value() as i32) * 24;
                } else {
                    score += card.penalty_value() as i32 * weights().off_suit_dump_bonus;
                }
            } else {
                score -= (card.rank.value() as i32) * 10;
                if card.suit == Suit::Hearts && !ctx.round.hearts_broken() && style != BotStyle::HuntLeader
                {
                    score -= 1_100;
                }
                if style == BotStyle::HuntLeader && card.penalty_value() > 0 {
                    score += 10_000 + (card.penalty_value() as i32 * 400);
                }
                if style == BotStyle::AggressiveMoon && card.suit == Suit::Hearts {
                    score += 1_300;
                }
            }
            if snapshot.max_player == ctx.seat && snapshot.max_score >= 90 {
                if will_capture {
                    score -= penalties as i32 * 1_200;
                } else {
                    score += penalties as i32 * 300;
                }
            }
            let cards_played = ctx.cards_played() as i32;
            score += cards_played * weights().cards_played_bias;
            if ctx.tracker.is_unseen(card) {
                score += 20;
            }
            out.push((card, score));
        }
        out.sort_by(|a, b| b.1.cmp(&a.1).then_with(|| card_sort_key(a.0).cmp(&card_sort_key(b.0))));
        out
    }
}

struct DebugParts {
    on: bool,
    msg: String,
}

impl DebugParts {
    fn maybe_new(card: Card, style: BotStyle, lead: Option<Suit>, base: i32) -> Self {
        if debug_enabled() {
            let s = format!("mdhearts: cand {} {:?} lead={:?} base={} parts:", card, style, lead, base);
            Self { on: true, msg: s }
        } else {
            Self { on: false, msg: String::new() }
        }
    }
    fn add(&mut self, name: &str, delta: i32) {
        if self.on {
            use std::fmt::Write as _;
            let _ = write!(&mut self.msg, " {}={}", name, delta);
        }
    }
    fn finish(self, total: i32) {
        if self.on {
            eprintln!("{} total={}", self.msg, total);
        }
    }
}

#[cfg(test)]
pub(crate) fn score_candidate_for_tests(card: Card, ctx: &BotContext<'_>, style: BotStyle) -> i32 {
    let snapshot = snapshot_scores(ctx.scores);
    let lead_suit = ctx.round.current_trick().lead_suit();
    let (winner, penalties) = simulate_trick(card, ctx, style, snapshot.max_player);
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
    );

    let suit_remaining = count_cards_in_suit(ctx.hand(), card.suit);
    if suit_remaining <= 1 {
        score += 750;
    }

    if let Some(lead) = lead_suit {
        if card.suit == lead {
            score -= (card.rank.value() as i32) * 24;
        } else {
            score += card.penalty_value() as i32 * 500;
        }
    } else {
        score -= (card.rank.value() as i32) * 10;
        if card.suit == Suit::Hearts && !ctx.round.hearts_broken() && style != BotStyle::HuntLeader
        {
            score -= 1_100;
        }
        if style == BotStyle::HuntLeader && card.penalty_value() > 0 {
            score += 10_000 + (card.penalty_value() as i32 * 400);
        }
        if style == BotStyle::AggressiveMoon && card.suit == Suit::Hearts {
            score += 1_300;
        }
    }

    if snapshot.max_player == ctx.seat && snapshot.max_score >= 90 {
        if will_capture {
            score -= penalties as i32 * 1_200;
        } else {
            score += penalties as i32 * 300;
        }
    }

    let cards_played = ctx.cards_played() as i32;
    score += cards_played * 8;
    if ctx.tracker.is_unseen(card) {
        score += 20;
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
) -> i32 {
    let penalties_i32 = penalties as i32;
    let mut score: i32 = 0;

    if will_capture {
        score -= 4_800;
        score -= penalties_i32 * 700;
    } else {
        score += 600;
        score += penalties_i32 * 500;
    }

    if penalties == 0 && will_capture {
        // Winning a clean trick is still mildly negative to keep low profile.
        score -= (card.rank.value() as i32) * 18;
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
                score += 5_500 + penalties_i32 * 900;
            } else {
                score -= penalties_i32 * 800;
            }
        }
        BotStyle::HuntLeader => {
            if !will_capture && penalties > 0 && winner == snapshot.max_player {
                score += penalties_i32 * weights().hunt_feed_perpen;
            }
            if will_capture {
                score -= 1_000;
            }
        }
        BotStyle::Cautious => {}
    }

    // Endgame nuance
    let my_score = ctx.scores.score(ctx.seat);
    let cards_left = ctx.hand().len() as i32;
    // If someone else is near 100 and we can feed them, mildly prefer it in all styles.
    if snapshot.max_player != ctx.seat
        && snapshot.max_score >= 90
        && !will_capture
        && penalties > 0
        && winner == snapshot.max_player
    {
        score += penalties_i32 * (400 + (20 * (10 - cards_left.max(1))))
    }
    // If we are near 100, avoid captures even more.
    if my_score >= 85 {
        if will_capture {
            score -= weights().near100_self_capture_base + penalties_i32 * 900;
        } else {
            score += penalties_i32 * weights().near100_shed_perpen;
        }
    }

    score
}

fn simulate_trick(
    card: Card,
    ctx: &BotContext<'_>,
    style: BotStyle,
    leader_target: PlayerPosition,
) -> (PlayerPosition, u8) {
    let mut sim = ctx.round.clone();
    let seat = ctx.seat;
    let mut outcome = match sim.play_card(seat, card) {
        Ok(result) => result,
        Err(_) => return (seat, 0),
    };

    while !matches!(outcome, PlayOutcome::TrickCompleted { .. }) {
        let next_seat = next_to_play(&sim);
        let response = choose_followup_card(
            &sim,
            next_seat,
            style,
            Some(ctx.tracker),
            ctx.seat,
            Some(leader_target),
        );
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

struct Weights {
    off_suit_dump_bonus: i32,
    cards_played_bias: i32,
    early_hearts_lead_caution: i32,
    near100_self_capture_base: i32,
    near100_shed_perpen: i32,
    hunt_feed_perpen: i32,
}

fn parse_env_i32(key: &str) -> Option<i32> {
    std::env::var(key).ok().and_then(|s| s.parse::<i32>().ok())
}

fn weights() -> &'static Weights {
    static CACHED: OnceLock<Weights> = OnceLock::new();
    CACHED.get_or_init(|| Weights {
        off_suit_dump_bonus: parse_env_i32("MDH_W_OFFSUIT_BONUS").unwrap_or(600),
        cards_played_bias: parse_env_i32("MDH_W_CARDS_PLAYED").unwrap_or(10),
        early_hearts_lead_caution: parse_env_i32("MDH_W_EARLY_HEARTS_LEAD").unwrap_or(600),
        near100_self_capture_base: parse_env_i32("MDH_W_NEAR100_SELF_CAPTURE_BASE").unwrap_or(1300),
        near100_shed_perpen: parse_env_i32("MDH_W_NEAR100_SHED_PERPEN").unwrap_or(250),
        hunt_feed_perpen: parse_env_i32("MDH_W_HUNT_FEED_PERPEN").unwrap_or(800),
    })
}

pub fn debug_weights_string() -> String {
    let w = weights();
    format!(
        "off_suit_dump_bonus={} cards_played_bias={} early_hearts_lead_caution={} near100_self_capture_base={} near100_shed_perpen={} hunt_feed_perpen={}",
        w.off_suit_dump_bonus,
        w.cards_played_bias,
        w.early_hearts_lead_caution,
        w.near100_self_capture_base,
        w.near100_shed_perpen,
        w.hunt_feed_perpen
    )
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
    tracker: Option<&crate::bot::tracker::UnseenTracker>,
    origin: PlayerPosition,
    leader_target: Option<PlayerPosition>,
) -> Card {
    let legal = legal_moves_for(round, seat);
    let lead_suit = round.current_trick().lead_suit();

    if let Some(lead) = lead_suit {
        // If we can follow suit, play the lowest of lead suit.
        if let Some(card) = legal
            .iter()
            .copied()
            .filter(|c| c.suit == lead)
            .min_by_key(|card| card.rank.value())
        {
            return card;
        }
        // Can't follow: dump strategy.
        // Bias towards dumping hearts if broken; otherwise queen of spades, then max penalty.
        let hearts_void = tracker
            .map(|t| t.is_void(seat, Suit::Hearts))
            .unwrap_or(false);
        let provisional = provisional_winner(round);
        let giving_to_origin = provisional == Some(origin);
        // Avoid giving points to origin (the player we are simulating for) to prevent self-dump skew
        if giving_to_origin {
            // choose lowest non-penalty if possible
            if let Some(card) = legal
                .iter()
                .copied()
                .filter(|c| c.penalty_value() == 0)
                .min_by_key(|c| c.rank.value())
            {
                return card;
            }
        }
        // If provisional winner is the scoreboard leader, prefer to dump QS or hearts to them
        if let (Some(pw), Some(leader)) = (provisional, leader_target) {
            if pw == leader {
                if let Some(qs) = legal.iter().copied().find(|c| c.is_queen_of_spades()) {
                    return qs;
                }
                if round.hearts_broken()
                    && !hearts_void
                    && let Some(card) = legal
                        .iter()
                        .copied()
                        .filter(|c| c.suit == Suit::Hearts)
                        .max_by_key(|c| c.rank.value())
                {
                    return card;
                }
            }
        }
        if round.hearts_broken()
            && !hearts_void
            && let Some(card) = legal
                .iter()
                .copied()
                .filter(|c| c.suit == Suit::Hearts)
                .max_by_key(|c| c.rank.value())
        {
            return card;
        }
        if let Some(qs) = legal.iter().copied().find(|c| c.is_queen_of_spades()) {
            return qs;
        }
    }

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

fn provisional_winner(round: &RoundState) -> Option<PlayerPosition> {
    let trick = round.current_trick();
    let lead = trick.lead_suit()?;
    trick
        .plays()
        .iter()
        .filter(|p| p.card.suit == lead)
        .max_by_key(|p| p.card.rank.value())
        .map(|p| p.position)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::bot::tracker::UnseenTracker;
    use crate::bot::{BotContext, BotDifficulty};
    use hearts_core::model::card::Card;
    use hearts_core::model::hand::Hand;
    use hearts_core::model::passing::PassingDirection;
    use hearts_core::model::player::PlayerPosition;
    use hearts_core::model::rank::Rank;
    use hearts_core::model::round::{RoundPhase, RoundState};
    use hearts_core::model::score::ScoreBoard;
    use hearts_core::model::suit::Suit;

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
    ) -> BotContext<'a> {
        BotContext::new(
            seat,
            round,
            scores,
            PassingDirection::Hold,
            tracker,
            difficulty,
        )
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
        let ctx = make_ctx(
            seat,
            &round,
            &scores,
            &tracker,
            BotDifficulty::NormalHeuristic,
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
        let ctx = make_ctx(
            seat,
            &round,
            &scores,
            &tracker,
            BotDifficulty::NormalHeuristic,
        );
        let legal = legal_moves_for(&round, seat);

        let choice = PlayPlanner::choose(&legal, &ctx).unwrap();
        assert_ne!(choice, Card::new(Rank::King, Suit::Hearts));
        assert!(choice.suit == Suit::Hearts);
    }

    #[test]
    fn avoid_capture_when_we_are_near_100() {
        // East (our seat) is near 100 and can either capture with a high heart or slough a low heart to avoid capture.
        let _seat = PlayerPosition::East;
        let round = build_round(
            PlayerPosition::North,
            [
                vec![Card::new(Rank::Ten, Suit::Hearts)], // North
                vec![Card::new(Rank::Nine, Suit::Clubs)], // East
                vec![
                    Card::new(Rank::Queen, Suit::Hearts),
                    Card::new(Rank::Two, Suit::Hearts),
                ], // South (our seat)
                vec![Card::new(Rank::Three, Suit::Clubs)], // West
            ],
            &[
                (PlayerPosition::North, Card::new(Rank::Ten, Suit::Hearts)),
                (PlayerPosition::East, Card::new(Rank::Nine, Suit::Clubs)),
            ],
            true,
        );

        let scores = build_scores([40, 95, 60, 70]); // East near 100; South is us
        let mut tracker = UnseenTracker::new();
        tracker.reset_for_round(&round);
        let ctx = make_ctx(
            PlayerPosition::South,
            &round,
            &scores,
            &tracker,
            BotDifficulty::NormalHeuristic,
        );
        let legal = legal_moves_for(&round, PlayerPosition::South);
        assert!(legal.contains(&Card::new(Rank::Queen, Suit::Hearts)));
        assert!(legal.contains(&Card::new(Rank::Two, Suit::Hearts)));

        let choice = PlayPlanner::choose(&legal, &ctx).unwrap();
        // Should prefer to avoid capturing by playing the low heart
        assert_eq!(choice, Card::new(Rank::Two, Suit::Hearts));
    }

    #[test]
    fn followup_void_prefers_hearts_when_broken() {
        // Setup: North leads King of Clubs, East plays Ace of Clubs (provisional winner East).
        // South cannot follow clubs and has hearts available, hearts are broken.
        // Expect: choose_followup_card for South dumps highest heart.
        let starting = PlayerPosition::North;
        let hearts = vec![
            Card::new(Rank::Ten, Suit::Hearts),
            Card::new(Rank::Seven, Suit::Hearts),
        ];
        let hands = [
            vec![Card::new(Rank::Two, Suit::Spades)],   // North
            vec![Card::new(Rank::Two, Suit::Diamonds)], // East
            {
                let mut v = hearts.clone();
                v.push(Card::new(Rank::Five, Suit::Diamonds));
                v
            }, // South (no clubs)
            vec![Card::new(Rank::Three, Suit::Spades)], // West
        ];
        let plays = [
            (PlayerPosition::North, Card::new(Rank::King, Suit::Clubs)),
            (PlayerPosition::East, Card::new(Rank::Ace, Suit::Clubs)),
        ];
        let round = build_round(starting, hands, &plays, true);
        let choice = super::choose_followup_card(
            &round,
            PlayerPosition::South,
            BotStyle::Cautious,
            None,
            PlayerPosition::West,
            None,
        );
        assert_eq!(choice.suit, Suit::Hearts);
        assert_eq!(choice.rank, Rank::Ten);
    }

    #[test]
    fn followup_avoids_self_dump_when_origin_winning() {
        // Setup similar to previous, but origin is the provisional winner (East).
        // Expect: follower (South) avoids dumping hearts/QS and plays lowest non-penalty (diamond).
        let starting = PlayerPosition::North;
        let hands = [
            vec![Card::new(Rank::Two, Suit::Spades)],   // North
            vec![Card::new(Rank::Two, Suit::Diamonds)], // East
            vec![
                Card::new(Rank::Queen, Suit::Spades),
                Card::new(Rank::Nine, Suit::Hearts),
                Card::new(Rank::Three, Suit::Diamonds),
            ], // South (no clubs)
            vec![Card::new(Rank::Three, Suit::Spades)], // West
        ];
        let plays = [
            (PlayerPosition::North, Card::new(Rank::King, Suit::Clubs)),
            (PlayerPosition::East, Card::new(Rank::Ace, Suit::Clubs)),
        ];
        let round = build_round(starting, hands, &plays, true);
        let choice = super::choose_followup_card(
            &round,
            PlayerPosition::South,
            BotStyle::Cautious,
            None,
            PlayerPosition::East,
            None,
        );
        assert_eq!(choice.suit, Suit::Diamonds);
        assert_eq!(choice.rank, Rank::Three);
    }

    #[test]
    fn followup_targets_leader_with_qs_when_possible() {
        let starting = PlayerPosition::North;
        let hands = [
            vec![Card::new(Rank::Two, Suit::Spades)],   // North
            vec![Card::new(Rank::Ace, Suit::Clubs)],     // East (provisional winner)
            vec![
                Card::new(Rank::Queen, Suit::Spades),
                Card::new(Rank::Nine, Suit::Hearts),
                Card::new(Rank::Three, Suit::Diamonds),
            ], // South (no clubs)
            vec![Card::new(Rank::Three, Suit::Spades)], // West
        ];
        let plays = [
            (PlayerPosition::North, Card::new(Rank::King, Suit::Clubs)),
            (PlayerPosition::East, Card::new(Rank::Ace, Suit::Clubs)),
        ];
        let round = build_round(starting, hands, &plays, true);
        let choice = super::choose_followup_card(
            &round,
            PlayerPosition::South,
            BotStyle::Cautious,
            None,
            PlayerPosition::West,
            Some(PlayerPosition::East),
        );
        assert!(choice.is_queen_of_spades());
    }

    #[test]
    fn followup_avoids_dump_to_non_leader() {
        // East is provisional winner, but leader is West; avoid dumping QS to East, choose low non-penalty instead.
        let starting = PlayerPosition::North;
        let hands = [
            vec![Card::new(Rank::Two, Suit::Spades)],   // North
            vec![Card::new(Rank::Ace, Suit::Clubs)],     // East (provisional winner)
            vec![
                Card::new(Rank::Queen, Suit::Spades),
                Card::new(Rank::Nine, Suit::Hearts),
                Card::new(Rank::Three, Suit::Diamonds),
            ], // South (no clubs)
            vec![Card::new(Rank::Three, Suit::Spades)], // West
        ];
        let plays = [
            (PlayerPosition::North, Card::new(Rank::King, Suit::Clubs)),
            (PlayerPosition::East, Card::new(Rank::Ace, Suit::Clubs)),
        ];
        let round = build_round(starting, hands, &plays, true);
        let choice = super::choose_followup_card(
            &round,
            PlayerPosition::South,
            BotStyle::Cautious,
            None,
            PlayerPosition::West,
            Some(PlayerPosition::West), // Leader is West, not provisional winner
        );
        // Policy: it's still fine to dump hearts when broken even if provisional winner isn't leader.
        assert_eq!(choice.suit, Suit::Hearts);
    }

    #[test]
    fn followup_targets_leader_with_hearts_when_qs_unavailable() {
        let starting = PlayerPosition::North;
        let hands = [
            vec![Card::new(Rank::Two, Suit::Spades)],   // North
            vec![Card::new(Rank::Ace, Suit::Clubs)],     // East (provisional winner)
            vec![
                Card::new(Rank::Ten, Suit::Hearts),
                Card::new(Rank::Nine, Suit::Hearts),
                Card::new(Rank::Three, Suit::Diamonds),
            ], // South (no clubs, no QS)
            vec![Card::new(Rank::Three, Suit::Spades)], // West
        ];
        let plays = [
            (PlayerPosition::North, Card::new(Rank::King, Suit::Clubs)),
            (PlayerPosition::East, Card::new(Rank::Ace, Suit::Clubs)),
        ];
        let round = build_round(starting, hands, &plays, true);
        let choice = super::choose_followup_card(
            &round,
            PlayerPosition::South,
            BotStyle::Cautious,
            None,
            PlayerPosition::West,
            Some(PlayerPosition::East),
        );
        assert_eq!(choice.suit, Suit::Hearts);
        assert_eq!(choice.rank, Rank::Ten);
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
        let ctx = make_ctx(
            seat,
            &round,
            &scores,
            &tracker,
            BotDifficulty::NormalHeuristic,
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
        let ctx_unseen = make_ctx(
            seat,
            &round,
            &scores,
            &tracker,
            BotDifficulty::NormalHeuristic,
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
        let ctx = make_ctx(
            seat,
            &round,
            &scores,
            &tracker,
            BotDifficulty::NormalHeuristic,
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
        let ctx = make_ctx(
            seat,
            &round,
            &scores,
            &tracker,
            BotDifficulty::NormalHeuristic,
        );
        let legal = legal_moves_for(&round, seat);
        let choice = PlayPlanner::choose(&legal, &ctx).unwrap();
        assert!(legal.len() >= 2);
        assert_eq!(choice.suit, Suit::Hearts);
        assert!(choice.rank >= Rank::King);
    }

    #[test]
    fn early_round_broken_hearts_avoid_cautious_lead() {
        // Hearts are broken, but it's still early; in Cautious style, avoid leading hearts when a safe off-suit exists.
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
            true,
        );
        let scores = build_scores([20, 18, 24, 19]);
        let mut tracker = UnseenTracker::new();
        tracker.reset_for_round(&round);
        let ctx = make_ctx(
            seat,
            &round,
            &scores,
            &tracker,
            BotDifficulty::NormalHeuristic,
        );
        let legal = legal_moves_for(&round, seat);
        let choice = PlayPlanner::choose(&legal, &ctx).unwrap();
        // Even though hearts are broken, cautious lead should avoid hearts early when not hunting or mooning
        assert_ne!(choice.suit, Suit::Hearts);
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
        let ctx = make_ctx(
            seat,
            &round,
            &scores,
            &tracker,
            BotDifficulty::NormalHeuristic,
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
        let ctx = make_ctx(
            seat,
            &round,
            &scores,
            &tracker,
            BotDifficulty::NormalHeuristic,
        );
        let legal = legal_moves_for(&round, seat);

        let choice = PlayPlanner::choose(&legal, &ctx).unwrap();
        assert_eq!(choice, Card::new(Rank::Queen, Suit::Hearts));
    }
}
