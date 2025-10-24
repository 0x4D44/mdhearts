use super::{BotContext, PlayPlanner, snapshot_scores};
use hearts_core::model::card::Card;
use hearts_core::model::player::PlayerPosition;
use hearts_core::model::round::{PlayOutcome, RoundState};
use hearts_core::model::suit::Suit;
use once_cell::sync::Lazy;
use std::sync::Mutex;
use std::time::Instant;
use std::cell::Cell;

// Stage 3 scaffold: Hard planner with configurable branch limit and (future) depth/time caps.
// For now, it orders by heuristic and considers the top N branches.
pub struct PlayPlannerHard;

#[derive(Debug, Clone, Copy)]
#[allow(dead_code)]
pub struct SearchConfig {
    pub branch_limit: usize,
    pub max_depth: u8,    // reserved for future two-ply
    pub time_cap_ms: u32, // reserved for future wall clock cap
    pub next_branch_limit: usize,
    pub early_cutoff_margin: i32,
}

impl Default for SearchConfig {
    fn default() -> Self {
        Self {
            branch_limit: 6,
            max_depth: 1,
            time_cap_ms: 10,
            next_branch_limit: 3,
            early_cutoff_margin: 300,
        }
    }
}

impl PlayPlannerHard {
    fn config() -> SearchConfig {
        // Allow env override of branch limit in early scaffolding.
        let bl = std::env::var("MDH_HARD_BRANCH_LIMIT")
            .ok()
            .and_then(|s| s.parse::<usize>().ok())
            .unwrap_or(6);
        let cap = std::env::var("MDH_HARD_TIME_CAP_MS")
            .ok()
            .and_then(|s| s.parse::<u32>().ok())
            .unwrap_or(10);
        let nbl = std::env::var("MDH_HARD_NEXT_BRANCH_LIMIT")
            .ok()
            .and_then(|s| s.parse::<usize>().ok())
            .unwrap_or(3);
        let cutoff = std::env::var("MDH_HARD_EARLY_CUTOFF_MARGIN")
            .ok()
            .and_then(|s| s.parse::<i32>().ok())
            .unwrap_or(300);
        SearchConfig {
            branch_limit: bl,
            time_cap_ms: cap,
            next_branch_limit: nbl,
            early_cutoff_margin: cutoff,
            ..SearchConfig::default()
        }
    }

    pub fn choose(legal: &[Card], ctx: &BotContext<'_>) -> Option<Card> {
        if legal.is_empty() {
            return None;
        }
        let cfg = Self::config();
        let deterministic = std::env::var("MDH_HARD_DETERMINISTIC")
            .map(|v| v == "1" || v.eq_ignore_ascii_case("true") || v.eq_ignore_ascii_case("on"))
            .unwrap_or(false);
        let step_cap = std::env::var("MDH_HARD_TEST_STEPS").ok().and_then(|s| s.parse::<usize>().ok());
        // Rank by heuristic (fast), keep top-N, then apply a tiny continuation bonus via a 1-ply trick rollout.
        let mut explained = PlayPlanner::explain_candidates(legal, ctx);
        explained.sort_by(|a, b| b.1.cmp(&a.1));
        let best_base = explained.first().map(|x| x.1).unwrap_or(0);
        let boost_gap = parse_env_i32("MDH_HARD_CONT_BOOST_GAP").unwrap_or(0);
        let boost_factor = parse_env_i32("MDH_HARD_CONT_BOOST_FACTOR").unwrap_or(1);
        let snapshot = snapshot_scores(ctx.scores);
        let mut best: Option<(Card, i32)> = None;
        let start = Instant::now();
        let mut budget = Budget::new(cfg.time_cap_ms, deterministic, step_cap);
        let mut scanned = 0usize;
        let (tier, leverage_score, limits) = effective_limits(ctx);
        let phaseb_topk = limits.phaseb_topk;
        let ab_margin = limits.ab_margin;
        let mut phase_b_candidates = 0usize;
        let mut phase_c_probes = 0usize;
        // capture tiny-next3 counter delta for this decision
        let n3_before = NEXT3_TINY_COUNT.with(|c| c.get());
        let mut iter = explained.into_iter().take(cfg.branch_limit).enumerate().peekable();
        while let Some((idx, (card, base))) = iter.next() {
            if budget.should_stop() {
                break;
            }
            if ab_margin > 0 {
                if let Some((_, alpha)) = best {
                    if base + ab_margin < alpha {
                        if debug_enabled() {
                            eprintln!("mdhearts: hard ab-skip {} base={} < alpha-{}", card, base, ab_margin);
                        }
                        scanned += 1;
                        budget.tick();
                        continue;
                    }
                }
            }
            let cont_raw = if phaseb_topk == 0 || idx < phaseb_topk {
                phase_b_candidates = phase_b_candidates.saturating_add(1);
                let before = budget.probe_calls;
                let allow_next3 = matches!(tier, Tier::Wide);
                let v = Self::rollout_current_trick(card, ctx, snapshot.max_player, &mut budget, start, allow_next3);
                phase_c_probes += budget.probe_calls.saturating_sub(before);
                v
            } else { 0 };
            let cont = if boost_gap > 0 && boost_factor > 1 && (best_base - base) <= boost_gap {
                cont_raw.saturating_mul(boost_factor)
            } else {
                cont_raw
            };
            let total = base + cont;
            if debug_enabled() {
                eprintln!(
                    "mdhearts: hard cand {} base={} cont={} total={}",
                    card, base, cont, total
                );
            }
            match best {
                None => best = Some((card, total)),
                Some((bc, bs)) => {
                    if total > bs {
                        best = Some((card, total));
                    } else if total == bs
                        && (card.suit as u8, card.rank.value()) < (bc.suit as u8, bc.rank.value())
                    {
                        best = Some((card, total));
                    }
                }
            }
            scanned += 1;
            budget.tick();
            // Early cutoff: if the next base score cannot overcome our current best even with a safety margin, stop.
            if let Some((_, best_total)) = best {
                if let Some((_, (next_card, next_base))) = iter.peek() {
                    let safe_cap = weights().cont_cap;
                    let safety_margin = if safe_cap > 0 { safe_cap } else { cfg.early_cutoff_margin };
                    if *next_base + safety_margin < best_total {
                        if debug_enabled() {
                            eprintln!(
                                "mdhearts: hard early cutoff at candidate {} (next_base={} + margin {} < best_total={})",
                                next_card, next_base, safety_margin, best_total
                            );
                        }
                        break;
                    }
                }
            }
            if budget.timed_out(start) {
                if debug_enabled() {
                    eprintln!(
                        "mdhearts: hard cap reached ({} ms), scanned {}",
                        cfg.time_cap_ms, scanned
                    );
                }
                break;
            }
        }
        let utilization = budget.utilization_percent(start);
        let n3_after = NEXT3_TINY_COUNT.with(|c| c.get());
        set_last_stats(Stats {
            scanned,
            elapsed_ms: start.elapsed().as_millis() as u32,
            scanned_phase_a: scanned.saturating_sub(phase_b_candidates),
            scanned_phase_b: phase_b_candidates,
            scanned_phase_c: phase_c_probes,
            leverage_score,
            tier,
            limits_in_effect: limits,
            utilization,
            wide_boost_feed_permil: parse_env_i32("MDH_HARD_WIDE_PERMIL_BOOST_FEED").unwrap_or(0),
            wide_boost_self_permil: parse_env_i32("MDH_HARD_WIDE_PERMIL_BOOST_SELFCAP").unwrap_or(0),
            cont_cap: weights().cont_cap,
            next3_tiny_hits: n3_after.saturating_sub(n3_before),
        });
        best.map(|(c, _)| c)
    }

