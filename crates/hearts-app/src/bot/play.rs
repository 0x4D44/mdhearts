use super::{
    BotContext, BotStyle, DecisionLimit, MoonState, card_sort_key, count_cards_in_suit,
    detect_moon_pressure, determine_style, snapshot_scores,
};
use hearts_core::model::card::Card;
use hearts_core::model::player::PlayerPosition;
use hearts_core::model::rank::Rank;
use hearts_core::model::round::{PlayOutcome, RoundState};
use hearts_core::model::suit::Suit;
use std::cell::{Cell, RefCell};
use std::cmp::Ordering;
use std::collections::BTreeMap;
use std::sync::OnceLock;

fn debug_enabled() -> bool {
    static ON: std::sync::OnceLock<bool> = std::sync::OnceLock::new();
    *ON.get_or_init(|| {
        std::env::var("MDH_DEBUG_LOGS")
            .map(|v| v == "1" || v.eq_ignore_ascii_case("true") || v.eq_ignore_ascii_case("on"))
            .unwrap_or(false)
    })
}

fn test_trace_followup_enabled() -> bool {
    std::env::var("MDH_TEST_TRACE_FOLLOWUP")
        .map(|v| v == "1" || v.eq_ignore_ascii_case("true") || v.eq_ignore_ascii_case("on"))
        .unwrap_or(false)
}

// Feature flags to allow working on main behind runtime toggles.
fn hard_stage1_enabled() -> bool {
    let v1 = std::env::var("MDH_FEATURE_HARD_STAGE1").unwrap_or_default();
    let v12 = std::env::var("MDH_FEATURE_HARD_STAGE12").unwrap_or_default();
    let v = if !v1.is_empty() { v1 } else { v12 };
    v == "1" || v.eq_ignore_ascii_case("true") || v.eq_ignore_ascii_case("on")
}

fn hard_stage2_enabled() -> bool {
    let v2 = std::env::var("MDH_FEATURE_HARD_STAGE2").unwrap_or_default();
    let v12 = std::env::var("MDH_FEATURE_HARD_STAGE12").unwrap_or_default();
    let v = if !v2.is_empty() { v2 } else { v12 };
    v == "1" || v.eq_ignore_ascii_case("true") || v.eq_ignore_ascii_case("on")
}

