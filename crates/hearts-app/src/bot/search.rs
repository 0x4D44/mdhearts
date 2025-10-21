use super::{BotContext, PlayPlanner, snapshot_scores};
use hearts_core::model::card::Card;
use hearts_core::model::player::PlayerPosition;
use hearts_core::model::round::{PlayOutcome, RoundState};
use hearts_core::model::suit::Suit;
use once_cell::sync::Lazy;
use std::sync::Mutex;

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
        SearchConfig { branch_limit: bl, time_cap_ms: cap, next_branch_limit: nbl, early_cutoff_margin: cutoff, ..SearchConfig::default() }
    }

    pub fn choose(legal: &[Card], ctx: &BotContext<'_>) -> Option<Card> {
        if legal.is_empty() {
            return None;
        }
        let cfg = Self::config();
        // Rank by heuristic (fast), keep top-N, then apply a tiny continuation bonus via a 1-ply trick rollout.
        let mut explained = PlayPlanner::explain_candidates(legal, ctx);
        explained.sort_by(|a, b| b.1.cmp(&a.1));
        let snapshot = snapshot_scores(ctx.scores);
        let mut best: Option<(Card, i32)> = None;
        let start = std::time::Instant::now();
        let mut scanned = 0usize;
        let mut iter = explained.into_iter().take(cfg.branch_limit).peekable();
        while let Some((card, base)) = iter.next() {
            let cont = Self::rollout_current_trick(card, ctx, snapshot.max_player);
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
            // Early cutoff: if the next base score cannot overcome our current best even with a safety margin, stop.
            if let Some((_, best_total)) = best {
                if let Some((next_card, next_base)) = iter.peek() {
                    if *next_base + cfg.early_cutoff_margin < best_total {
                        if debug_enabled() {
                            eprintln!(
                                "mdhearts: hard early cutoff at candidate {} (next_base={} + margin {} < best_total={})",
                                next_card, next_base, cfg.early_cutoff_margin, best_total
                            );
                        }
                        break;
                    }
                }
            }
            if start.elapsed().as_millis() as u32 >= cfg.time_cap_ms {
                if debug_enabled() {
                    eprintln!(
                        "mdhearts: hard cap reached ({} ms), scanned {}",
                        cfg.time_cap_ms, scanned
                    );
                }
                break;
            }
        }
        set_last_stats(Stats {
            scanned,
            elapsed_ms: start.elapsed().as_millis() as u32,
        });
        best.map(|(c, _)| c)
    }

    pub fn explain_candidates(legal: &[Card], ctx: &BotContext<'_>) -> Vec<(Card, i32)> {
        let cfg = Self::config();
        let mut v = PlayPlanner::explain_candidates(legal, ctx);
        v.sort_by(|a, b| b.1.cmp(&a.1));
        let snapshot = snapshot_scores(ctx.scores);
        let start = std::time::Instant::now();
        let mut out = Vec::new();
        let mut scanned = 0usize;
        for (card, base) in v.into_iter().take(cfg.branch_limit) {
            let cont = Self::rollout_current_trick(card, ctx, snapshot.max_player);
            let total = base + cont;
            if debug_enabled() {
                eprintln!(
                    "mdhearts: hard explain {} base={} cont={} total={}",
                    card, base, cont, total
                );
            }
            out.push((card, total));
            scanned += 1;
            if start.elapsed().as_millis() as u32 >= cfg.time_cap_ms {
                if debug_enabled() {
                    eprintln!(
                        "mdhearts: hard explain cap reached ({} ms)",
                        cfg.time_cap_ms
                    );
                }
                break;
            }
        }
        set_last_stats(Stats {
            scanned,
            elapsed_ms: start.elapsed().as_millis() as u32,
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
        let mut v = PlayPlanner::explain_candidates(legal, ctx);
        v.sort_by(|a, b| b.1.cmp(&a.1));
        let snapshot = snapshot_scores(ctx.scores);
        let start = std::time::Instant::now();
        let mut out = Vec::new();
        let mut scanned = 0usize;
        for (card, base) in v.into_iter().take(cfg.branch_limit) {
            let cont = Self::rollout_current_trick(card, ctx, snapshot.max_player);
            let total = base + cont;
            out.push((card, base, cont, total));
            scanned += 1;
            if start.elapsed().as_millis() as u32 >= cfg.time_cap_ms {
                break;
            }
        }
        set_last_stats(Stats {
            scanned,
            elapsed_ms: start.elapsed().as_millis() as u32,
        });
        out
    }
    fn rollout_current_trick(
        card: Card,
        ctx: &BotContext<'_>,
        leader_target: PlayerPosition,
    ) -> i32 {
        // Simulate only the remainder of the current trick with a simple, void-aware policy.
        let mut sim = ctx.round.clone();
        let seat = ctx.seat;
        let mut outcome = match sim.play_card(seat, card) {
            Ok(o) => o,
            Err(_) => return 0,
        };
        while !matches!(outcome, PlayOutcome::TrickCompleted { .. }) {
            let next = next_to_play(&sim);
            let reply =
                choose_followup_search(&sim, next, Some(ctx.tracker), seat, Some(leader_target));
            outcome = match sim.play_card(next, reply) {
                Ok(o) => o,
                Err(_) => break,
            };
        }
        match outcome {
            PlayOutcome::TrickCompleted { winner, penalties } => {
                let p = penalties as i32;
                // Very small continuation: prefer feeding leader on penalty tricks, avoid self-capture of penalties.
                let mut cont = 0;
                if winner == leader_target && p > 0 {
                    cont += weights().cont_feed_perpen * p;
                }
                if winner == seat && p > 0 {
                    cont -= weights().cont_self_capture_perpen * p;
                }
                // Next-trick start probe: if we lead next, small bonus for void creation potential.
                if winner == seat {
                    cont += next_trick_start_bonus(&sim, seat);
                    cont += next_trick_probe(&sim, seat, ctx, leader_target);
                }
                cont
            }
            _ => 0,
        }
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
            }
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
        if let Some(qs) = legal.iter().copied().find(|c| c.is_queen_of_spades()) {
            return qs;
        }
    }
    legal
        .into_iter()
        .max_by_key(|c| (c.penalty_value(), c.rank.value()))
        .expect("legal non-empty")
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
}