    pub fn explain_candidates(legal: &[Card], ctx: &BotContext<'_>) -> Vec<(Card, i32)> {
        let cfg = Self::config();
        let deterministic = std::env::var("MDH_HARD_DETERMINISTIC")
            .map(|v| v == "1" || v.eq_ignore_ascii_case("true") || v.eq_ignore_ascii_case("on"))
            .unwrap_or(false);
        let step_cap = std::env::var("MDH_HARD_TEST_STEPS").ok().and_then(|s| s.parse::<usize>().ok());
        let mut v = PlayPlanner::explain_candidates(legal, ctx);
        v.sort_by(|a, b| b.1.cmp(&a.1));
        let best_base = v.first().map(|x| x.1).unwrap_or(0);
        let boost_gap = parse_env_i32("MDH_HARD_CONT_BOOST_GAP").unwrap_or(0);
        let boost_factor = parse_env_i32("MDH_HARD_CONT_BOOST_FACTOR").unwrap_or(1);
        let snapshot = snapshot_scores(ctx.scores);
        let start = Instant::now();
        let mut budget = Budget::new(cfg.time_cap_ms, deterministic, step_cap);
        let mut out = Vec::new();
        let (tier, leverage_score, limits) = effective_limits(ctx);
        let phaseb_topk = limits.phaseb_topk;
        let mut scanned = 0usize;
        let mut phase_b_candidates = 0usize;
        let mut phase_c_probes = 0usize;
        for (idx, (card, base)) in v.into_iter().take(cfg.branch_limit).enumerate() {
            if budget.should_stop() { break; }
            let cont_raw = if phaseb_topk == 0 || idx < phaseb_topk {
                phase_b_candidates = phase_b_candidates.saturating_add(1);
                let before = budget.probe_calls;
                let v = Self::rollout_current_trick(card, ctx, snapshot.max_player, &mut budget, start, false);
                phase_c_probes += budget.probe_calls.saturating_sub(before);
                v 
            } else { 0 };
            let cont = if boost_gap > 0 && boost_factor > 1 && (best_base - base) <= boost_gap {
                cont_raw.saturating_mul(boost_factor)
            } else {
                cont_raw
            };
            let total = base + cont;
            if debug_enabled() {
                eprintln!(
                    "mdhearts: hard explain {} base={} cont={} total={}",
                    card, base, cont, total
                );
            }
            out.push((card, total));
            scanned += 1;
            budget.tick();
            if budget.timed_out(start) {
                if debug_enabled() {
                    eprintln!(
                        "mdhearts: hard explain cap reached ({} ms)",
                        cfg.time_cap_ms
                    );
                }
                break;
            }
        }
        let utilization = budget.utilization_percent(start);
        set_last_stats(Stats {
            scanned,
            elapsed_ms: start.elapsed().as_millis() as u32,
            scanned_phase_a: scanned.saturating_sub(phase_b_candidates),
            scanned_phase_b: phase_b_candidates,
            scanned_phase_c: phase_c_probes,
            leverage_score,
            tier,
            limits_in_effect: limits,
            utilization,
            wide_boost_feed_permil: parse_env_i32("MDH_HARD_WIDE_PERMIL_BOOST_FEED").unwrap_or(0),
            wide_boost_self_permil: parse_env_i32("MDH_HARD_WIDE_PERMIL_BOOST_SELFCAP").unwrap_or(0),
            cont_cap: weights().cont_cap,
            next3_tiny_hits: 0,
        });
        out
    }
}

impl PlayPlannerHard {
    pub fn explain_candidates_verbose(
        legal: &[Card],
        ctx: &BotContext<'_>,
    ) -> Vec<(Card, i32, i32, i32)> {
        let cfg = Self::config();
        let deterministic = std::env::var("MDH_HARD_DETERMINISTIC")
            .map(|v| v == "1" || v.eq_ignore_ascii_case("true") || v.eq_ignore_ascii_case("on"))
            .unwrap_or(false);
        let step_cap = std::env::var("MDH_HARD_TEST_STEPS").ok().and_then(|s| s.parse::<usize>().ok());
        let mut v = PlayPlanner::explain_candidates(legal, ctx);
        v.sort_by(|a, b| b.1.cmp(&a.1));
        let best_base = v.first().map(|x| x.1).unwrap_or(0);
        let boost_gap = parse_env_i32("MDH_HARD_CONT_BOOST_GAP").unwrap_or(0);
        let boost_factor = parse_env_i32("MDH_HARD_CONT_BOOST_FACTOR").unwrap_or(1);
        let snapshot = snapshot_scores(ctx.scores);
        let start = Instant::now();
        let mut budget = Budget::new(cfg.time_cap_ms, deterministic, step_cap);
        let mut out = Vec::new();
        let (tier, leverage_score, limits) = effective_limits(ctx);
        let phaseb_topk = limits.phaseb_topk;
        let mut scanned = 0usize;
        for (idx, (card, base)) in v.into_iter().take(cfg.branch_limit).enumerate() {
            if budget.should_stop() { break; }
            let cont_raw = if phaseb_topk == 0 || idx < phaseb_topk {
                Self::rollout_current_trick(card, ctx, snapshot.max_player, &mut budget, start, false) 
            } else { 0 };
            let cont = if boost_gap > 0 && boost_factor > 1 && (best_base - base) <= boost_gap {
                cont_raw.saturating_mul(boost_factor)
            } else {
                cont_raw
            };
            let total = base + cont;
            out.push((card, base, cont, total));
            scanned += 1;
            budget.tick();
            if budget.timed_out(start) {
                break;
            }
        }
        set_last_stats(Stats {
            scanned,
            elapsed_ms: start.elapsed().as_millis() as u32,
            scanned_phase_a: scanned,
            scanned_phase_b: 0,
            scanned_phase_c: 0,
            leverage_score,
            tier,
            limits_in_effect: limits,
            utilization: budget.utilization_percent(start),
            wide_boost_feed_permil: parse_env_i32("MDH_HARD_WIDE_PERMIL_BOOST_FEED").unwrap_or(0),
            wide_boost_self_permil: parse_env_i32("MDH_HARD_WIDE_PERMIL_BOOST_SELFCAP").unwrap_or(0),
            cont_cap: weights().cont_cap,
            next3_tiny_hits: 0,
        });
        out
    }

