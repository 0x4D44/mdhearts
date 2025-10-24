use hearts_core::model::player::PlayerPosition;
use hearts_app::controller::GameController;
use hearts_app::bot::{BotDifficulty, PlayPlannerHard};

#[test]
fn hard_verbose_parts_sum_matches_continuation() {
    // Deterministic budget to keep outputs stable across toolchains
    unsafe { std::env::set_var("MDH_HARD_DETERMINISTIC", "1") };
    unsafe { std::env::set_var("MDH_HARD_TEST_STEPS", "200") };
    // Keep continuation computation on a few top candidates only
    unsafe { std::env::set_var("MDH_HARD_PHASEB_TOPK", "3") };

    let seat = PlayerPosition::West;
    let mut controller = GameController::new_with_seed(Some(1040), PlayerPosition::North);
    controller.set_bot_difficulty(BotDifficulty::FutureHard);

    // Resolve passing phase if present and autoplay to our turn
    if controller.in_passing_phase() {
        if let Some(cards) = controller.simple_pass_for(seat) {
            let _ = controller.submit_pass(seat, cards);
        }
        let _ = controller.submit_auto_passes_for_others(seat);
        let _ = controller.resolve_passes();
    }
    while !controller.in_passing_phase() && controller.expected_to_play() != seat {
        if controller.autoplay_one(seat).is_none() {
            break;
        }
    }

    let legal = controller.legal_moves(seat);
    assert!(!legal.is_empty(), "no legal moves for test seat");

    let ctx = controller.bot_context(seat);
    let expl = PlayPlannerHard::explain_candidates_verbose(&legal, &ctx);
    let parts = PlayPlannerHard::explain_candidates_verbose_parts(&legal, &ctx);

    // Place into maps by card for quick alignment
    use std::collections::HashMap;
    let mut m_expl: HashMap<String, (i32, i32, i32)> = HashMap::new();
    for (c, b, cont, t) in expl {
        m_expl.insert(c.to_string(), (b, cont, t));
    }
    let mut m_parts: HashMap<String, (i32, i32, i32, i32, i32, i32, i32, i32)> = HashMap::new();
    for (c, b, p, _t) in parts {
        m_parts.insert(
            c.to_string(),
            (
                b,
                p.feed,
                p.self_capture,
                p.next_start,
                p.next_probe,
                p.qs_risk,
                p.ctrl_hearts,
                p.ctrl_handoff + p.capped_delta, // include cap adjustment in sum
            ),
        );
    }
    assert_eq!(m_expl.len(), m_parts.len());

    // Verify base+cont==total and cont equals sum of parts (including any cap delta)
    for (k, (b, cont, total)) in m_expl.iter() {
        let (b2, feed, selfc, start, probe, qs, hearts, handoff_cap) = m_parts
            .get(k)
            .unwrap_or_else(|| panic!("missing parts for card {}", k));
        assert_eq!(b + cont, *total, "card {} base+cont!=total", k);
        assert_eq!(*b2, *b, "card {} base mismatch", k);
        let parts_sum = feed + selfc + start + probe + qs + hearts + handoff_cap;
        assert_eq!(parts_sum, *cont, "card {} cont parts mismatch", k);
    }
}