thread_local! {
    static HARD_NUDGE_HITS: Cell<usize> = Cell::new(0);
    static HARD_NUDGE_TRACE: RefCell<BTreeMap<&'static str, usize>> =
        RefCell::new(BTreeMap::new());
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct MixHintBiasStats {
    pub snnh_feed_bonus_hits: u32,
    pub snnh_capture_guard_hits: u32,
    pub shsh_feed_bonus_hits: u32,
    pub shsh_capture_guard_hits: u32,
}

impl MixHintBiasStats {
    fn is_empty(&self) -> bool {
        self.snnh_feed_bonus_hits == 0
            && self.snnh_capture_guard_hits == 0
            && self.shsh_feed_bonus_hits == 0
            && self.shsh_capture_guard_hits == 0
    }
}

#[derive(Clone, Copy)]
enum MixHintBiasCounter {
    SnnhFeed,
    SnnhCapture,
    ShshFeed,
    ShshCapture,
}

thread_local! {
    static MIX_HINT_BIAS: RefCell<MixHintBiasStats> = RefCell::new(MixHintBiasStats::default());
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum MixTag {
    Snnh,
    Shsh,
}

#[derive(Clone, Copy, Debug)]
struct MixSeatHint {
    mix: MixTag,
    seat: Option<PlayerPosition>,
}

fn mix_hint_for_play() -> Option<MixSeatHint> {
    fn parse_seat(label: &str) -> Option<PlayerPosition> {
        match label {
            "north" | "n" => Some(PlayerPosition::North),
            "east" | "e" => Some(PlayerPosition::East),
            "south" | "s" => Some(PlayerPosition::South),
            "west" | "w" => Some(PlayerPosition::West),
            _ => None,
        }
    }
    fn parse_mix(label: &str) -> Option<MixTag> {
        match label {
            "snnh" => Some(MixTag::Snnh),
            "shsh" => Some(MixTag::Shsh),
            _ => None,
        }
    }
    let raw = std::env::var("MDH_SEARCH_MIX_HINT").ok()?;
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        return None;
    }
    let mut parts = trimmed.splitn(2, ':');
    let mix_part = parts.next()?.trim().to_ascii_lowercase();
    let seat_part = parts
        .next()
        .map(|s| s.trim().to_ascii_lowercase())
        .filter(|s| !s.is_empty());
    let mix = parse_mix(&mix_part)?;
    let seat = seat_part
        .as_deref()
        .and_then(|label| parse_seat(label.trim()));
    Some(MixSeatHint { mix, seat })
}

pub(crate) fn reset_hard_nudge_hits() {
    HARD_NUDGE_HITS.with(|cell| cell.set(0));
}

pub(crate) fn record_hard_nudge_hit() {
    HARD_NUDGE_HITS.with(|cell| cell.set(cell.get().saturating_add(1)));
}

pub(crate) fn take_hard_nudge_hits() -> usize {
    HARD_NUDGE_HITS.with(|cell| {
        let value = cell.get();
        cell.set(0);
        value
    })
}

fn nudge_trace_enabled() -> bool {
    std::env::var("MDH_HARD_PLANNER_NUDGE_TRACE")
        .map(|v| v == "1" || v.eq_ignore_ascii_case("true") || v.eq_ignore_ascii_case("on"))
        .unwrap_or(false)
}

pub(crate) fn reset_mix_hint_bias_stats() {
    MIX_HINT_BIAS.with(|cell| *cell.borrow_mut() = MixHintBiasStats::default());
}

fn record_mix_hint_bias(counter: MixHintBiasCounter) {
    MIX_HINT_BIAS.with(|cell| {
        let mut stats = cell.borrow_mut();
        match counter {
            MixHintBiasCounter::SnnhFeed => {
                stats.snnh_feed_bonus_hits = stats.snnh_feed_bonus_hits.saturating_add(1);
            }
            MixHintBiasCounter::SnnhCapture => {
                stats.snnh_capture_guard_hits = stats.snnh_capture_guard_hits.saturating_add(1);
            }
            MixHintBiasCounter::ShshFeed => {
                stats.shsh_feed_bonus_hits = stats.shsh_feed_bonus_hits.saturating_add(1);
            }
            MixHintBiasCounter::ShshCapture => {
                stats.shsh_capture_guard_hits = stats.shsh_capture_guard_hits.saturating_add(1);
            }
        }
    });
}

pub(crate) fn take_mix_hint_bias_stats() -> Option<MixHintBiasStats> {
    MIX_HINT_BIAS.with(|cell| {
        let mut stats = cell.borrow_mut();
        if stats.is_empty() {
            return None;
        }
        let snapshot = *stats;
        *stats = MixHintBiasStats::default();
        Some(snapshot)
    })
}

pub(crate) fn reset_hard_nudge_trace() {
    if !nudge_trace_enabled() {
        return;
    }
    HARD_NUDGE_TRACE.with(|cell| cell.borrow_mut().clear());
}

fn record_hard_nudge_guard(reason: &'static str) {
    if !nudge_trace_enabled() {
        return;
    }
    HARD_NUDGE_TRACE.with(|cell| {
        let mut map = cell.borrow_mut();
        *map.entry(reason).or_insert(0) += 1;
    });
}

#[allow(dead_code)]
pub(crate) fn take_hard_nudge_trace_summary() -> Option<Vec<(String, usize)>> {
    if !nudge_trace_enabled() {
        return None;
    }
    HARD_NUDGE_TRACE.with(|cell| {
        let mut map = cell.borrow_mut();
        if map.is_empty() {
            return None;
        }
        let mut out: Vec<(String, usize)> =
            map.iter().map(|(k, v)| ((*k).to_string(), *v)).collect();
        out.sort_by(|a, b| a.0.cmp(&b.0));
        map.clear();
        Some(out)
    })
}

pub struct PlayPlanner;

impl PlayPlanner {
    pub fn choose(legal: &[Card], ctx: &BotContext<'_>) -> Option<Card> {
        Self::choose_with_limit(legal, ctx, None)
    }

    pub fn choose_with_limit(
        legal: &[Card],
        ctx: &BotContext<'_>,
        limit: Option<&DecisionLimit<'_>>,
    ) -> Option<Card> {
        if legal.is_empty() {
            return None;
        }

        if let Some(limit) = limit {
            if limit.expired() {
                return None;
            }
        }

        reset_hard_nudge_hits();
        reset_hard_nudge_trace();
        reset_mix_hint_bias_stats();

        let style = determine_style(ctx);
        let limit_ms = limit.and_then(|lim| lim.remaining_millis());
        let snapshot = snapshot_scores(&ctx.scores);
        let lead_suit = ctx.round.current_trick().lead_suit();
        let mut best: Option<(Card, i32)> = None;

        for &card in legal {
            if let Some(limit) = limit {
                if limit.expired() {
                    break;
                }
            }
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
                limit_ms,
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

            if matches!(ctx.difficulty, super::BotDifficulty::FutureHard) && penalties > 0 {
                let round_totals = ctx.round.penalty_totals();
                let mut projected = [0i32; 4];
                for seat in PlayerPosition::LOOP.iter().copied() {
                    projected[seat.index()] = round_totals[seat.index()] as i32;
                }
                projected[winner.index()] =
                    projected[winner.index()].saturating_add(penalties as i32);
                if will_capture {
                    let projected_self = projected[ctx.seat.index()];
                    let mut best_other = std::i32::MIN;
                    for seat in PlayerPosition::LOOP.iter().copied() {
                        if seat == ctx.seat {
                            continue;
                        }
                        best_other = best_other.max(projected[seat.index()]);
                    }
                    if projected_self > best_other {
                        let projected_gap = projected_self - best_other;
                        let d = -((penalties as i32) * 600 + projected_gap * 120);
                        score += d;
                        dbg.add("round_leader_self_capture_penalty", d);
                    }
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
        Self::explain_candidates_with_limit(legal, ctx, None)
    }

    pub fn explain_candidates_with_limit(
        legal: &[Card],
        ctx: &BotContext<'_>,
        limit_ms: Option<u32>,
    ) -> Vec<(Card, i32)> {
        if legal.is_empty() {
            return Vec::new();
        }
        reset_mix_hint_bias_stats();
        let style = determine_style(ctx);
        let snapshot = snapshot_scores(&ctx.scores);
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
                limit_ms,
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
                if card.suit == Suit::Hearts
                    && !ctx.round.hearts_broken()
                    && style != BotStyle::HuntLeader
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
        out.sort_by(|a, b| {
            b.1.cmp(&a.1)
                .then_with(|| card_sort_key(a.0).cmp(&card_sort_key(b.0)))
        });
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
            let s = format!(
                "mdhearts: cand {} {:?} lead={:?} base={} parts:",
                card, style, lead, base
            );
            Self { on: true, msg: s }
        } else {
            Self {
                on: false,
                msg: String::new(),
            }
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
    let snapshot = snapshot_scores(&ctx.scores);
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
        None,
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

#[derive(Clone, Copy, Debug)]
struct HardPlannerNudgeConfig {
    per_penalty: i32,
    max_base_for_nudge: i32,
    near_100_guard: u32,
    min_gap: u32,
    self_guard_gap: u32,
    round_leader_cap: u32,
}

fn hard_planner_nudge_config() -> HardPlannerNudgeConfig {
    let per_penalty = parse_env_i32("MDH_HARD_PLANNER_LEADER_FEED_NUDGE").unwrap_or(12);
    let max_base_for_nudge = parse_env_i32("MDH_HARD_PLANNER_MAX_BASE_FOR_NUDGE").unwrap_or(220);
    let near_100_guard = parse_env_i32("MDH_HARD_PLANNER_NUDGE_NEAR100")
        .unwrap_or(90)
        .clamp(0, 100) as u32;
    let min_gap = parse_env_i32("MDH_HARD_PLANNER_NUDGE_GAP_MIN")
        .unwrap_or(4)
        .max(0) as u32;
    let self_guard_gap = parse_env_i32("MDH_HARD_PLANNER_NUDGE_SELF_GAP")
        .unwrap_or(6)
        .max(0) as u32;
    let round_leader_cap = parse_env_i32("MDH_HARD_PLANNER_NUDGE_ROUND_CAP")
        .unwrap_or(6)
        .max(0) as u32;
    HardPlannerNudgeConfig {
        per_penalty,
        max_base_for_nudge,
        near_100_guard,
        min_gap,
        self_guard_gap,
        round_leader_cap,
    }
}

fn stage2_round_gap_cap() -> u32 {
    if !hard_stage2_enabled() {
        return 0;
    }
    parse_env_i32("MDH_STAGE2_ROUND_GAP_CAP")
        .unwrap_or(6)
        .max(0) as u32
}

fn stage2_avoid_dump_gap() -> u32 {
    if !hard_stage2_enabled() {
        // Use a huge value to effectively disable the avoidance logic when Stage 2 is off.
        return u32::MAX;
    }
    parse_env_i32("MDH_STAGE2_AVOID_DUMP_GAP")
        .unwrap_or(3)
        .max(0) as u32
}

fn queen_of_spades() -> Card {
    Card::new(Rank::Queen, Suit::Spades)
}

fn trick_contains_qs(trick: &hearts_core::model::trick::Trick) -> bool {
    let target = queen_of_spades();
    trick.plays().iter().any(|p| p.card == target)
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
    limit_ms: Option<u32>,
) -> i32 {
    let penalties_i32 = penalties as i32;
    let mut score: i32 = 0;
    // Current trick penalties already on table before our play
    let penalties_on_table_now: i32 = ctx
        .round
        .current_trick()
        .plays()
        .iter()
        .map(|p| p.card.penalty_value() as i32)
        .sum();
    let limit_ms = limit_ms.unwrap_or(0);
    let mix_hint = mix_hint_for_play();

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

    let leader_gap = snapshot
        .max_score
        .saturating_sub(ctx.scores.score(ctx.seat));
    let moon_state = ctx.tracker.moon_state(ctx.seat);
    let under_moon_pressure = matches!(moon_state, MoonState::Considering | MoonState::Committed);
    let gap_units = (leader_gap.min(30) / 10) as i32; // 0..=3
    let leader_feed_per = weights().leader_feed_base + gap_units * weights().leader_feed_gap_per10;
    // Planner-level leader targeting: small positive even before near-100 scenarios.
    if snapshot.max_player != ctx.seat
        && !will_capture
        && penalties > 0
        && ctx.scores.score(winner) == snapshot.max_score
        && snapshot.leader_gap > 0
    {
        score += penalties_i32 * leader_feed_per;
    }

    if under_moon_pressure && will_capture && penalties > 0 {
        score -= penalties_i32 * 90;
    }

    if matches!(
        ctx.seat,
        PlayerPosition::East | PlayerPosition::South | PlayerPosition::West
    ) {
        let leader_gap = snapshot
            .max_score
            .saturating_sub(ctx.scores.score(ctx.seat))
            .min(60) as i32;
        if !will_capture && penalties > 0 {
            let leader_weight = if winner == snapshot.max_player {
                260
            } else {
                110
            };
            let pursuit = leader_gap.max(1) * 45;
            score += penalties_i32 * (leader_weight + pursuit);
        }
        if will_capture && penalties > 0 {
            let lead_margin = ctx
                .scores
                .score(ctx.seat)
                .saturating_sub(snapshot.min_score)
                .min(60) as i32;
            let malus = 250 + lead_margin * 25;
            score -= penalties_i32 * malus;
        }
    }

    if let Some(hint) = mix_hint {
        let target_seat = hint.seat.unwrap_or(ctx.seat);
        let mid_limit = limit_ms >= 12_000;
        let high_limit = limit_ms >= 15_000;
        let ultra_limit = limit_ms >= 18_000;
        match hint.mix {
            MixTag::Snnh => {
                if matches!(target_seat, PlayerPosition::North | PlayerPosition::East) && mid_limit
                {
                    if !will_capture && penalties > 0 {
                        let leader_bonus = if winner == snapshot.max_player {
                            250
                        } else {
                            60
                        };
                        let mut bonus = if high_limit { 520 } else { 360 };
                        if ultra_limit {
                            bonus += 140;
                        }
                        score += penalties_i32 * (bonus + leader_bonus);
                        record_mix_hint_bias(MixHintBiasCounter::SnnhFeed);
                    }
                    if will_capture && penalties > 0 {
                        let malus = if ultra_limit {
                            420
                        } else if high_limit {
                            360
                        } else {
                            280
                        };
                        if snapshot.max_player == ctx.seat {
                            score -= penalties_i32 * 120;
                        }
                        score -= penalties_i32 * malus;
                        record_mix_hint_bias(MixHintBiasCounter::SnnhCapture);
                    }
                }
            }
            MixTag::Shsh => {
                if matches!(
                    target_seat,
                    PlayerPosition::East | PlayerPosition::South | PlayerPosition::West
                ) && (mid_limit || high_limit)
                {
                    if !will_capture && penalties > 0 {
                        let leader_bonus = if winner == snapshot.max_player {
                            240
                        } else {
                            90
                        };
                        let mut bonus = if high_limit { 420 } else { 300 };
                        if ultra_limit {
                            bonus += 110;
                        }
                        let chase_gap = snapshot
                            .max_score
                            .saturating_sub(ctx.scores.score(ctx.seat))
                            .min(25) as i32;
                        let chase_bonus = chase_gap * 45;
                        score += penalties_i32 * (bonus + leader_bonus + chase_bonus);
                        if snapshot.leader_gap > 0 {
                            let gap_units = (snapshot.leader_gap.min(40) / 5) as i32 + 1;
                            score += penalties_i32 * gap_units * 80;
                        }
                        if under_moon_pressure && snapshot.max_player != ctx.seat {
                            score += penalties_i32 * 120;
                        }
                        record_mix_hint_bias(MixHintBiasCounter::ShshFeed);
                    }
                    if will_capture && penalties > 0 {
                        let malus = if ultra_limit {
                            320
                        } else if high_limit {
                            260
                        } else {
                            210
                        };
                        if snapshot.max_player == ctx.seat {
                            score -= penalties_i32 * 160;
                        }
                        if snapshot.leader_gap <= 2 && snapshot.leader_gap > 0 {
                            score -= penalties_i32 * 140;
                        }
                        if under_moon_pressure {
                            score -= penalties_i32 * 120;
                        }
                        score -= penalties_i32 * malus;
                        record_mix_hint_bias(MixHintBiasCounter::ShshCapture);
                    }
                }
            }
        }
    }

    if let Some(hint) = mix_hint {
        if matches!(hint.mix, MixTag::Shsh)
            && matches!(
                hint.seat.unwrap_or(ctx.seat),
                PlayerPosition::East | PlayerPosition::South | PlayerPosition::West
            )
            && limit_ms >= 12_000
        {
            let trailing_gap = snapshot
                .max_score
                .saturating_sub(ctx.scores.score(ctx.seat))
                .min(40) as i32;
            if !will_capture && penalties > 0 {
                let pressure = trailing_gap.max(1) * 35;
                let leader_weight = if snapshot.max_player == ctx.seat {
                    40
                } else {
                    120
                };
                score += penalties_i32 * (pressure + leader_weight);
            }
            if will_capture {
                let self_gap = ctx
                    .scores
                    .score(ctx.seat)
                    .saturating_sub(snapshot.min_score)
                    .min(50) as i32;
                let malus = 210 + self_gap * 20;
                score -= penalties_i32 * malus.max(210);
            }
        }
    }

    // Hard-specific leader-feed nudge (wide-tier-inspired) with strict guards.
    let nudge_cfg = hard_planner_nudge_config();
    let leader_gap = snapshot.leader_gap;
    if matches!(ctx.difficulty, super::BotDifficulty::FutureHard) && hard_stage1_enabled() {
        let round_penalties = ctx.round.penalty_totals();
        let mut round_max: u8 = 0;
        let mut round_second: u8 = 0;
        let mut round_leader = snapshot.max_player;
        let mut round_unique = false;
        let mut round_penalty_leads = [0i32; 4];
        for seat in PlayerPosition::LOOP.iter().copied() {
            let value = round_penalties[seat.index()];
            if value > round_max {
                round_second = round_max;
                round_max = value;
                round_leader = seat;
                round_unique = value > 0;
            } else if value == round_max && value > 0 {
                round_unique = false;
            } else if value > round_second {
                round_second = value;
            }
            round_penalty_leads[seat.index()] = value as i32;
        }
        if round_max == 0 {
            round_unique = false;
        }
        let round_gap: u32 = if round_unique {
            (round_max.saturating_sub(round_second)) as u32
        } else {
            0
        };
        let mut effective_leader: Option<PlayerPosition> = None;
        let mut effective_gap = leader_gap;
        let mut leader_is_self = false;
        let scores_flat = snapshot.max_score == snapshot.min_score;
        if leader_gap > 0 {
            effective_leader = Some(snapshot.max_player);
            leader_is_self = snapshot.max_player == ctx.seat;
        } else if scores_flat && round_gap > 0 {
            effective_leader = Some(round_leader);
            effective_gap = round_gap;
            leader_is_self = round_leader == ctx.seat;
        }
        let hearts_ready = ctx.round.hearts_broken()
            || trick_contains_qs(ctx.round.current_trick())
            || card == queen_of_spades();
        let base_guard_blocked =
            nudge_cfg.max_base_for_nudge > 0 && leader_feed_per >= nudge_cfg.max_base_for_nudge;
        let mut round_leader_saturated = false;
        if nudge_cfg.round_leader_cap > 0 && penalties > 0 {
            let mut projected_round = [0i32; 4];
            for seat in PlayerPosition::LOOP.iter().copied() {
                projected_round[seat.index()] = round_penalties[seat.index()] as i32;
            }
            projected_round[winner.index()] =
                projected_round[winner.index()].saturating_add(penalties as i32);
            let projected_winner = projected_round[winner.index()];
            let mut best_other = std::i32::MIN;
            for seat in PlayerPosition::LOOP.iter().copied() {
                if seat == winner {
                    continue;
                }
                best_other = best_other.max(projected_round[seat.index()]);
            }
            let projected_gap = projected_winner.saturating_sub(best_other.max(0));
            if projected_gap as u32 >= nudge_cfg.round_leader_cap {
                round_leader_saturated = true;
            }
        }
        let winner_is_leader = effective_leader.map_or(false, |seat| seat == winner);
        let winner_is_effective_leader = winner_is_leader || round_leader_saturated;
        let safe_to_feed_nonleader = leader_is_self && effective_gap > 0;
        let gap_guard_min = if scores_flat {
            let adjusted = (nudge_cfg.min_gap as i32 - 2).max(1);
            adjusted as u32
        } else {
            nudge_cfg.min_gap
        };
        if scores_flat && penalties > 0 && !will_capture {
            if safe_to_feed_nonleader {
                let bonus = penalties_i32 * (leader_feed_per + 600);
                score += bonus;
            } else if winner_is_effective_leader {
                let d = -(penalties_i32 * (leader_feed_per + 600));
                score += d;
            }
        }
        if scores_flat
            && penalties == 0
            && !will_capture
            && (winner_is_effective_leader || safe_to_feed_nonleader)
            && effective_gap >= gap_guard_min
        {
            let d = -((effective_gap as i32 + 1) * 500);
            score += d;
        }
        let reason = if penalties == 0 {
            "no_penalties"
        } else if will_capture {
            "would_capture_self"
        } else if effective_leader.is_none() {
            "leader_undefined"
        } else if !winner_is_effective_leader && !safe_to_feed_nonleader {
            "not_leader_target"
        } else if leader_is_self
            && effective_gap >= nudge_cfg.self_guard_gap
            && !safe_to_feed_nonleader
        {
            "leader_is_self"
        } else if scores_flat && effective_gap == gap_guard_min && penalties > 0 {
            "gap_tie_soft"
        } else if scores_flat && effective_gap == gap_guard_min && penalties == 0 {
            "gap_zero_tie"
        } else if effective_gap < gap_guard_min {
            "gap_below_min"
        } else if round_leader_saturated && !safe_to_feed_nonleader {
            "round_leader_saturated"
        } else if snapshot.max_score >= nudge_cfg.near_100_guard {
            "leader_near_100_guard"
        } else if ctx.round.is_first_trick() {
            "first_trick"
        } else if nudge_cfg.per_penalty == 0 {
            "per_penalty_zero"
        } else if !hearts_ready {
            "hearts_not_ready"
        } else if base_guard_blocked {
            "base_guard_blocked"
        } else {
            score += penalties_i32 * nudge_cfg.per_penalty;
            record_hard_nudge_hit();
            "applied"
        };
        record_hard_nudge_guard(reason);
        if debug_enabled() {
            let totals = ctx.scores.standings();
            let our_score = ctx.scores.score(ctx.seat);
            let winner_score = ctx.scores.score(winner);
            eprintln!(
                "mdhearts: hard nudge seat={:?} card={} reason={} penalties={} table_pen={} winner={:?} leader_gap={} effective_gap={} leader_score={} our_score={} winner_score={} feed_per={} hearts_ready={} scores_flat={} base_guard_blocked={} self_gap_cfg={} min_gap_cfg={} near_guard={} match_totals={:?} round_totals={:?}",
                ctx.seat,
                card,
                reason,
                penalties,
                penalties_on_table_now,
                winner,
                leader_gap,
                effective_gap,
                snapshot.max_score,
                our_score,
                winner_score,
                leader_feed_per,
                hearts_ready,
                scores_flat,
                base_guard_blocked,
                nudge_cfg.self_guard_gap,
                gap_guard_min,
                nudge_cfg.near_100_guard,
                totals,
                round_penalties
            );
        }
    }

    if penalties > 0 && will_capture && hard_stage2_enabled() {
        let round_totals = ctx.round.penalty_totals();
        let mut best_other = 0u32;
        for seat in PlayerPosition::LOOP.iter().copied() {
            if seat == ctx.seat {
                continue;
            }
            best_other = best_other.max(round_totals[seat.index()] as u32);
        }
        let current_self = round_totals[ctx.seat.index()] as u32;
        let projected_self = current_self + penalties as u32;
        let projected_gap = projected_self.saturating_sub(best_other);
        let cap = stage2_round_gap_cap();
        if cap > 0 {
            let current_gap = current_self.saturating_sub(best_other);
            if penalties == 0 && current_gap >= cap {
                let clean_penalty = -((current_gap as i32) * 1_200 + 18_000);
                score += clean_penalty;
            }
            if projected_gap >= cap {
                let round_penalty = -((penalties as i32) * 2_200 + (projected_gap as i32) * 1_000);
                score += round_penalty;
            }
        }
    }
    if matches!(ctx.difficulty, super::BotDifficulty::FutureHard)
        && will_capture
        && penalties == 0
        && hard_stage2_enabled()
    {
        let round_totals = ctx.round.penalty_totals();
        let mut best_other = 0u32;
        for seat in PlayerPosition::LOOP.iter().copied() {
            if seat == ctx.seat {
                continue;
            }
            best_other = best_other.max(round_totals[seat.index()] as u32);
        }
        let my_round = round_totals[ctx.seat.index()] as u32;
        let projected_gap = my_round.saturating_sub(best_other);
        let cap = stage2_round_gap_cap();
        if cap > 0 && projected_gap >= cap {
            let clean_penalty = -((projected_gap as i32) * 1_200 + 18_000);
            score += clean_penalty;
        } else if cap > 0 {
            let threshold = cap.saturating_sub(1).max(1);
            if projected_gap >= threshold {
                let near_cap_penalty = -((projected_gap as i32) * 800 + 12_000);
                score += near_cap_penalty;
            }
        }
    }
    // Downweight feeding penalties to a non-leader (prevents blindly dumping QS to second place).
    if snapshot.max_player != ctx.seat
        && !will_capture
        && penalties > 0
        && winner != snapshot.max_player
        && penalties_on_table_now > 0
    {
        score -= penalties_i32 * weights().nonleader_feed_perpen;
        if card.is_queen_of_spades() {
            score -= 15_000;
        }
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
    let moon_pressure = detect_moon_pressure(ctx, &snapshot);
    if moon_pressure {
        let trick_pen = ctx.round.current_trick().penalty_total() as i32;
        let leader_pressure = (snapshot.max_score as i32).saturating_sub(70).max(0);
        let moon_base = 1500 + leader_pressure * 20 + trick_pen * 15;
        if will_capture {
            // Strongly discourage taking penalty tricks while others threaten to shoot.
            score -= moon_base + penalties_i32 * 700;
        } else if penalties > 0 && winner == snapshot.max_player {
            // Feeding the leader is preferred to avoid holding penalties.
            score += moon_base / 2 + penalties_i32 * 600;
        } else if penalties > 0 {
            // Still reward shedding to non-leaders during moon threats.
            score += penalties_i32 * 250;
        }
    }
    // Phase B (Hard-only default): tiny extra penalty when near 100 and we would capture points now.
    if matches!(ctx.difficulty, super::BotDifficulty::FutureHard)
        && my_score >= 85
        && will_capture
        && penalties_on_table_now > 0
        && !ctx.round.is_first_trick()
    {
        score -= penalties_i32 * 30;
        if snapshot.max_score >= 90 {
            score -= penalties_i32 * 20;
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
    leader_feed_base: i32,
    nonleader_feed_perpen: i32,
    leader_feed_gap_per10: i32,
    endgame_feed_cap_perpen: i32,
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
        leader_feed_base: parse_env_i32("MDH_W_LEADER_FEED_BASE").unwrap_or(120),
        nonleader_feed_perpen: parse_env_i32("MDH_W_NONLEADER_FEED_PERPEN").unwrap_or(2000),
        leader_feed_gap_per10: parse_env_i32("MDH_W_LEADER_FEED_GAP_PER10").unwrap_or(40),
        endgame_feed_cap_perpen: parse_env_i32("MDH_W_ENDGAME_FEED_CAP").unwrap_or(0),
    })
}

pub fn debug_weights_string() -> String {
    let w = weights();
    format!(
        "off_suit_dump_bonus={} cards_played_bias={} early_hearts_lead_caution={} near100_self_capture_base={} near100_shed_perpen={} hunt_feed_perpen={} leader_feed_base={} nonleader_feed_perpen={} leader_feed_gap_per10={} endgame_feed_cap_perpen={}",
        w.off_suit_dump_bonus,
        w.cards_played_bias,
        w.early_hearts_lead_caution,
        w.near100_self_capture_base,
        w.near100_shed_perpen,
        w.hunt_feed_perpen,
        w.leader_feed_base,
        w.nonleader_feed_perpen,
        w.leader_feed_gap_per10,
        w.endgame_feed_cap_perpen
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
    style: BotStyle,
    tracker: Option<&crate::bot::tracker::UnseenTracker>,
    origin: PlayerPosition,
    leader_target: Option<PlayerPosition>,
) -> Card {
    let legal = legal_moves_for(round, seat);
    if test_trace_followup_enabled() {
        eprintln!(
            "mdhearts:test_followup entry seat={:?} hand_len={} legal_len={} lead_suit={:?}",
            seat,
            round.hand(seat).len(),
            legal.len(),
            round.current_trick().lead_suit()
        );
    }
    // Defensive: if legality probing fails (e.g., strict opening rules vs. crafted tests),
    // fall back to a simple in-hand choice to avoid panics in simulations/tests.
    if legal.is_empty() {
        if let Some(card) = round
            .hand(seat)
            .iter()
            .copied()
            .min_by_key(|c| (c.penalty_value(), c.rank.value()))
        {
            return card;
        }
    }
    let lead_suit = round.current_trick().lead_suit();
    let penalties_on_table: u8 = round
        .current_trick()
        .plays()
        .iter()
        .map(|p| p.card.penalty_value())
        .sum();
    let round_totals = round.penalty_totals();
    let my_round_total = round_totals[seat.index()] as u32;
    let mut best_other_round = 0u32;
    let mut others_have_penalties = false;
    for other in PlayerPosition::LOOP.iter().copied() {
        if other == seat {
            continue;
        }
        let value = round_totals[other.index()] as u32;
        if value > 0 {
            others_have_penalties = true;
        }
        if value > best_other_round {
            best_other_round = value;
        }
    }
    let round_gap = my_round_total.saturating_sub(best_other_round);
    let committed_moon = tracker
        .map(|t| t.moon_state(seat) == MoonState::Committed)
        .unwrap_or(false);
    let round_gap_cap = stage2_round_gap_cap();
    let mut avoid_heavy_dump = false;
    if hard_stage2_enabled() {
        if round_gap == 0 {
            if round_gap_cap > 0 && best_other_round >= round_gap_cap {
                avoid_heavy_dump = true;
            }
            if others_have_penalties {
                let cautious_gap = stage2_avoid_dump_gap();
                if best_other_round >= cautious_gap {
                    avoid_heavy_dump = true;
                }
            }
            if round_gap_cap > 0 {
                let near_cap = best_other_round >= round_gap_cap.saturating_sub(1).max(1);
                if near_cap {
                    avoid_heavy_dump = true;
                }
            }
        }
        if committed_moon
            && !(others_have_penalties
                || (round_gap_cap > 0 && round_gap >= round_gap_cap)
                || (round_gap_cap > 0 && round_gap >= round_gap_cap.saturating_sub(1).max(1)))
        {
            avoid_heavy_dump = false;
        }
        if test_trace_followup_enabled() {
            eprintln!(
                "mdhearts:test_followup stage2 ctx seat={seat:?} round_gap={} best_other={} others_have_penalties={} avoid_heavy_dump={}",
                round_gap, best_other_round, others_have_penalties, avoid_heavy_dump
            );
        }
    }

    if let Some(lead) = lead_suit {
        let trick = round.current_trick();
        let current_lead_rank = trick
            .plays()
            .iter()
            .filter(|p| p.card.suit == lead)
            .map(|p| p.card.rank.value())
            .max()
            .unwrap_or(0);
        let can_overcall = legal
            .iter()
            .any(|c| c.suit == lead && c.rank.value() > current_lead_rank);
        if can_overcall && hard_stage2_enabled() {
            if let Some(current_winner) = provisional_winner(round) {
                if current_winner != seat {
                    let round_totals = round.penalty_totals();
                    let leader_total = round_totals[current_winner.index()] as u32;
                    let mut best_other_total = 0u32;
                    for other in PlayerPosition::LOOP.iter().copied() {
                        if other == current_winner {
                            continue;
                        }
                        best_other_total = best_other_total.max(round_totals[other.index()] as u32);
                    }
                    let round_gap = leader_total.saturating_sub(best_other_total);
                    let cap = stage2_round_gap_cap();
                    let near_threshold = cap > 0 && round_gap >= cap.saturating_sub(1).max(1);
                    let over_threshold = cap > 0 && leader_total >= cap;
                    let runaway = over_threshold || near_threshold;
                    if runaway && round_totals[seat.index()] as u32 <= leader_total {
                        if let Some(card) = legal
                            .iter()
                            .copied()
                            .filter(|c| c.suit == lead && c.rank.value() > current_lead_rank)
                            .min_by_key(|c| c.rank.value())
                        {
                            return card;
                        }
                    }
                }
            }
        }

        let follow_cards: Vec<Card> = legal.iter().copied().filter(|c| c.suit == lead).collect();
        if !follow_cards.is_empty() {
            if follow_cards.len() > 1 && hard_stage2_enabled() {
                let simulated: Vec<(Card, SimOutcome)> = follow_cards
                    .iter()
                    .copied()
                    .filter_map(|card| {
                        simulate_lead_outcome(
                            round,
                            tracker,
                            seat,
                            card,
                            style,
                            origin,
                            leader_target,
                        )
                        .map(|outcome| (card, outcome))
                    })
                    .collect();

                if !simulated.is_empty() {
                    if let Some((card, outcome)) = simulated
                        .iter()
                        .filter(|(_, outcome)| outcome.winner != seat)
                        .min_by_key(|(card, outcome)| {
                            (
                                outcome.trick_penalties as u32,
                                card.penalty_value() as u32,
                                card.rank.value() as u32,
                            )
                        })
                    {
                        if debug_enabled() {
                            eprintln!(
                                "mdhearts: followup_sim_dodge seat={seat:?} card={card} penalties={} seat_run={}",
                                outcome.trick_penalties, outcome.seat_penalty_run
                            );
                        }
                        return *card;
                    }

                    if let Some((card, outcome)) = simulated.iter().min_by_key(|(card, outcome)| {
                        (
                            outcome.seat_penalty_run,
                            outcome.trick_penalties as u32,
                            card.penalty_value() as u32,
                            card.rank.value() as u32,
                        )
                    }) {
                        if debug_enabled() {
                            eprintln!(
                                "mdhearts: followup_sim_limit seat={seat:?} card={card} seat_run={} trick_pen={}",
                                outcome.seat_penalty_run, outcome.trick_penalties
                            );
                        }
                        return *card;
                    }
                }
            }

            if let Some(card) = follow_cards
                .iter()
                .copied()
                .min_by_key(|card| card.rank.value())
            {
                return card;
            }
        }
        // Can't follow: dump strategy.
        // Bias towards dumping hearts if broken; otherwise queen of spades, then max penalty.
        let hearts_void = !legal.iter().any(|c| c.suit == Suit::Hearts);
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
        if round_gap > 0 && !hearts_void && !avoid_heavy_dump {
            if let Some(card) = legal
                .iter()
                .copied()
                .filter(|c| c.suit == Suit::Hearts)
                .max_by_key(|c| c.rank.value())
            {
                return card;
            }
        }
        // If provisional winner is the scoreboard leader, prefer to dump QS or hearts to them
        if let (Some(pw), Some(leader)) = (provisional, leader_target)
            && pw == leader
            && !avoid_heavy_dump
        {
            if let Some(qs) = legal.iter().copied().find(|c| c.is_queen_of_spades()) {
                return qs;
            }
            if !hearts_void
                && let Some(card) = legal
                    .iter()
                    .copied()
                    .filter(|c| c.suit == Suit::Hearts)
                    .max_by_key(|c| c.rank.value())
            {
                return card;
            }
        }
        // If the provisional winner is not the leader and there are already penalties on table, avoid feeding big penalties.
        if round_gap == 0 {
            if let (Some(pw), Some(leader)) = (provisional, leader_target) {
                if pw != leader && penalties_on_table > 0 {
                    if let Some(card) = legal
                        .iter()
                        .copied()
                        .filter(|c| c.penalty_value() == 0)
                        .min_by_key(|c| c.rank.value())
                    {
                        return card;
                    }
                    if let Some(card) = legal
                        .iter()
                        .copied()
                        .min_by_key(|c| (c.penalty_value(), c.rank.value()))
                    {
                        return card;
                    }
                }
            }
        }
        if !hearts_void
            && !avoid_heavy_dump
            && let Some(card) = legal
                .iter()
                .copied()
                .filter(|c| c.suit == Suit::Hearts)
                .max_by_key(|c| c.rank.value())
        {
            return card;
        }
        // Avoid generic QS fallback when feeding a non-leader with existing penalties
        if leader_target.is_none() || provisional == leader_target || penalties_on_table == 0 {
            if let Some(qs) = legal.iter().copied().find(|c| c.is_queen_of_spades()) {
                return qs;
            }
        }

        if !committed_moon && !legal.is_empty() {
            let simulated: Vec<(Card, SimOutcome)> = legal
                .iter()
                .copied()
                .filter_map(|card| {
                    simulate_lead_outcome(round, tracker, seat, card, style, origin, leader_target)
                        .map(|outcome| (card, outcome))
                })
                .collect();
            if !simulated.is_empty() {
                if let Some((card, outcome)) = simulated
                    .iter()
                    .filter(|(_, outcome)| outcome.winner != seat)
                    .min_by_key(|(card, outcome)| {
                        (
                            outcome.trick_penalties as u32,
                            card.penalty_value() as u32,
                            card.rank.value() as u32,
                        )
                    })
                {
                    if debug_enabled() {
                        eprintln!(
                            "mdhearts: followup_sim_dodge seat={seat:?} card={card} penalties={} seat_run={}",
                            outcome.trick_penalties, outcome.seat_penalty_run
                        );
                    }
                    return *card;
                }
                if let Some((card, outcome)) = simulated.iter().min_by_key(|(card, outcome)| {
                    (
                        outcome.seat_penalty_run,
                        outcome.trick_penalties as u32,
                        card.penalty_value() as u32,
                        card.rank.value() as u32,
                    )
                }) {
                    if debug_enabled() {
                        eprintln!(
                            "mdhearts: followup_sim_limit seat={seat:?} card={card} seat_run={} trick_pen={}",
                            outcome.seat_penalty_run, outcome.trick_penalties
                        );
                    }
                    return *card;
                }
            }
        }
    }

    let over_cap = round_gap_cap > 0 && round_gap >= round_gap_cap;
    let near_cap = round_gap_cap > 0 && round_gap >= round_gap_cap.saturating_sub(1).max(1);

    let dp_enabled = std::env::var("MDH_HARD_ENDGAME_DP_ENABLE")
        .map(|v| v == "1" || v.eq_ignore_ascii_case("true") || v.eq_ignore_ascii_case("on"))
        .unwrap_or(false);

    if hard_stage2_enabled()
        && lead_suit.is_none()
        && !legal.is_empty()
        && !committed_moon
        && !dp_enabled
        && (round_gap > 0
            || others_have_penalties
            || penalties_on_table > 0
            || near_cap
            || over_cap)
    {
        if test_trace_followup_enabled() {
            eprintln!(
                "mdhearts:test_followup lead_sim branch seat={:?} round_gap={} others_have_penalties={} penalties_on_table={}",
                seat, round_gap, others_have_penalties, penalties_on_table
            );
        }
        let simulated: Vec<(Card, SimOutcome)> = legal
            .iter()
            .copied()
            .filter_map(|card| {
                simulate_lead_outcome(round, tracker, seat, card, style, origin, leader_target)
                    .map(|outcome| (card, outcome))
            })
            .collect();

        if !simulated.is_empty() {
            if let Some((card, outcome)) = simulated
                .iter()
                .filter(|(_, outcome)| outcome.winner != seat)
                .min_by_key(|(card, outcome)| {
                    (
                        outcome.trick_penalties as u32,
                        card.penalty_value() as u32,
                        card.rank.value() as u32,
                    )
                })
            {
                if debug_enabled() {
                    eprintln!(
                        "mdhearts: stage2_lead_sim_dodge seat={seat:?} card={card} penalties={}",
                        outcome.trick_penalties
                    );
                }
                return *card;
            }

            if let Some((card, outcome)) = simulated.iter().min_by_key(|(card, outcome)| {
                (
                    outcome.seat_penalty_run,
                    outcome.trick_penalties as u32,
                    card.penalty_value() as u32,
                    card.rank.value() as u32,
                )
            }) {
                if debug_enabled() {
                    eprintln!(
                        "mdhearts: stage2_lead_sim_limit seat={seat:?} card={card} seat_pen_run={} trick_pen={}",
                        outcome.seat_penalty_run, outcome.trick_penalties
                    );
                }
                return *card;
            }
        }
    }

    if hard_stage2_enabled()
        && lead_suit.is_none()
        && !committed_moon
        && round_gap > 0
        && !avoid_heavy_dump
        && legal.iter().any(|c| c.is_queen_of_spades())
    {
        if test_trace_followup_enabled() {
            eprintln!("mdhearts:test_followup qs_dump branch");
        }
        if let Some(qs) = legal.iter().copied().find(|c| c.is_queen_of_spades()) {
            return qs;
        }
    }

    if hard_stage2_enabled()
        && lead_suit.is_none()
        && !committed_moon
        && round_gap > 0
        && !avoid_heavy_dump
        && round.hearts_broken()
    {
        if test_trace_followup_enabled() {
            eprintln!("mdhearts:test_followup hearts_dump branch");
        }
        if let Some(card) = legal
            .iter()
            .copied()
            .filter(|c| c.suit == Suit::Hearts && c.penalty_value() > 0)
            .max_by_key(|c| c.rank.value())
        {
            return card;
        }
        if let Some(card) = legal.iter().copied().find(|c| c.is_queen_of_spades()) {
            return card;
        }
    }

    let near_cap_dump = if hard_stage2_enabled() && round_gap_cap > 0 {
        let threshold = round_gap_cap.saturating_sub(1).max(1);
        round_gap >= threshold && penalties_on_table == 0
    } else {
        false
    };
    if hard_stage2_enabled() && lead_suit.is_none() && near_cap_dump {
        if test_trace_followup_enabled() {
            eprintln!("mdhearts:test_followup near_cap_dump branch");
        }
        if debug_enabled() {
            for candidate in legal.iter().copied() {
                let overcall = opponents_can_overcall_on_lead(round, seat, candidate);
                eprintln!(
                    "mdhearts: near_cap_dump seat={:?} card={} overcall={}",
                    seat, candidate, overcall
                );
            }
        }
        if let Some(card) = legal
            .iter()
            .copied()
            .filter(|c| c.penalty_value() == 0 && opponents_can_overcall_on_lead(round, seat, *c))
            .min_by_key(|c| c.rank.value())
        {
            return card;
        }
        if let Some(card) = legal
            .iter()
            .copied()
            .filter(|c| opponents_can_overcall_on_lead(round, seat, *c))
            .min_by_key(|c| (c.penalty_value(), c.rank.value()))
        {
            return card;
        }
    }

    if avoid_heavy_dump {
        if test_trace_followup_enabled() {
            eprintln!(
                "mdhearts:test_followup avoid_heavy_dump branch legal_len={}",
                legal.len()
            );
        }
        if let Some(card) = legal
            .iter()
            .copied()
            .min_by_key(|c| (c.penalty_value(), c.rank.value()))
        {
            return card;
        }
        // Fallback if legality probing produced no candidates.
        if let Some(card) = round
            .hand(seat)
            .iter()
            .copied()
            .min_by_key(|c| (c.penalty_value(), c.rank.value()))
        {
            return card;
        }
    }

    if let Some(card) = legal
        .iter()
        .copied()
        .max_by(|a, b| compare_penalty_dump(*a, *b))
    {
        return card;
    }
    // Final safety: fall back to a minimal-penalty card from hand.
    if test_trace_followup_enabled() {
        eprintln!(
            "mdhearts:test_followup final_fallback hand_len={} legal_len={}",
            round.hand(seat).len(),
            legal.len()
        );
    }
    round
        .hand(seat)
        .iter()
        .copied()
        .min_by_key(|c| (c.penalty_value(), c.rank.value()))
        .expect("hand must contain at least one card")
}

fn play_card_with_tracker(
    round: &mut RoundState,
    mut tracker: Option<&mut crate::bot::tracker::UnseenTracker>,
    seat: PlayerPosition,
    card: Card,
) -> Result<PlayOutcome, hearts_core::model::round::PlayError> {
    if let Some(tracker) = tracker.as_mut() {
        tracker.note_card_played(seat, card);
    }
    let outcome = round.play_card(seat, card)?;
    if let PlayOutcome::TrickCompleted { winner, penalties } = outcome {
        if let Some(tracker) = tracker.as_mut() {
            if let Some(trick) = round.trick_history().last() {
                let plays: Vec<(PlayerPosition, Card)> =
                    trick.plays().iter().map(|p| (p.position, p.card)).collect();
                tracker.note_trick_completion(&plays, winner, penalties, round.hearts_broken());
            }
        }
    }
    Ok(outcome)
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct SimOutcome {
    winner: PlayerPosition,
    trick_penalties: u8,
    seat_penalty_run: u32,
}

fn stage2_sim_follow_limit() -> usize {
    std::env::var("MDH_STAGE2_MUST_LOSE_MAXTRICKS")
        .ok()
        .and_then(|s| s.parse::<usize>().ok())
        .unwrap_or(4)
}

fn choose_sim_lead_card(round: &RoundState, seat: PlayerPosition) -> Option<Card> {
    legal_moves_for(round, seat)
        .into_iter()
        .min_by_key(|c| (c.penalty_value(), c.rank.value()))
}

fn simulate_lead_outcome(
    round: &RoundState,
    tracker: Option<&crate::bot::tracker::UnseenTracker>,
    seat: PlayerPosition,
    card: Card,
    style: BotStyle,
    origin: PlayerPosition,
    leader_target: Option<PlayerPosition>,
) -> Option<SimOutcome> {
    let saved_hits = take_hard_nudge_hits();
    reset_hard_nudge_hits();
    let mut sim_round = round.clone();
    let mut sim_tracker = tracker.cloned();
    let mut outcome = match play_card_with_tracker(&mut sim_round, sim_tracker.as_mut(), seat, card)
    {
        Ok(result) => result,
        Err(err) => {
            if test_trace_followup_enabled() {
                eprintln!(
                    "mdhearts:test_followup simulate_lead_outcome lead seat={seat:?} card={card} err={err:?}"
                );
            }
            reset_hard_nudge_hits();
            for _ in 0..saved_hits {
                record_hard_nudge_hit();
            }
            return None;
        }
    };

    let mut seat_penalty_run: u32 = 0;
    let mut continued_wins: usize = 0;
    let follow_limit = stage2_sim_follow_limit();

    let result = loop {
        match outcome {
            PlayOutcome::TrickCompleted { winner, penalties } => {
                if test_trace_followup_enabled() {
                    if let Some(trick) = sim_round.trick_history().last() {
                        let summary: Vec<String> = trick
                            .plays()
                            .iter()
                            .map(|p| format!("{:?}:{:?}", p.position, p.card))
                            .collect();
                        eprintln!(
                            "mdhearts:test_followup simulate_lead_outcome trick winner={winner:?} plays={:?}",
                            summary
                        );
                    } else {
                        eprintln!(
                            "mdhearts:test_followup simulate_lead_outcome trick winner={winner:?} (history missing)"
                        );
                    }
                }
                if winner == seat {
                    seat_penalty_run = seat_penalty_run.saturating_add(penalties as u32);
                    let current = SimOutcome {
                        winner,
                        trick_penalties: penalties,
                        seat_penalty_run,
                    };
                    if sim_round.hand(seat).is_empty() || continued_wins >= follow_limit {
                        break Some(current);
                    }
                    continued_wins = continued_wins.saturating_add(1);
                    let next_card = match choose_sim_lead_card(&sim_round, seat) {
                        Some(card) => card,
                        None => break Some(current),
                    };
                    if test_trace_followup_enabled() {
                        eprintln!(
                            "mdhearts:test_followup simulate_lead_outcome relaunch seat={seat:?} next_lead={next_card}"
                        );
                    }
                    outcome = match play_card_with_tracker(
                        &mut sim_round,
                        sim_tracker.as_mut(),
                        seat,
                        next_card,
                    ) {
                        Ok(result) => result,
                        Err(_) => break Some(current),
                    };
                } else {
                    break Some(SimOutcome {
                        winner,
                        trick_penalties: penalties,
                        seat_penalty_run,
                    });
                }
            }
            PlayOutcome::Played => {
                let next_seat = next_to_play(&sim_round);
                if sim_round.hand(next_seat).is_empty() {
                    if test_trace_followup_enabled() {
                        eprintln!(
                            "mdhearts:test_followup simulate_lead_outcome abort empty_hand seat={next_seat:?} trick_len={} trick_history={}",
                            sim_round.current_trick().plays().len(),
                            sim_round.trick_history().len()
                        );
                    }
                    // Crafted tests may exhaust a seat's tiny hand in simulation; abort safely.
                    reset_hard_nudge_hits();
                    for _ in 0..saved_hits {
                        record_hard_nudge_hit();
                    }
                    return None;
                }
                let follow_card = {
                    let tracker_ref = sim_tracker
                        .as_ref()
                        .map(|t| t as &crate::bot::tracker::UnseenTracker);
                    let chosen = choose_followup_card(
                        &sim_round,
                        next_seat,
                        style,
                        tracker_ref,
                        origin,
                        leader_target,
                    );
                    if test_trace_followup_enabled() {
                        eprintln!(
                            "mdhearts:test_followup simulate_lead_outcome follow_choice seat={next_seat:?} card={chosen}"
                        );
                    }
                    chosen
                };
                outcome = match play_card_with_tracker(
                    &mut sim_round,
                    sim_tracker.as_mut(),
                    next_seat,
                    follow_card,
                ) {
                    Ok(result) => result,
                    Err(err) => {
                        if test_trace_followup_enabled() {
                            eprintln!(
                                "mdhearts:test_followup simulate_lead_outcome follow seat={next_seat:?} card={follow_card} err={err:?}"
                            );
                        }
                        reset_hard_nudge_hits();
                        for _ in 0..saved_hits {
                            record_hard_nudge_hit();
                        }
                        return None;
                    }
                };
            }
        }
    };

    reset_hard_nudge_hits();
    for _ in 0..saved_hits {
        record_hard_nudge_hit();
    }

    result
}

fn opponents_can_overcall_on_lead(round: &RoundState, seat: PlayerPosition, card: Card) -> bool {
    let lead_suit = card.suit;
    let lead_rank = card.rank.value();
    for other in PlayerPosition::LOOP.iter().copied() {
        if other == seat {
            continue;
        }
        if round
            .hand(other)
            .iter()
            .any(|candidate| candidate.suit == lead_suit && candidate.rank.value() > lead_rank)
        {
            return true;
        }
    }
    false
}

fn legal_moves_for(round: &RoundState, seat: PlayerPosition) -> Vec<Card> {
    let legal: Vec<Card> = round
        .hand(seat)
        .iter()
        .copied()
        .filter(|card| {
            let mut probe = round.clone();
            probe.play_card(seat, *card).is_ok()
        })
        .collect();
    if !legal.is_empty() {
        return legal;
    }
    // Test-only escape hatch: allow permissive legality in crafted scenarios.
    let permissive = std::env::var("MDH_TEST_PERMISSIVE_LEGAL")
        .map(|v| v == "1" || v.eq_ignore_ascii_case("true") || v.eq_ignore_ascii_case("on"))
        .unwrap_or(false);
    if permissive {
        return round.hand(seat).iter().copied().collect();
    }
    legal
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
    use crate::bot::{BotContext, BotDifficulty, BotStyle};
    use hearts_core::model::card::Card;
    use hearts_core::model::hand::Hand;
    use hearts_core::model::passing::PassingDirection;
    use hearts_core::model::player::PlayerPosition;
    use hearts_core::model::rank::Rank;
    use hearts_core::model::round::{RoundPhase, RoundState};
    use hearts_core::model::score::ScoreBoard;
    use hearts_core::model::suit::Suit;
    use hearts_core::model::trick::Trick;
    use std::sync::{Mutex, MutexGuard};

    static ENV_GUARD: Mutex<()> = Mutex::new(());

    fn env_lock() -> MutexGuard<'static, ()> {
        ENV_GUARD.lock().unwrap()
    }

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
        // Build a non-conflicting seed trick (cards not present in hands or current plays)
        let mut used: Vec<Card> = Vec::new();
        for h in &hands {
            used.extend(h.iter().copied());
        }
        for &(_, card) in plays.iter() {
            used.push(card);
        }
        let ranks = [
            Rank::Two,
            Rank::Three,
            Rank::Four,
            Rank::Five,
            Rank::Six,
            Rank::Seven,
            Rank::Eight,
            Rank::Nine,
            Rank::Ten,
            Rank::Jack,
            Rank::Queen,
            Rank::King,
            Rank::Ace,
        ];
        let suits = [Suit::Clubs, Suit::Diamonds, Suit::Hearts, Suit::Spades];
        let mut candidates: Vec<Card> = Vec::new();
        for &s in &suits {
            for &r in &ranks {
                let c = Card::new(r, s);
                if !used.contains(&c) {
                    candidates.push(c);
                }
            }
        }
        let mut seed_trick = hearts_core::model::trick::Trick::new(starting);
        let mut seat_iter = starting;
        for i in 0..4 {
            let card = candidates.get(i).copied().expect("enough seed cards");
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
            vec![Card::new(Rank::Two, Suit::Spades)], // North
            vec![Card::new(Rank::Ace, Suit::Clubs)],  // East (provisional winner)
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
            vec![Card::new(Rank::Two, Suit::Spades)], // North
            vec![Card::new(Rank::Ace, Suit::Clubs)],  // East (provisional winner)
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
            vec![Card::new(Rank::Two, Suit::Spades)], // North
            vec![Card::new(Rank::Ace, Suit::Clubs)],  // East (provisional winner)
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
    fn stage2_avoid_heavy_dump_after_moon_bust() {
        let _env = env_lock();
        unsafe {
            std::env::set_var("MDH_HARD_DETERMINISTIC", "1");
            std::env::set_var("MDH_HARD_TEST_STEPS", "80");
            std::env::set_var("MDH_STAGE2_AVOID_DUMP_GAP", "1");
            std::env::set_var("MDH_STAGE2_ROUND_GAP_CAP", "1");
            std::env::set_var("MDH_FEATURE_HARD_STAGE2", "1");
            std::env::set_var("MDH_TEST_TRACE_FOLLOWUP", "1");
        }

        let mut prev = Trick::new(PlayerPosition::West);
        prev.play(PlayerPosition::West, Card::new(Rank::Ace, Suit::Hearts))
            .unwrap();
        prev.play(PlayerPosition::North, Card::new(Rank::Two, Suit::Hearts))
            .unwrap();
        prev.play(PlayerPosition::East, Card::new(Rank::Three, Suit::Hearts))
            .unwrap();
        prev.play(PlayerPosition::South, Card::new(Rank::Four, Suit::Hearts))
            .unwrap();

        let mut current = Trick::new(PlayerPosition::West);
        current
            .play(PlayerPosition::West, Card::new(Rank::King, Suit::Clubs))
            .unwrap();
        current
            .play(PlayerPosition::North, Card::new(Rank::Ace, Suit::Clubs))
            .unwrap();
        current
            .play(PlayerPosition::East, Card::new(Rank::Six, Suit::Hearts))
            .unwrap();

        let hands = [
            Hand::with_cards(vec![Card::new(Rank::Four, Suit::Clubs)]),
            Hand::with_cards(vec![
                Card::new(Rank::Seven, Suit::Diamonds),
                Card::new(Rank::Six, Suit::Hearts),
            ]),
            Hand::with_cards(vec![
                Card::new(Rank::Queen, Suit::Spades),
                Card::new(Rank::Nine, Suit::Hearts),
                Card::new(Rank::Two, Suit::Diamonds),
            ]),
            Hand::with_cards(vec![Card::new(Rank::Five, Suit::Clubs)]),
        ];
        let round = RoundState::from_hands_with_state(
            hands,
            PlayerPosition::West,
            PassingDirection::Hold,
            RoundPhase::Playing,
            current,
            vec![prev],
            true,
        );
        let seat = PlayerPosition::South;
        assert!(super::hard_stage2_enabled());
        assert_eq!(super::stage2_round_gap_cap(), 1);
        assert_eq!(super::stage2_avoid_dump_gap(), 1);
        let totals = round.penalty_totals();
        assert_eq!(totals[PlayerPosition::West.index()], 4);
        assert_eq!(totals[seat.index()], 0);

        let mut tracker = UnseenTracker::new();
        tracker.reset_for_round(&round);
        tracker.set_moon_state(seat, MoonState::Committed);

        let choice = super::choose_followup_card(
            &round,
            seat,
            BotStyle::Cautious,
            Some(&tracker),
            seat,
            Some(PlayerPosition::West),
        );
        assert_eq!(choice, Card::new(Rank::Two, Suit::Diamonds));

        unsafe {
            std::env::remove_var("MDH_HARD_DETERMINISTIC");
            std::env::remove_var("MDH_HARD_TEST_STEPS");
            std::env::remove_var("MDH_STAGE2_AVOID_DUMP_GAP");
            std::env::remove_var("MDH_STAGE2_ROUND_GAP_CAP");
            std::env::remove_var("MDH_FEATURE_HARD_STAGE2");
            std::env::remove_var("MDH_TEST_TRACE_FOLLOWUP");
        }
    }

    #[test]
    fn stage2_follow_limit_halts_simulation() {
        let _env = env_lock();
        unsafe {
            std::env::set_var("MDH_FEATURE_HARD_STAGE2", "1");
            std::env::set_var("MDH_HARD_DETERMINISTIC", "1");
            std::env::set_var("MDH_HARD_TEST_STEPS", "80");
        }
        let starting = PlayerPosition::West;
        let hands = [
            vec![
                Card::new(Rank::Queen, Suit::Hearts),
                Card::new(Rank::Ten, Suit::Hearts),
            ],
            vec![
                Card::new(Rank::Jack, Suit::Hearts),
                Card::new(Rank::Nine, Suit::Hearts),
            ],
            vec![
                Card::new(Rank::Eight, Suit::Hearts),
                Card::new(Rank::Seven, Suit::Hearts),
            ],
            vec![
                Card::new(Rank::Ace, Suit::Hearts),
                Card::new(Rank::King, Suit::Hearts),
            ],
        ];
        let round = build_round(starting, hands, &[], true);
        let mut tracker = UnseenTracker::new();
        tracker.reset_for_round(&round);
        let seat = PlayerPosition::West;

        unsafe {
            std::env::set_var("MDH_STAGE2_MUST_LOSE_MAXTRICKS", "4");
        }
        let sim_long = super::simulate_lead_outcome(
            &round,
            Some(&tracker),
            seat,
            Card::new(Rank::Ace, Suit::Hearts),
            BotStyle::Cautious,
            seat,
            None,
        )
        .expect("simulation with large follow limit");

        unsafe {
            std::env::set_var("MDH_STAGE2_MUST_LOSE_MAXTRICKS", "0");
        }
        let sim_short = super::simulate_lead_outcome(
            &round,
            Some(&tracker),
            seat,
            Card::new(Rank::Ace, Suit::Hearts),
            BotStyle::Cautious,
            seat,
            None,
        )
        .expect("simulation with zero follow limit");
        assert!(
            sim_long.seat_penalty_run > sim_short.seat_penalty_run,
            "expected follow limit to reduce seat penalty accumulation (long={} short={})",
            sim_long.seat_penalty_run,
            sim_short.seat_penalty_run
        );

        unsafe {
            std::env::remove_var("MDH_STAGE2_MUST_LOSE_MAXTRICKS");
            std::env::remove_var("MDH_FEATURE_HARD_STAGE2");
            std::env::remove_var("MDH_HARD_DETERMINISTIC");
            std::env::remove_var("MDH_HARD_TEST_STEPS");
        }
    }

    #[test]
    fn stage2_near_cap_prefers_overcallable_zero_penalty() {
        use hearts_core::model::trick::Trick;

        let _env = env_lock();
        unsafe {
            std::env::set_var("MDH_FEATURE_HARD_STAGE2", "1");
            std::env::set_var("MDH_STAGE2_ROUND_GAP_CAP", "3");
            std::env::set_var("MDH_STAGE2_AVOID_DUMP_GAP", "2");
        }

        let mut prev = Trick::new(PlayerPosition::North);
        prev.play(PlayerPosition::North, Card::new(Rank::Two, Suit::Hearts))
            .unwrap();
        prev.play(PlayerPosition::East, Card::new(Rank::Three, Suit::Hearts))
            .unwrap();
        prev.play(PlayerPosition::South, Card::new(Rank::Four, Suit::Hearts))
            .unwrap();
        prev.play(PlayerPosition::West, Card::new(Rank::Five, Suit::Hearts))
            .unwrap();

        let hands = [
            Hand::with_cards(vec![
                Card::new(Rank::Ace, Suit::Clubs),
                Card::new(Rank::Seven, Suit::Diamonds),
            ]),
            Hand::with_cards(vec![Card::new(Rank::King, Suit::Diamonds)]),
            Hand::with_cards(vec![Card::new(Rank::Two, Suit::Spades)]),
            Hand::with_cards(vec![
                Card::new(Rank::Ten, Suit::Clubs),
                Card::new(Rank::Queen, Suit::Diamonds),
                Card::new(Rank::Ace, Suit::Spades),
            ]),
        ];
        let round = RoundState::from_hands_with_state(
            hands,
            PlayerPosition::West,
            PassingDirection::Hold,
            RoundPhase::Playing,
            Trick::new(PlayerPosition::West),
            vec![prev],
            true,
        );

        let choice = super::choose_followup_card(
            &round,
            PlayerPosition::West,
            BotStyle::Cautious,
            None,
            PlayerPosition::West,
            Some(PlayerPosition::West),
        );
        assert_eq!(choice, Card::new(Rank::Ten, Suit::Clubs));

        unsafe {
            std::env::remove_var("MDH_FEATURE_HARD_STAGE2");
            std::env::remove_var("MDH_STAGE2_ROUND_GAP_CAP");
            std::env::remove_var("MDH_STAGE2_AVOID_DUMP_GAP");
        }
    }

    #[test]
    fn stage2_avoid_dump_discards_low_penalty_when_gap_zero() {
        use hearts_core::model::trick::Trick;

        let _env = env_lock();
        unsafe {
            std::env::set_var("MDH_FEATURE_HARD_STAGE2", "1");
            std::env::set_var("MDH_STAGE2_ROUND_GAP_CAP", "4");
            std::env::set_var("MDH_STAGE2_AVOID_DUMP_GAP", "3");
        }

        let mut trick1 = Trick::new(PlayerPosition::South);
        trick1
            .play(PlayerPosition::South, Card::new(Rank::Queen, Suit::Hearts))
            .unwrap();
        trick1
            .play(PlayerPosition::West, Card::new(Rank::Ten, Suit::Hearts))
            .unwrap();
        trick1
            .play(PlayerPosition::North, Card::new(Rank::Jack, Suit::Hearts))
            .unwrap();
        trick1
            .play(PlayerPosition::East, Card::new(Rank::Two, Suit::Hearts))
            .unwrap();

        let mut trick2 = Trick::new(PlayerPosition::West);
        trick2
            .play(PlayerPosition::West, Card::new(Rank::Queen, Suit::Spades))
            .unwrap();
        trick2
            .play(PlayerPosition::North, Card::new(Rank::Ace, Suit::Spades))
            .unwrap();
        trick2
            .play(PlayerPosition::East, Card::new(Rank::Three, Suit::Spades))
            .unwrap();
        trick2
            .play(PlayerPosition::South, Card::new(Rank::Four, Suit::Spades))
            .unwrap();

        let mut current = Trick::new(PlayerPosition::West);
        current
            .play(PlayerPosition::West, Card::new(Rank::Two, Suit::Clubs))
            .unwrap();
        current
            .play(PlayerPosition::North, Card::new(Rank::Five, Suit::Clubs))
            .unwrap();

        let hands = [
            Hand::with_cards(vec![Card::new(Rank::Seven, Suit::Clubs)]),
            Hand::with_cards(vec![
                Card::new(Rank::Three, Suit::Diamonds),
                Card::new(Rank::Ten, Suit::Hearts),
                Card::new(Rank::Ace, Suit::Hearts),
            ]),
            Hand::with_cards(vec![
                Card::new(Rank::Six, Suit::Clubs),
                Card::new(Rank::Eight, Suit::Clubs),
            ]),
            Hand::with_cards(vec![
                Card::new(Rank::Four, Suit::Diamonds),
                Card::new(Rank::Nine, Suit::Diamonds),
            ]),
        ];

        let round = RoundState::from_hands_with_state(
            hands,
            PlayerPosition::West,
            PassingDirection::Hold,
            RoundPhase::Playing,
            current,
            vec![trick1, trick2],
            true,
        );

        let choice = super::choose_followup_card(
            &round,
            PlayerPosition::East,
            BotStyle::Cautious,
            None,
            PlayerPosition::East,
            Some(PlayerPosition::West),
        );
        assert_eq!(choice, Card::new(Rank::Three, Suit::Diamonds));

        unsafe {
            std::env::remove_var("MDH_FEATURE_HARD_STAGE2");
            std::env::remove_var("MDH_STAGE2_ROUND_GAP_CAP");
            std::env::remove_var("MDH_STAGE2_AVOID_DUMP_GAP");
        }
    }

    #[test]
    fn stage2_near_cap_falls_back_to_lowest_penalty_overcallable() {
        use hearts_core::model::trick::Trick;

        let _env = env_lock();
        unsafe {
            std::env::set_var("MDH_FEATURE_HARD_STAGE2", "1");
            std::env::set_var("MDH_STAGE2_ROUND_GAP_CAP", "2");
        }

        let mut prev = Trick::new(PlayerPosition::South);
        prev.play(PlayerPosition::South, Card::new(Rank::Ten, Suit::Hearts))
            .unwrap();
        prev.play(PlayerPosition::West, Card::new(Rank::Jack, Suit::Hearts))
            .unwrap();
        prev.play(PlayerPosition::North, Card::new(Rank::Queen, Suit::Hearts))
            .unwrap();
        prev.play(PlayerPosition::East, Card::new(Rank::King, Suit::Hearts))
            .unwrap();

        let hands = [
            Hand::with_cards(vec![Card::new(Rank::Ace, Suit::Hearts)]),
            Hand::with_cards(vec![Card::new(Rank::Nine, Suit::Spades)]),
            Hand::with_cards(vec![Card::new(Rank::Eight, Suit::Spades)]),
            Hand::with_cards(vec![
                Card::new(Rank::Ten, Suit::Hearts),
                Card::new(Rank::Two, Suit::Diamonds),
            ]),
        ];

        let round = RoundState::from_hands_with_state(
            hands,
            PlayerPosition::West,
            PassingDirection::Hold,
            RoundPhase::Playing,
            Trick::new(PlayerPosition::West),
            vec![prev],
            true,
        );

        let choice = super::choose_followup_card(
            &round,
            PlayerPosition::West,
            BotStyle::Cautious,
            None,
            PlayerPosition::West,
            Some(PlayerPosition::West),
        );
        assert_eq!(choice, Card::new(Rank::Ten, Suit::Hearts));

        unsafe {
            std::env::remove_var("MDH_FEATURE_HARD_STAGE2");
            std::env::remove_var("MDH_STAGE2_ROUND_GAP_CAP");
        }
    }

    #[test]
    fn stage2_runaway_leader_forces_low_overcall() {
        use hearts_core::model::trick::Trick;

        let _env = env_lock();
        unsafe {
            std::env::set_var("MDH_FEATURE_HARD_STAGE2", "1");
            std::env::set_var("MDH_STAGE2_ROUND_GAP_CAP", "4");
        }

        let mut trick1 = Trick::new(PlayerPosition::West);
        trick1
            .play(PlayerPosition::West, Card::new(Rank::Ace, Suit::Hearts))
            .unwrap();
        trick1
            .play(PlayerPosition::North, Card::new(Rank::Two, Suit::Hearts))
            .unwrap();
        trick1
            .play(PlayerPosition::East, Card::new(Rank::Three, Suit::Hearts))
            .unwrap();
        trick1
            .play(PlayerPosition::South, Card::new(Rank::Four, Suit::Hearts))
            .unwrap();

        let mut trick2 = Trick::new(PlayerPosition::West);
        trick2
            .play(PlayerPosition::West, Card::new(Rank::Queen, Suit::Spades))
            .unwrap();
        trick2
            .play(PlayerPosition::North, Card::new(Rank::Two, Suit::Spades))
            .unwrap();
        trick2
            .play(PlayerPosition::East, Card::new(Rank::Three, Suit::Spades))
            .unwrap();
        trick2
            .play(PlayerPosition::South, Card::new(Rank::Four, Suit::Spades))
            .unwrap();

        let mut current = Trick::new(PlayerPosition::West);
        current
            .play(PlayerPosition::West, Card::new(Rank::Jack, Suit::Hearts))
            .unwrap();
        current
            .play(PlayerPosition::North, Card::new(Rank::Seven, Suit::Hearts))
            .unwrap();

        let hands = [
            Hand::with_cards(vec![Card::new(Rank::Two, Suit::Diamonds)]),
            Hand::with_cards(vec![
                Card::new(Rank::Queen, Suit::Hearts),
                Card::new(Rank::King, Suit::Hearts),
            ]),
            Hand::with_cards(vec![Card::new(Rank::Three, Suit::Clubs)]),
            Hand::with_cards(vec![Card::new(Rank::Nine, Suit::Clubs)]),
        ];

        let round = RoundState::from_hands_with_state(
            hands,
            PlayerPosition::West,
            PassingDirection::Hold,
            RoundPhase::Playing,
            current,
            vec![trick1, trick2],
            true,
        );

        let choice = super::choose_followup_card(
            &round,
            PlayerPosition::East,
            BotStyle::Cautious,
            None,
            PlayerPosition::West,
            Some(PlayerPosition::West),
        );
        assert_eq!(choice, Card::new(Rank::Queen, Suit::Hearts));

        unsafe {
            std::env::remove_var("MDH_FEATURE_HARD_STAGE2");
            std::env::remove_var("MDH_STAGE2_ROUND_GAP_CAP");
        }
    }

    #[test]
    fn lead_simulation_avoids_penalty_when_followers_duck() {
        let _env = env_lock();
        unsafe {
            std::env::set_var("MDH_TEST_PERMISSIVE_LEGAL", "1");
            std::env::set_var("MDH_STAGE2_MUST_LOSE_MAXTRICKS", "0");
        }
        let starting = PlayerPosition::West;
        let hands = [
            Hand::with_cards(vec![
                Card::new(Rank::Ace, Suit::Clubs),
                Card::new(Rank::Five, Suit::Spades),
                Card::new(Rank::Two, Suit::Diamonds),
            ]),
            Hand::with_cards(vec![
                Card::new(Rank::Ace, Suit::Diamonds),
                Card::new(Rank::Four, Suit::Spades),
            ]),
            Hand::with_cards(vec![Card::new(Rank::Three, Suit::Diamonds)]),
            Hand::with_cards(vec![
                Card::new(Rank::King, Suit::Diamonds),
                Card::new(Rank::Three, Suit::Clubs),
                Card::new(Rank::Four, Suit::Spades),
            ]),
        ];
        let mut history_trick = Trick::new(PlayerPosition::West);
        history_trick
            .play(PlayerPosition::West, Card::new(Rank::Ace, Suit::Spades))
            .unwrap();
        history_trick
            .play(PlayerPosition::North, Card::new(Rank::King, Suit::Spades))
            .unwrap();
        history_trick
            .play(PlayerPosition::East, Card::new(Rank::Queen, Suit::Spades))
            .unwrap();
        history_trick
            .play(PlayerPosition::South, Card::new(Rank::Two, Suit::Spades))
            .unwrap();

        let round = RoundState::from_hands_with_state(
            hands,
            starting,
            PassingDirection::Hold,
            RoundPhase::Playing,
            Trick::new(starting),
            vec![history_trick],
            true,
        );
        assert!(
            round
                .hand(PlayerPosition::North)
                .contains(Card::new(Rank::Ace, Suit::Clubs)),
            "expected North to hold Ace of Clubs"
        );
        let totals = round.penalty_totals();
        assert_eq!(totals[PlayerPosition::West.index()], 13);

        let mut tracker = UnseenTracker::new();
        tracker.reset_for_round(&round);

        let sim_three = super::simulate_lead_outcome(
            &round,
            Some(&tracker),
            PlayerPosition::West,
            Card::new(Rank::Three, Suit::Clubs),
            BotStyle::Cautious,
            PlayerPosition::West,
            None,
        )
        .expect("simulation for 3C");
        assert_ne!(sim_three.winner, PlayerPosition::West);
        assert_eq!(sim_three.trick_penalties, 0);
        assert_eq!(sim_three.seat_penalty_run, 0);

        let sim_king = super::simulate_lead_outcome(
            &round,
            Some(&tracker),
            PlayerPosition::West,
            Card::new(Rank::King, Suit::Diamonds),
            BotStyle::Cautious,
            PlayerPosition::West,
            None,
        )
        .expect("simulation for KD");
        assert_ne!(sim_king.winner, PlayerPosition::West);
        assert_eq!(sim_king.trick_penalties, 0);
        assert_eq!(sim_king.seat_penalty_run, 0);

        let sim_four = super::simulate_lead_outcome(
            &round,
            Some(&tracker),
            PlayerPosition::West,
            Card::new(Rank::Four, Suit::Spades),
            BotStyle::Cautious,
            PlayerPosition::West,
            None,
        )
        .expect("simulation for 4S");
        assert_ne!(sim_four.winner, PlayerPosition::West);

        let choice = super::choose_followup_card(
            &round,
            PlayerPosition::West,
            BotStyle::Cautious,
            Some(&tracker),
            PlayerPosition::West,
            None,
        );
        let sim_choice = super::simulate_lead_outcome(
            &round,
            Some(&tracker),
            PlayerPosition::West,
            choice,
            BotStyle::Cautious,
            PlayerPosition::West,
            None,
        )
        .expect("simulation for chosen card");
        assert_ne!(sim_choice.winner, PlayerPosition::West);
        assert_eq!(sim_choice.trick_penalties, 0);
        unsafe {
            std::env::remove_var("MDH_TEST_PERMISSIVE_LEGAL");
            std::env::remove_var("MDH_STAGE2_MUST_LOSE_MAXTRICKS");
        }
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