    pub fn explain_candidates_verbose_parts(
        legal: &[Card],
        ctx: &BotContext<'_>,
    ) -> Vec<(Card, i32, ContParts, i32)> {
        let cfg = Self::config();
        let deterministic = std::env::var("MDH_HARD_DETERMINISTIC")
            .map(|v| v == "1" || v.eq_ignore_ascii_case("true") || v.eq_ignore_ascii_case("on"))
            .unwrap_or(false);
        let step_cap = std::env::var("MDH_HARD_TEST_STEPS").ok().and_then(|s| s.parse::<usize>().ok());
        let mut v = PlayPlanner::explain_candidates(legal, ctx);
        v.sort_by(|a, b| b.1.cmp(&a.1));
        let snapshot = snapshot_scores(ctx.scores);
        let start = Instant::now();
        let mut budget = Budget::new(cfg.time_cap_ms, deterministic, step_cap);
        let mut out = Vec::new();
        let (_, _, limits) = effective_limits(ctx);
        let phaseb_topk = limits.phaseb_topk;
        for (idx, (card, base)) in v.into_iter().take(cfg.branch_limit).enumerate() {
            if budget.should_stop() { break; }
            let (cont, parts) = if phaseb_topk == 0 || idx < phaseb_topk {
                Self::rollout_current_trick_with_parts(card, ctx, snapshot.max_player, &mut budget, start, false)
            } else { (0, ContParts::default()) };
            let total = base + cont;
            out.push((card, base, parts, total));
            budget.tick();
            if budget.timed_out(start) { break; }
        }
        out
    }
    // Wrapper: supports optional deterministic multi-sample aggregation (env-gated)
    fn rollout_current_trick(
        card: Card,
        ctx: &BotContext<'_>,
        leader_target: PlayerPosition,
        budget: &mut Budget,
        start: Instant,
        next3_allowed: bool,
    ) -> i32 {
        if det_enabled_for(ctx) {
            let mut acc = 0i32;
            let k = det_sample_k().max(1);
            for _ in 0..k {
                if budget.should_stop() || budget.timed_out(start) { break; }
                acc += Self::rollout_current_trick_core(card, ctx, leader_target, budget, start, next3_allowed);
            }
            if k > 0 { acc / (k as i32) } else { 0 }
        } else {
            Self::rollout_current_trick_core(card, ctx, leader_target, budget, start, next3_allowed)
        }
    }

    // Core: single-sample current-trick rollout (existing logic)
    fn rollout_current_trick_core(
        card: Card,
        ctx: &BotContext<'_>,
        leader_target: PlayerPosition,
        budget: &mut Budget,
        start: Instant,
        next3_allowed: bool,
    ) -> i32 {
        // Simulate only the remainder of the current trick with a simple, void-aware policy.
        let mut sim = ctx.round.clone();
        let seat = ctx.seat;
        let mut outcome = match sim.play_card(seat, card) {
            Ok(o) => o,
            Err(_) => return 0,
        };
        budget.tick();
        while !matches!(outcome, PlayOutcome::TrickCompleted { .. }) {
            if budget.should_stop() || budget.timed_out(start) { break; }
            let next = next_to_play(&sim);
            let reply = if det_enabled_for(ctx) {
                choose_followup_search_sampled(&sim, next, Some(ctx.tracker), seat, Some(leader_target), budget)
            } else {
                choose_followup_search(&sim, next, Some(ctx.tracker), seat, Some(leader_target))
            };
            outcome = match sim.play_card(next, reply) {
                Ok(o) => o,
                Err(_) => break,
            };
            budget.tick();
        }
        match outcome {
            PlayOutcome::TrickCompleted { winner, penalties } => {
                let p = penalties as i32;
                // Very small continuation: prefer feeding leader on penalty tricks, avoid self-capture of penalties.
                let mut cont = 0;
                // Determine tier for potential Wide-tier boost (env-gated; defaults 0)
                let (tier_here, _lev, _lim) = effective_limits(ctx);
                let wide_boost_feed = if matches!(tier_here, Tier::Wide) { parse_env_i32("MDH_HARD_WIDE_PERMIL_BOOST_FEED").unwrap_or(300) } else { 0 };
                let wide_boost_self = if matches!(tier_here, Tier::Wide) { parse_env_i32("MDH_HARD_WIDE_PERMIL_BOOST_SELFCAP").unwrap_or(180) } else { 0 };
                if winner == leader_target && p > 0 {
                    let base = weights().cont_feed_perpen * p;
                    let scale = 1000 + (weights().scale_feed_permil.max(0) * p) + wide_boost_feed.max(0);
                    cont += (base * scale) / 1000;
                }
                if winner == seat && p > 0 {
                    let base = weights().cont_self_capture_perpen * p;
                    let scale = 1000 + (weights().scale_self_permil.max(0) * p) + wide_boost_self.max(0);
                    cont -= (base * scale) / 1000;
                }
                // Next-trick start probe: if we lead next, small bonus for void creation potential.
                if winner == seat {
                    cont += next_trick_start_bonus(&sim, seat);
                    budget.probe_calls = budget.probe_calls.saturating_add(1);
                    cont += next_trick_probe(&sim, seat, ctx, leader_target, next3_allowed);
                    // Tiny extras (defaults 0):
                    // - QS exposure risk: leading with high spades control may attract QS; penalize slightly when holding A♠.
                    if weights().qs_risk_per != 0 {
                        let has_ace_spades = sim
                            .hand(seat)
                            .iter()
                            .any(|c| c.suit == Suit::Spades && c.rank.value() == 14);
                        if has_ace_spades {
                            cont -= weights().qs_risk_per;
                        }
                    }
                    // - Hearts control drift: small positive per heart when hearts are broken and we lead next.
                    if weights().ctrl_hearts_per != 0 && sim.hearts_broken() {
                        let hearts_cnt = sim
                            .hand(seat)
                            .iter()
                            .filter(|c| c.suit == Suit::Hearts)
                            .count() as i32;
                        cont += hearts_cnt * weights().ctrl_hearts_per;
                    }
                    // - Moon relief: if our moon state is active, offset some penalty capture pressure when we win.
                    if weights().moon_relief_perpen != 0 {
                        let state = ctx.tracker.moon_state(seat);
                        if matches!(state, crate::bot::MoonState::Considering | crate::bot::MoonState::Committed) && p > 0 {
                            cont += weights().moon_relief_perpen * p;
                        }
                    }
                }
                // - Control handoff penalty: if we won't lead next, small penalty to reflect loss of initiative.
                if winner != seat && weights().ctrl_handoff_pen != 0 {
                    cont -= weights().ctrl_handoff_pen;
                }
                // Hard cap on continuation magnitude (symmetric), default 0 (off).
                let cap = weights().cont_cap;
                if cap > 0 {
                    if cont > cap { cont = cap; }
                    if cont < -cap { cont = -cap; }
                }
                cont
            }
            _ => 0,
        }
    }

    // Wrapper with parts: supports optional deterministic multi-sample aggregation (env-gated)
    fn rollout_current_trick_with_parts(
        card: Card,
        ctx: &BotContext<'_>,
        leader_target: PlayerPosition,
        budget: &mut Budget,
        start: Instant,
        next3_allowed: bool,
    ) -> (i32, ContParts) {
        if det_enabled() {
            let mut acc = 0i32;
            let mut acc_parts = ContParts::default();
            let k = det_sample_k().max(1);
            for _ in 0..k {
                if budget.should_stop() || budget.timed_out(start) { break; }
                let (v, p) = Self::rollout_current_trick_with_parts_core(card, ctx, leader_target, budget, start, next3_allowed);
                acc += v;
                acc_parts.feed += p.feed;
                acc_parts.self_capture += p.self_capture;
                acc_parts.next_start += p.next_start;
                acc_parts.next_probe += p.next_probe;
                acc_parts.qs_risk += p.qs_risk;
                acc_parts.ctrl_hearts += p.ctrl_hearts;
                acc_parts.ctrl_handoff += p.ctrl_handoff;
                acc_parts.moon_relief += p.moon_relief;
                acc_parts.capped_delta += p.capped_delta;
            }
            if k > 0 {
                let div = k as i32;
                acc_parts.feed /= div;
                acc_parts.self_capture /= div;
                acc_parts.next_start /= div;
                acc_parts.next_probe /= div;
                acc_parts.qs_risk /= div;
                acc_parts.ctrl_hearts /= div;
                acc_parts.ctrl_handoff /= div;
                acc_parts.moon_relief /= div;
                acc_parts.capped_delta /= div;
                (acc / div, acc_parts)
            } else { (0, ContParts::default()) }
        } else {
            Self::rollout_current_trick_with_parts_core(card, ctx, leader_target, budget, start, next3_allowed)
        }
    }