fn parse_env_i32(key: &str) -> Option<i32> {
    std::env::var(key).ok().and_then(|s| s.parse::<i32>().ok())
}

fn weights() -> &'static HardWeights {
    static W: std::sync::OnceLock<HardWeights> = std::sync::OnceLock::new();
    W.get_or_init(|| HardWeights {
        cont_feed_perpen: parse_env_i32("MDH_HARD_CONT_FEED_PERPEN").unwrap_or(60),
        cont_self_capture_perpen: parse_env_i32("MDH_HARD_CONT_SELF_CAPTURE_PERPEN").unwrap_or(80),
        next_trick_singleton_bonus: parse_env_i32("MDH_HARD_NEXTTRICK_SINGLETON").unwrap_or(25),
        next_trick_hearts_per: parse_env_i32("MDH_HARD_NEXTTRICK_HEARTS_PER").unwrap_or(2),
        next_trick_hearts_cap: parse_env_i32("MDH_HARD_NEXTTRICK_HEARTS_CAP").unwrap_or(10),
        next2_feed_perpen: parse_env_i32("MDH_HARD_NEXT2_FEED_PERPEN").unwrap_or(40),
        next2_self_capture_perpen: parse_env_i32("MDH_HARD_NEXT2_SELF_CAPTURE_PERPEN").unwrap_or(60),
    })
}

fn debug_enabled() -> bool {
    static ON: std::sync::OnceLock<bool> = std::sync::OnceLock::new();
    *ON.get_or_init(|| {
        std::env::var("MDH_DEBUG_LOGS")
            .map(|v| v == "1" || v.eq_ignore_ascii_case("true") || v.eq_ignore_ascii_case("on"))
            .unwrap_or(false)
    })
}

#[derive(Debug, Clone, Copy)]
#[allow(dead_code)]
pub struct Stats {
    pub scanned: usize,
    pub elapsed_ms: u32,
}

static LAST_STATS: Lazy<Mutex<Option<Stats>>> = Lazy::new(|| Mutex::new(None));

fn set_last_stats(s: Stats) {
    if let Ok(mut slot) = LAST_STATS.lock() {
        *slot = Some(s);
    }
}

#[allow(dead_code)]
pub fn last_stats() -> Option<Stats> {
    LAST_STATS.lock().ok().and_then(|g| *g)
}

