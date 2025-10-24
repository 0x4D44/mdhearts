use criterion::{black_box, criterion_group, criterion_main, Criterion};
use hearts_app::controller::GameController;
use hearts_core::model::player::PlayerPosition;

fn bench_explain_hard(seed: u64, seat: PlayerPosition) {
    let mut controller = GameController::new_with_seed(Some(seed), PlayerPosition::North);
    controller.set_bot_difficulty(hearts_app::bot::BotDifficulty::FutureHard);
    if controller.in_passing_phase() {
        if let Some(cards) = controller.simple_pass_for(seat) { let _ = controller.submit_pass(seat, cards); }
        let _ = controller.submit_auto_passes_for_others(seat);
        let _ = controller.resolve_passes();
    }
    while !controller.in_passing_phase() && controller.expected_to_play() != seat {
        if controller.autoplay_one(seat).is_none() { break; }
    }
    let _ = black_box(controller.explain_candidates_for(seat));
}

fn hard_decision_bench(c: &mut Criterion) {
    let mut group = c.benchmark_group("hard_decision");
    for (seed, seat) in [
        (1040u64, PlayerPosition::West),
        (1082u64, PlayerPosition::West),
        (1145u64, PlayerPosition::North),
    ] { group.bench_function(format!("hard_explain_{}_{}", seed, seat as u8), |b| b.iter(|| bench_explain_hard(seed, seat))); }
    group.finish();
}

criterion_group!(benches, hard_decision_bench);
criterion_main!(benches);