    // Core with parts: single-sample current-trick rollout (existing logic)
    fn rollout_current_trick_with_parts_core(
        card: Card,
        ctx: &BotContext<'_>,
        leader_target: PlayerPosition,
        budget: &mut Budget,
        start: Instant,
        next3_allowed: bool,
    ) -> (i32, ContParts) {
        let mut parts = ContParts::default();
        // Simulate only the remainder of the current trick with a simple, void-aware policy.
        let mut sim = ctx.round.clone();
        let seat = ctx.seat;
        let mut outcome = match sim.play_card(seat, card) {
            Ok(o) => o,
            Err(_) => return (0, parts),
        };
        budget.tick();
        while !matches!(outcome, PlayOutcome::TrickCompleted { .. }) {
            if budget.should_stop() || budget.timed_out(start) { break; }
            let next = next_to_play(&sim);
            let reply = if det_enabled() {
                choose_followup_search_sampled(&sim, next, Some(ctx.tracker), seat, Some(leader_target), budget)
            } else {
                choose_followup_search(&sim, next, Some(ctx.tracker), seat, Some(leader_target))
            };
            outcome = match sim.play_card(next, reply) {
                Ok(o) => o,
                Err(_) => break,
            };
            budget.tick();
        }
        let mut cont = 0;
        if let PlayOutcome::TrickCompleted { winner, penalties } = outcome {
            let p = penalties as i32;
            let (tier_here, _lev, _lim) = effective_limits(ctx);
            let wide_boost_feed = if matches!(tier_here, Tier::Wide) { parse_env_i32("MDH_HARD_WIDE_PERMIL_BOOST_FEED").unwrap_or(300) } else { 0 };
            let wide_boost_self = if matches!(tier_here, Tier::Wide) { parse_env_i32("MDH_HARD_WIDE_PERMIL_BOOST_SELFCAP").unwrap_or(180) } else { 0 };
            if winner == leader_target && p > 0 {
                let base = weights().cont_feed_perpen * p;
                let scale = 1000 + (weights().scale_feed_permil.max(0) * p) + wide_boost_feed.max(0);
                let v = (base * scale) / 1000;
                parts.feed = v;
                cont += v;
            }
            if winner == seat && p > 0 {
                let base = weights().cont_self_capture_perpen * p;
                let scale = 1000 + (weights().scale_self_permil.max(0) * p) + wide_boost_self.max(0);
                let v = - (base * scale) / 1000;
                parts.self_capture = v;
                cont += v;
            }
            if winner == seat {
                let start_bonus = next_trick_start_bonus(&sim, seat);
                parts.next_start = start_bonus;
                cont += start_bonus;
                budget.probe_calls = budget.probe_calls.saturating_add(1);
                let probe = next_trick_probe(&sim, seat, ctx, leader_target, next3_allowed);
                parts.next_probe = probe;
                cont += probe;
                if weights().qs_risk_per != 0 {
                    let has_ace_spades = sim
                        .hand(seat)
                        .iter()
                        .any(|c| c.suit == Suit::Spades && c.rank.value() == 14);
                    if has_ace_spades {
                        parts.qs_risk = -weights().qs_risk_per;
                        cont += parts.qs_risk;
                    }
                }
                if weights().ctrl_hearts_per != 0 && sim.hearts_broken() {
                    let hearts_cnt = sim
                        .hand(seat)
                        .iter()
                        .filter(|c| c.suit == Suit::Hearts)
                        .count() as i32;
                    let v = hearts_cnt * weights().ctrl_hearts_per;
                    parts.ctrl_hearts = v;
                    cont += v;
                }
                if weights().moon_relief_perpen != 0 && p > 0 {
                    let state = ctx.tracker.moon_state(seat);
                    if matches!(state, crate::bot::MoonState::Considering | crate::bot::MoonState::Committed) {
                        let v = weights().moon_relief_perpen * p;
                        parts.moon_relief = v;
                        cont += v;
                    }
                }
            }
            if winner != seat && weights().ctrl_handoff_pen != 0 {
                parts.ctrl_handoff = -weights().ctrl_handoff_pen;
                cont += parts.ctrl_handoff;
            }
        }
        let cap = weights().cont_cap;
        if cap > 0 {
            if cont > cap { parts.capped_delta = cap - cont; cont = cap; }
            if cont < -cap { parts.capped_delta = -cap - cont; cont = -cap; }
        }
        (cont, parts)
    }
}

fn next_to_play(round: &RoundState) -> PlayerPosition {
    let trick = round.current_trick();
    trick
        .plays()
        .last()
        .map(|p| p.position.next())
        .unwrap_or(trick.leader())
}

fn choose_followup_search(
    round: &RoundState,
    seat: PlayerPosition,
    tracker: Option<&crate::bot::tracker::UnseenTracker>,
    origin: PlayerPosition,
    leader_target: Option<PlayerPosition>,
) -> Card {
    // Minimal, void-aware heuristic response.
    let legal = legal_moves_for(round, seat);
    let lead_suit = round.current_trick().lead_suit();
    if let Some(lead) = lead_suit {
        if let Some(card) = legal
            .iter()
            .copied()
            .filter(|c| c.suit == lead)
            .min_by_key(|c| c.rank.value())
        {
            return card;
        }
        let hearts_void = tracker
            .map(|t| t.is_void(seat, Suit::Hearts))
            .unwrap_or(false);
        let provisional = provisional_winner(round);
        if provisional == Some(origin) {
            // Avoid giving points to origin (our seat) in simulation
            if let Some(card) = legal
                .iter()
                .copied()
                .filter(|c| c.penalty_value() == 0)
                .min_by_key(|c| c.rank.value())
            {
                return card;
            }
        }
        if let (Some(pw), Some(leader)) = (provisional, leader_target) {
            if pw == leader {
                // Target the leader: dump QS if possible, otherwise dump highest heart.
                if let Some(qs) = legal.iter().copied().find(|c| c.is_queen_of_spades()) {
                    return qs;
                }
                if !hearts_void {
                    if let Some(card) = legal
                        .iter()
                        .copied()
                        .filter(|c| c.suit == Suit::Hearts)
                        .max_by_key(|c| c.rank.value())
                    {
                        return card;
                    }
                }
            } else {
                // Provisional winner is not the leader: avoid feeding penalties.
                let mut non_penalties: Vec<Card> = legal
                    .iter()
                    .copied()
                    .filter(|c| c.penalty_value() == 0)
                    .collect();
                if !non_penalties.is_empty() {
                    // Small deterministic bias: choose a middle-rank discard when we have several options,
                    // to avoid extremes that could create future exposure; otherwise pick the lowest.
                    non_penalties.sort_by_key(|c| c.rank.value());
                    let idx = if non_penalties.len() >= 3 {
                        non_penalties.len() / 2
                    } else { 0 };
                    return non_penalties[idx];
                }
                // If forced to give points, choose the smallest penalty (low hearts before QS).
                if !hearts_void {
                    if let Some(card) = legal
                        .iter()
                        .copied()
                        .filter(|c| c.suit == Suit::Hearts)
                        .min_by_key(|c| c.rank.value())
                    {
                        return card;
                    }
                }
                if let Some(qs) = legal.iter().copied().find(|c| c.is_queen_of_spades()) {
                    return qs;
                }
            }
        } else {
            // No leader targeting info: prefer safe discard.
            let mut non_penalties: Vec<Card> = legal
                .iter()
                .copied()
                .filter(|c| c.penalty_value() == 0)
                .collect();
            if !non_penalties.is_empty() {
                non_penalties.sort_by_key(|c| c.rank.value());
                let idx = if non_penalties.len() >= 3 {
                    non_penalties.len() / 2
                } else { 0 };
                return non_penalties[idx];
            }
            if !hearts_void {
                if let Some(card) = legal
                    .iter()
                    .copied()
                    .filter(|c| c.suit == Suit::Hearts)
                    .min_by_key(|c| c.rank.value())
                {
                    return card;
                }
            }
            if let Some(qs) = legal.iter().copied().find(|c| c.is_queen_of_spades()) {
                return qs;
            }
        }
    }
    legal
        .into_iter()
        .max_by_key(|c| (c.penalty_value(), c.rank.value()))
        .expect("legal non-empty")
}