fn next_trick_probe(sim_round: &RoundState, leader: PlayerPosition, ctx: &BotContext<'_>, leader_target: PlayerPosition) -> i32 {
    let cfg = PlayPlannerHard::config();
    let tmp_ctx = BotContext::new(
        leader,
        sim_round,
        ctx.scores,
        ctx.passing_direction,
        ctx.tracker,
        ctx.difficulty,
    );
    let legal = legal_moves_for(sim_round, leader);
    if legal.is_empty() { return 0; }
    let mut ordered = PlayPlanner::explain_candidates(&legal, &tmp_ctx);
    ordered.sort_by(|a,b| b.1.cmp(&a.1));
    let start = std::time::Instant::now();
    let mut bonus = 0;
    for (lead_card, _) in ordered.into_iter().take(cfg.next_branch_limit) {
        if start.elapsed().as_millis() as u32 >= cfg.time_cap_ms { break; }
        let mut probe = sim_round.clone();
        // Play our lead
        let _ = match probe.play_card(leader, lead_card) { Ok(o) => o, Err(_) => continue };
        // Branch selectively on the first opponent reply (two variants)
        let first_opponent = next_to_play(&probe);
        let mut replies: Vec<Card> = Vec::new();
        // Canonical reply
        let canon = choose_followup_search(&probe, first_opponent, Some(ctx.tracker), leader, Some(leader_target));
        replies.push(canon);
        // Alternate: max-penalty dump if available when not following suit
        if let Some(lead_suit) = probe.current_trick().lead_suit() {
            let legal = legal_moves_for(&probe, first_opponent);
            let can_follow = legal.iter().any(|c| c.suit == lead_suit);
            if !can_follow {
                if let Some(alt) = legal.into_iter().max_by_key(|c| (c.penalty_value(), c.rank.value())) {
                    if alt != canon { replies.push(alt); }
                }
            }
        }
        // Evaluate each reply variant; additionally branch on the second opponent reply when time permits.
        let mut local_best = 0;
        for reply in replies.into_iter() {
            if start.elapsed().as_millis() as u32 >= cfg.time_cap_ms { break; }
            let mut branch = probe.clone();
            let _ = match branch.play_card(first_opponent, reply) { Ok(o) => o, Err(_) => continue };
            // Optional second-opponent branching
            let second_opponent = next_to_play(&branch);
            let mut replies2: Vec<Card> = Vec::new();
            let canon2 = choose_followup_search(&branch, second_opponent, Some(ctx.tracker), leader, Some(leader_target));
            replies2.push(canon2);
            if let Some(lead_suit2) = branch.current_trick().lead_suit() {
                let legal2 = legal_moves_for(&branch, second_opponent);
                let can_follow2 = legal2.iter().any(|c| c.suit == lead_suit2);
                if !can_follow2 {
                    if let Some(alt2) = legal2.into_iter().max_by_key(|c| (c.penalty_value(), c.rank.value())) {
                        if alt2 != canon2 { replies2.push(alt2); }
                    }
                }
            }
            for reply2 in replies2.into_iter() {
                if start.elapsed().as_millis() as u32 >= cfg.time_cap_ms { break; }
                let mut branch2 = branch.clone();
                let mut outcome2 = match branch2.play_card(second_opponent, reply2) { Ok(o) => o, Err(_) => continue };
                while !matches!(outcome2, PlayOutcome::TrickCompleted { .. }) {
                    let nxt = next_to_play(&branch2);
                    let r = choose_followup_search(&branch2, nxt, Some(ctx.tracker), leader, Some(leader_target));
                    outcome2 = match branch2.play_card(nxt, r) { Ok(o) => o, Err(_) => break };
                }
                if let PlayOutcome::TrickCompleted { winner, penalties } = outcome2 {
                    let p = penalties as i32;
                    let mut cont = 0;
                    if winner == leader_target && p > 0 { cont += weights().next2_feed_perpen * p; }
                    if winner == leader && p > 0 { cont -= weights().next2_self_capture_perpen * p; }
                    if cont > local_best { local_best = cont; }
                }
            }
        }
        bonus += local_best;
    }
    bonus
}

pub fn debug_hard_weights_string() -> String {
    let w = weights();
    let cfg = PlayPlannerHard::config();
    format!(
        "branch_limit={} next_branch_limit={} time_cap_ms={} cutoff_margin={} cont_feed_perpen={} cont_self_capture_perpen={} next_singleton={} next_hearts_per={} next_hearts_cap={} next2_feed_perpen={} next2_self_capture_perpen={}",
        cfg.branch_limit,
        cfg.next_branch_limit,
        cfg.time_cap_ms,
        cfg.early_cutoff_margin,
        w.cont_feed_perpen,
        w.cont_self_capture_perpen,
        w.next_trick_singleton_bonus,
        w.next_trick_hearts_per,
        w.next_trick_hearts_cap,
        w.next2_feed_perpen,
        w.next2_self_capture_perpen,
    )
}