// Determinization helper: choose canonical or alternate off-suit dump deterministically using a cheap bit from budget state.
fn choose_followup_search_sampled(
    round: &RoundState,
    seat: PlayerPosition,
    tracker: Option<&crate::bot::tracker::UnseenTracker>,
    origin: PlayerPosition,
    leader_target: Option<PlayerPosition>,
    budget: &mut Budget,
) -> Card {
    // Canonical first
    let canon = choose_followup_search(round, seat, tracker, origin, leader_target);
    // Only consider alternate when off-suit is happening; otherwise canonical is fine
    if let Some(lead) = round.current_trick().lead_suit() {
        let legal = legal_moves_for(round, seat);
        let can_follow = legal.iter().any(|c| c.suit == lead);
        if !can_follow {
            // Alternate = max penalty off-suit if not equal to canonical
            if let Some(alt) = legal
                .iter()
                .copied()
                .max_by_key(|c| (c.penalty_value(), c.rank.value()))
            {
                if alt != canon {
                    // Derive a deterministic sample bit from simple state
                    let bit = sample_bit_for(round, seat, budget);
                    if bit { return alt; }
                }
            }
        }
    }
    canon
}

fn sample_bit_for(round: &RoundState, seat: PlayerPosition, budget: &Budget) -> bool {
    // Cheap hash over a few integers; stable within process for the same sequence
    let trick = round.current_trick();
    let plays = trick.plays().len() as u64;
    let lead = trick.leader() as u8 as u64;
    let seatv = seat as u8 as u64;
    let steps = budget.steps as u64;
    let base = steps.wrapping_mul(0x9E37_79B9_7F4A_7C15).wrapping_add(plays << 3).wrapping_add((lead << 1) ^ seatv);
    let x = base ^ (base >> 33) ^ (base << 17);
    (x & 1) == 1
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

fn next_trick_start_bonus(round: &RoundState, leader: PlayerPosition) -> i32 {
    let hand = round.hand(leader);
    // +25 per singleton in non-hearts suits (max 3). Small +2 per heart when hearts are broken (cap 10).
    let mut counts = [0u8; 4];
    for c in hand.iter() {
        counts[c.suit as usize] = counts[c.suit as usize].saturating_add(1);
    }
    let mut bonus = 0;
    for (i, &cnt) in counts.iter().enumerate() {
        if i != Suit::Hearts as usize && cnt == 1 {
            bonus += weights().next_trick_singleton_bonus;
        }
    }
    bonus = bonus.min(weights().next_trick_singleton_bonus * 3);
    if round.hearts_broken() {
        let hearts = counts[Suit::Hearts as usize] as i32;
        bonus += (weights().next_trick_hearts_per * hearts).min(weights().next_trick_hearts_cap);
    }
    bonus
}

struct HardWeights {
    cont_feed_perpen: i32,
    cont_self_capture_perpen: i32,
    next_trick_singleton_bonus: i32,
    next_trick_hearts_per: i32,
    next_trick_hearts_cap: i32,
    next2_feed_perpen: i32,
    next2_self_capture_perpen: i32,
    // Tiny continuation extras (defaults 0):
    qs_risk_per: i32,
    ctrl_hearts_per: i32,
    ctrl_handoff_pen: i32,
    cont_cap: i32,
    moon_relief_perpen: i32,
    // Adaptive scaling (per‑mille per penalty; defaults 0):
    scale_feed_permil: i32,
    scale_self_permil: i32,
}

fn parse_env_i32(key: &str) -> Option<i32> {
    std::env::var(key).ok().and_then(|s| s.parse::<i32>().ok())
}

fn weights() -> &'static HardWeights {
    static W: std::sync::OnceLock<HardWeights> = std::sync::OnceLock::new();
    W.get_or_init(|| HardWeights {
        // Phase A: modestly stronger defaults for Hard continuation
        cont_feed_perpen: parse_env_i32("MDH_HARD_CONT_FEED_PERPEN").unwrap_or(70),
        cont_self_capture_perpen: parse_env_i32("MDH_HARD_CONT_SELF_CAPTURE_PERPEN").unwrap_or(95),
        next_trick_singleton_bonus: parse_env_i32("MDH_HARD_NEXTTRICK_SINGLETON").unwrap_or(25),
        next_trick_hearts_per: parse_env_i32("MDH_HARD_NEXTTRICK_HEARTS_PER").unwrap_or(2),
        next_trick_hearts_cap: parse_env_i32("MDH_HARD_NEXTTRICK_HEARTS_CAP").unwrap_or(10),
        next2_feed_perpen: parse_env_i32("MDH_HARD_NEXT2_FEED_PERPEN").unwrap_or(40),
        next2_self_capture_perpen: parse_env_i32("MDH_HARD_NEXT2_SELF_CAPTURE_PERPEN")
            .unwrap_or(60),
        qs_risk_per: parse_env_i32("MDH_HARD_QS_RISK_PER").unwrap_or(0),
        ctrl_hearts_per: parse_env_i32("MDH_HARD_CTRL_HEARTS_PER").unwrap_or(0),
        ctrl_handoff_pen: parse_env_i32("MDH_HARD_CTRL_HANDOFF_PEN").unwrap_or(0),
        cont_cap: parse_env_i32("MDH_HARD_CONT_CAP").unwrap_or(250),
        moon_relief_perpen: parse_env_i32("MDH_HARD_MOON_RELIEF_PERPEN").unwrap_or(0),
        scale_feed_permil: parse_env_i32("MDH_HARD_CONT_SCALE_FEED_PERMIL").unwrap_or(0),
        scale_self_permil: parse_env_i32("MDH_HARD_CONT_SCALE_SELFCAP_PERMIL").unwrap_or(0),
    })
}

#[derive(Debug, Default, Clone, Copy)]
pub struct ContParts {
    pub feed: i32,
    pub self_capture: i32,
    pub next_start: i32,
    pub next_probe: i32,
    pub qs_risk: i32,
    pub ctrl_hearts: i32,
    pub ctrl_handoff: i32,
    pub moon_relief: i32,
    pub capped_delta: i32,
}

fn debug_enabled() -> bool {
    static ON: std::sync::OnceLock<bool> = std::sync::OnceLock::new();
    *ON.get_or_init(|| {
        std::env::var("MDH_DEBUG_LOGS")
            .map(|v| v == "1" || v.eq_ignore_ascii_case("true") || v.eq_ignore_ascii_case("on"))
            .unwrap_or(false)
    })
}

#[derive(Debug, Clone)]
#[allow(dead_code)]
pub struct Stats {
    pub scanned: usize,
    pub elapsed_ms: u32,
    pub scanned_phase_a: usize,
    pub scanned_phase_b: usize,
    pub scanned_phase_c: usize,
    pub leverage_score: u8,
    pub tier: Tier,
    pub limits_in_effect: Limits,
    pub utilization: u8,
    // Telemetry for env-configured continuation scaling/cap (for tuning introspection)
    pub wide_boost_feed_permil: i32,
    pub wide_boost_self_permil: i32,
    pub cont_cap: i32,
    pub next3_tiny_hits: usize,
}

static LAST_STATS: Lazy<Mutex<Option<Stats>>> = Lazy::new(|| Mutex::new(None));

fn set_last_stats(s: Stats) {
    if let Ok(mut slot) = LAST_STATS.lock() {
        *slot = Some(s);
    }
}

#[allow(dead_code)]
pub fn last_stats() -> Option<Stats> {
    LAST_STATS.lock().ok().and_then(|g| g.clone())
}

fn next_trick_probe(
    sim_round: &RoundState,
    leader: PlayerPosition,
    ctx: &BotContext<'_>,
    leader_target: PlayerPosition,
    next3_allowed: bool,
) -> i32 {
    let cfg = PlayPlannerHard::config();
    let probe_ab_margin = parse_env_i32("MDH_HARD_PROBE_AB_MARGIN").unwrap_or(0);
    // Effective per-tier limit for how many next-trick leads to probe
    let (_, _, limits) = effective_limits(ctx);
    let tmp_ctx = BotContext::new(
        leader,
        sim_round,
        ctx.scores,
        ctx.passing_direction,
        ctx.tracker,
        ctx.difficulty,
    );
    let legal = legal_moves_for(sim_round, leader);
    if legal.is_empty() {
        return 0;
    }
    let mut ordered = PlayPlanner::explain_candidates(&legal, &tmp_ctx);
    ordered.sort_by(|a, b| b.1.cmp(&a.1));
    let start = Instant::now();
    let mut bonus = 0;
    let mut next_limit = if limits.next_probe_m > 0 { limits.next_probe_m } else { cfg.next_branch_limit };
    if next3_allowed { next_limit = next_limit.saturating_add(3); }
    // Determinization wide-like probe widening (env-gated)
    if bool_env("MDH_HARD_DET_ENABLE") && bool_env("MDH_HARD_DET_PROBE_WIDE_LIKE") {
        next_limit = next_limit.saturating_add(2);
    }
    for (lead_card, _) in ordered.into_iter().take(next_limit) {
        if start.elapsed().as_millis() as u32 >= cfg.time_cap_ms {
            break;
        }
        let mut probe = sim_round.clone();
        // Play our lead
        let _ = match probe.play_card(leader, lead_card) {
            Ok(o) => o,
            Err(_) => continue,
        };
        // Branch selectively on the first opponent reply (two variants)
        let first_opponent = next_to_play(&probe);
        let mut replies: Vec<Card> = Vec::new();
        // Canonical reply
        let canon = choose_followup_search(
            &probe,
            first_opponent,
            Some(ctx.tracker),
            leader,
            Some(leader_target),
        );
        replies.push(canon);
        // Alternate: max-penalty dump if available when not following suit
        if let Some(lead_suit) = probe.current_trick().lead_suit() {
            let legal = legal_moves_for(&probe, first_opponent);
            let can_follow = legal.iter().any(|c| c.suit == lead_suit);
            if !can_follow {
                if let Some(alt) = legal
                    .into_iter()
                    .max_by_key(|c| (c.penalty_value(), c.rank.value()))
                {
                    if alt != canon {
                        replies.push(alt);
                    }
                }
            }
        }
        // Evaluate each reply variant; additionally branch on the second opponent reply when time permits.
        let mut local_best = 0;
        for reply in replies.into_iter() {
            if start.elapsed().as_millis() as u32 >= cfg.time_cap_ms {
                break;
            }
            let mut branch = probe.clone();
            let _ = match branch.play_card(first_opponent, reply) {
                Ok(o) => o,
                Err(_) => continue,
            };
            // Optional second-opponent branching
            let second_opponent = next_to_play(&branch);
            let mut replies2: Vec<Card> = Vec::new();
            let canon2 = choose_followup_search(
                &branch,
                second_opponent,
                Some(ctx.tracker),
                leader,
                Some(leader_target),
            );
            replies2.push(canon2);
            if let Some(lead_suit2) = branch.current_trick().lead_suit() {
                let legal2 = legal_moves_for(&branch, second_opponent);
                let can_follow2 = legal2.iter().any(|c| c.suit == lead_suit2);
                if !can_follow2 {
                    if let Some(alt2) = legal2
                        .into_iter()
                        .max_by_key(|c| (c.penalty_value(), c.rank.value()))
                    {
                        if alt2 != canon2 {
                            replies2.push(alt2);
                        }
                    }
                }
            }
            for reply2 in replies2.into_iter() {
                if start.elapsed().as_millis() as u32 >= cfg.time_cap_ms {
                    break;
                }
                if probe_ab_margin > 0 && local_best >= probe_ab_margin {
                    break;
                }
                let mut branch2 = branch.clone();
                let mut outcome2 = match branch2.play_card(second_opponent, reply2) {
                    Ok(o) => o,
                    Err(_) => continue,
                };
                // Optionally branch on third opponent reply (env-gated), otherwise finish canonically.
                let next3_enabled_env = bool_env("MDH_HARD_NEXT3_ENABLE") || (det_enabled_for(ctx) && bool_env("MDH_HARD_DET_NEXT3_ENABLE"));
                let (tier_here, _, _) = effective_limits(ctx);
                let tiny_next3_normal = matches!(tier_here, Tier::Normal) && bool_env("MDH_HARD_NEXT3_TINY_NORMAL");
                if tiny_next3_normal { NEXT3_TINY_COUNT.with(|c| c.set(c.get().saturating_add(1))); }
                let next3_enabled = next3_allowed || next3_enabled_env || tiny_next3_normal;
                if next3_enabled {
                    // Build third-opponent reply set: canonical + optional max-penalty off-suit dump
                    if start.elapsed().as_millis() as u32 >= cfg.time_cap_ms { break; }
                    // Identify third opponent (the next to play now)
                    let third_opponent = next_to_play(&branch2);
                    let mut replies3: Vec<Card> = Vec::new();
                    let canon3 = choose_followup_search(
                        &branch2,
                        third_opponent,
                        Some(ctx.tracker),
                        leader,
                        Some(leader_target),
                    );
                    replies3.push(canon3);
                    if let Some(lead_suit3) = branch2.current_trick().lead_suit() {
                        let legal3 = legal_moves_for(&branch2, third_opponent);
                        let can_follow3 = legal3.iter().any(|c| c.suit == lead_suit3);
                        if !can_follow3 {
                            if let Some(alt3) = legal3.into_iter().max_by_key(|c| (c.penalty_value(), c.rank.value())) {
                                if alt3 != canon3 {
                                    replies3.push(alt3);
                                }
                            }
                        }
                    }
                    for reply3 in replies3.into_iter() {
                        if start.elapsed().as_millis() as u32 >= cfg.time_cap_ms { break; }
                        if probe_ab_margin > 0 && local_best >= probe_ab_margin { break; }
                        let mut branch3 = branch2.clone();
                        let mut outcome3 = match branch3.play_card(third_opponent, reply3) {
                            Ok(o) => o,
                            Err(_) => continue,
                        };
                        while !matches!(outcome3, PlayOutcome::TrickCompleted { .. }) {
                            let nxt = next_to_play(&branch3);
                            let r = choose_followup_search(
                                &branch3,
                                nxt,
                                Some(ctx.tracker),
                                leader,
                                Some(leader_target),
                            );
                            outcome3 = match branch3.play_card(nxt, r) {
                                Ok(o) => o,
                                Err(_) => break,
                            };
                        }
                        if let PlayOutcome::TrickCompleted { winner, penalties } = outcome3 {
                            let p = penalties as i32;
                            let mut cont = 0;
                            if winner == leader_target && p > 0 { cont += weights().next2_feed_perpen * p; }
                            if winner == leader && p > 0 { cont -= weights().next2_self_capture_perpen * p; }
                            // Endgame micro-solver (choose-only; env-gated)
                            cont += micro_endgame_bonus(&branch3, ctx, leader, leader_target);
                            if cont > local_best { local_best = cont; }
                        }
                    }
                } else {
                    // Canonical finish of trick
                    while !matches!(outcome2, PlayOutcome::TrickCompleted { .. }) {
                        let nxt = next_to_play(&branch2);
                        let r = choose_followup_search(
                            &branch2,
                            nxt,
                            Some(ctx.tracker),
                            leader,
                            Some(leader_target),
                        );
                        outcome2 = match branch2.play_card(nxt, r) {
                            Ok(o) => o,
                            Err(_) => break,
                        };
                    }
                    if let PlayOutcome::TrickCompleted { winner, penalties } = outcome2 {
                        let p = penalties as i32;
                        let mut cont = 0;
                        if winner == leader_target && p > 0 { cont += weights().next2_feed_perpen * p; }
                        if winner == leader && p > 0 { cont -= weights().next2_self_capture_perpen * p; }
                        // Endgame micro-solver (choose-only; env-gated)
                        cont += micro_endgame_bonus(&branch2, ctx, leader, leader_target);
                        if cont > local_best { local_best = cont; }
                    }
                }
            }
        }
        bonus += local_best;
    }
    bonus
}

// Track how many times the tiny Normal-tier next3 gate triggered during a decision (debug/telemetry)
thread_local! {
    static NEXT3_TINY_COUNT: Cell<usize> = Cell::new(0);
}

// ----- Endgame micro-solver (choose-only; env-gated) -----

fn micro_endgame_enabled() -> bool {
    std::env::var("MDH_HARD_ENDGAME_DP_ENABLE")
        .map(|v| v == "1" || v.eq_ignore_ascii_case("true") || v.eq_ignore_ascii_case("on"))
        .unwrap_or(false)
}

fn micro_endgame_max_cards() -> usize {
    std::env::var("MDH_HARD_ENDGAME_MAX_CARDS").ok().and_then(|s| s.parse().ok()).unwrap_or(3)
}

fn micro_endgame_bonus(sim: &RoundState, _ctx: &BotContext<'_>, leader: PlayerPosition, leader_target: PlayerPosition) -> i32 {
    if !micro_endgame_enabled() { return 0; }
    // Quick trigger check: all seats at or below max cards
    let maxn = micro_endgame_max_cards();
    let mut ok = true;
    for seat in [PlayerPosition::North, PlayerPosition::East, PlayerPosition::South, PlayerPosition::West] {
        if sim.hand(seat).len() > maxn { ok = false; break; }
    }
    if !ok { return 0; }
    // Minimal deterministic signal placeholder; defaults keep it 0
    let bonus = std::env::var("MDH_HARD_ENDGAME_BONUS").ok().and_then(|s| s.parse::<i32>().ok()).unwrap_or(0);
    if bonus == 0 { return 0; }
    // Apply a tiny, bounded influence once per call; respect overall continuation cap in caller
    // Optionally bias when provisional winner is leader_target and penalties exist
    let mut out = bonus;
    if let Some(pw) = provisional_winner(sim) {
        if pw == leader_target && sim.current_trick().penalty_total() > 0 { out = out.saturating_add(bonus); }
        if pw == leader && sim.current_trick().penalty_total() > 0 { out = out.saturating_sub(bonus); }
    }
    out
}

pub fn debug_hard_weights_string() -> String {
    let w = weights();
    let cfg = PlayPlannerHard::config();
    let boost_gap = parse_env_i32("MDH_HARD_CONT_BOOST_GAP").unwrap_or(0);
    let boost_factor = parse_env_i32("MDH_HARD_CONT_BOOST_FACTOR").unwrap_or(1);
    // Wide-tier continuation permille boosts (env-only; default 0)
    let wide_boost_feed = parse_env_i32("MDH_HARD_WIDE_PERMIL_BOOST_FEED").unwrap_or(0);
    let wide_boost_self = parse_env_i32("MDH_HARD_WIDE_PERMIL_BOOST_SELFCAP").unwrap_or(0);
    let phaseb_topk = std::env::var("MDH_HARD_PHASEB_TOPK").ok().and_then(|s| s.parse::<usize>().ok()).unwrap_or(0);
    let det = std::env::var("MDH_HARD_DETERMINISTIC").unwrap_or_default();
    let steps = std::env::var("MDH_HARD_TEST_STEPS").unwrap_or_default();
    let det_enable = std::env::var("MDH_HARD_DET_ENABLE").unwrap_or_default();
    let det_k = std::env::var("MDH_HARD_DET_SAMPLE_K").unwrap_or_default();
    let det_ms = std::env::var("MDH_HARD_DET_TIME_MS").unwrap_or_default();
    let abm = std::env::var("MDH_HARD_AB_MARGIN").unwrap_or_default();
    let next3 = std::env::var("MDH_HARD_NEXT3_ENABLE").unwrap_or_default();
    let probe_ab = std::env::var("MDH_HARD_PROBE_AB_MARGIN").unwrap_or_default();
    let tiers = std::env::var("MDH_HARD_TIERS_ENABLE").unwrap_or_default();
    let th_narrow = std::env::var("MDH_HARD_LEVERAGE_THRESH_NARROW").unwrap_or_default();
    let th_normal = std::env::var("MDH_HARD_LEVERAGE_THRESH_NORMAL").unwrap_or_default();
    let tiers_auto = std::env::var("MDH_HARD_TIERS_DEFAULT_ON_HARD").unwrap_or_default();
    let promoted = std::env::var("MDH_HARD_PROMOTE_DEFAULTS").unwrap_or_default();
    format!(
        "branch_limit={} next_branch_limit={} time_cap_ms={} cutoff_margin={} ab_margin={} probe_ab_margin={} next3={} cont_feed_perpen={} cont_self_capture_perpen={} next_singleton={} next_hearts_per={} next_hearts_cap={} next2_feed_perpen={} next2_self_capture_perpen={} qs_risk_per={} ctrl_hearts_per={} ctrl_handoff_pen={} cont_cap={} moon_relief_perpen={} cont_boost_gap={} cont_boost_factor={} wide_boost_feed_permil={} wide_boost_self_permil={} phaseb_topk={} det={} steps={} det_enable={} det_k={} det_ms={} tiers={} tiers_auto={} promoted={} th_narrow={} th_normal={} cont_scale_feed_permil={} cont_scale_self_permil={}",
        cfg.branch_limit,
        cfg.next_branch_limit,
        cfg.time_cap_ms,
        cfg.early_cutoff_margin,
        abm,
        probe_ab,
        next3,
        w.cont_feed_perpen,
        w.cont_self_capture_perpen,
        w.next_trick_singleton_bonus,
        w.next_trick_hearts_per,
        w.next_trick_hearts_cap,
        w.next2_feed_perpen,
        w.next2_self_capture_perpen,
        w.qs_risk_per,
        w.ctrl_hearts_per,
        w.ctrl_handoff_pen,
        w.cont_cap,
        w.moon_relief_perpen,
        boost_gap,
        boost_factor,
        wide_boost_feed,
        wide_boost_self,
        phaseb_topk,
        det,
        steps,
        det_enable,
        det_k,
        det_ms,
        tiers,
        tiers_auto,
        promoted,
        th_narrow,
        th_normal,
        w.scale_feed_permil,
        w.scale_self_permil,
    )
}

// Deterministic/time-capped budget for Hard planner (env-gated)
struct Budget {
    time_cap_ms: u32,
    deterministic: bool,
    step_cap: Option<usize>,
    steps: usize,
    // telemetry counters
    probe_calls: usize,
}

impl Budget {
    fn new(time_cap_ms: u32, deterministic: bool, step_cap: Option<usize>) -> Self {
        Self { time_cap_ms, deterministic, step_cap, steps: 0, probe_calls: 0 }
    }
    fn tick(&mut self) {
        if self.deterministic {
            self.steps = self.steps.saturating_add(1);
        }
    }
    fn should_stop(&self) -> bool {
        if let (true, Some(cap)) = (self.deterministic, self.step_cap) {
            return self.steps >= cap;
        }
        false
    }
    fn timed_out(&self, start: Instant) -> bool {
        if self.deterministic { return self.should_stop(); }
        start.elapsed().as_millis() as u32 >= self.time_cap_ms
    }
    fn utilization_percent(&self, start: Instant) -> u8 {
        if self.deterministic {
            if let Some(cap) = self.step_cap {
                return (((self.steps as f32) / (cap as f32)) * 100.0).round().clamp(0.0, 100.0) as u8;
            }
            return 0;
        }
        let used = start.elapsed().as_millis() as u32;
        if self.time_cap_ms == 0 { return 0; }
        (((used as f32) / (self.time_cap_ms as f32)) * 100.0).round().clamp(0.0, 100.0) as u8
    }
}


// ----- Leverage tiers and effective limits -----

#[derive(Debug, Clone, Copy)]
pub enum Tier { Narrow, Normal, Wide }

#[derive(Debug, Clone, Copy)]
pub struct Limits {
    pub phaseb_topk: usize,
    pub next_probe_m: usize,
    pub ab_margin: i32,
}

fn bool_env(key: &str) -> bool {
    std::env::var(key)
        .map(|v| v == "1" || v.eq_ignore_ascii_case("true") || v.eq_ignore_ascii_case("on"))
        .unwrap_or(false)
}

fn effective_limits(ctx: &BotContext<'_>) -> (Tier, u8, Limits) {
    // Phase A: enable tiers by default for Hard difficulty; still respect explicit global enable/disable via env.
    let tier_for_hard = matches!(ctx.difficulty, super::BotDifficulty::FutureHard);
    let tiers_on = tier_for_hard || bool_env("MDH_HARD_TIERS_ENABLE");
    if !tiers_on {
        let phaseb_topk = std::env::var("MDH_HARD_PHASEB_TOPK").ok().and_then(|s| s.parse::<usize>().ok()).unwrap_or(0);
        let ab_margin = parse_env_i32("MDH_HARD_AB_MARGIN").unwrap_or(0);
        let next_probe_m = PlayPlannerHard::config().next_branch_limit;
        return (Tier::Normal, 0, Limits { phaseb_topk, next_probe_m, ab_margin });
    }
    let (tier, score) = compute_leverage(ctx);
    // Respect explicit env overrides if present (global and Wide-tier specific)
    let explicit_topk = std::env::var("MDH_HARD_PHASEB_TOPK").ok().and_then(|s| s.parse::<usize>().ok());
    let explicit_next = std::env::var("MDH_HARD_NEXT_BRANCH_LIMIT").ok().and_then(|s| s.parse::<usize>().ok());
    let wide_topk_only = if matches!(tier, Tier::Wide) {
        std::env::var("MDH_HARD_WIDE_PHASEB_TOPK").ok().and_then(|s| s.parse::<usize>().ok())
    } else { None };
    let wide_next_only = if matches!(tier, Tier::Wide) {
        std::env::var("MDH_HARD_WIDE_NEXT_BRANCH_LIMIT").ok().and_then(|s| s.parse::<usize>().ok())
    } else { None };
    let explicit_ab = parse_env_i32("MDH_HARD_AB_MARGIN");
    let (def_topk, def_next, def_ab) = match tier {
        Tier::Narrow => (4usize, 1usize, 100i32),
        Tier::Normal => (6usize, 2usize, 150i32),
        Tier::Wide => (8usize, 3usize, 200i32),
    };
    let limits = Limits {
        phaseb_topk: explicit_topk.or(wide_topk_only).unwrap_or(def_topk),
        next_probe_m: explicit_next.or(wide_next_only).unwrap_or(def_next),
        ab_margin: explicit_ab.unwrap_or(def_ab),
    };
    (tier, score, limits)
}

fn compute_leverage(ctx: &BotContext<'_>) -> (Tier, u8) {
    // Simple first pass per HLD: scoreboard pressure, near-100 risk, and penalties on table.
    let snap = super::snapshot_scores(ctx.scores);
    let my = ctx.scores.score(ctx.seat) as i32;
    let lead_score = snap.max_score as i32;
    let mut s: i32 = 0;
    // Proximity to 100
    s += if lead_score >= 90 { 40 } else if lead_score >= 80 { 25 } else { 10 };
    // We are not leader and trail by gap
    if snap.max_player != ctx.seat {
        let gap = (lead_score - my).max(0);
        if gap >= 15 { s += 25; } else if gap >= 8 { s += 15; } else if gap >= 4 { s += 8; }
    }
    // Penalties on table and targeting leader provisional winner
    let cur = ctx.round.current_trick();
    let pen = cur.penalty_total() as i32;
    if pen > 0 { s += 10; }
    if let Some(pw) = provisional_winner(ctx.round) {
        if pw == snap.max_player { s += 10; }
        if pw == ctx.seat { s += 5; }
    }
    // Clamp 0..100
    let s = s.clamp(0, 100) as u8;
    // Map to tiers using thresholds
    let th_narrow = std::env::var("MDH_HARD_LEVERAGE_THRESH_NARROW").ok().and_then(|v| v.parse::<u8>().ok()).unwrap_or(20);
    let th_normal = std::env::var("MDH_HARD_LEVERAGE_THRESH_NORMAL").ok().and_then(|v| v.parse::<u8>().ok()).unwrap_or(50);
    let tier = if s < th_narrow { Tier::Narrow } else if s < th_normal { Tier::Normal } else { Tier::Wide };
    (tier, s)
}


// ----- Determinization (Phase 2 scaffold; env/Hard-gated) -----
fn det_enabled() -> bool { bool_env("MDH_HARD_DET_ENABLE") }
fn det_enabled_for(ctx: &BotContext<'_>) -> bool {
    if det_enabled() { return true; }
    if matches!(ctx.difficulty, super::BotDifficulty::FutureHard) && bool_env("MDH_HARD_DET_DEFAULT_ON") {
        return true;
    }
    false
}
fn det_sample_k() -> usize {
    std::env::var("MDH_HARD_DET_SAMPLE_K").ok().and_then(|s| s.parse::<usize>().ok()).unwrap_or(0)
}
