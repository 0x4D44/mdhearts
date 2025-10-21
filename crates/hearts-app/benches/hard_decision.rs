use criterion::{BatchSize, Criterion, criterion_group, criterion_main};
use hearts_app::controller::GameController;
use hearts_core::model::player::PlayerPosition;

fn bench_hard_explain(c: &mut Criterion) {
    let mut group = c.benchmark_group("hard_decision");

    // Run in hard mode by env; bench launcher can set MDH_BOT_DIFFICULTY=hard.
    let cases: &[(u64, PlayerPosition)] = &[
        (42, PlayerPosition::South),
        (12345, PlayerPosition::East),
        (8675309, PlayerPosition::North),
    ];

    for (seed, seat) in cases.iter().copied() {
        group.bench_function(format!("hard_explain_seed{}_seat{:?}", seed, seat), |b| {
            b.iter_batched(
                || {
                    let mut controller =
                        GameController::new_with_seed(Some(seed), PlayerPosition::North);
                    controller.set_bot_difficulty(hearts_app::bot::BotDifficulty::FutureHard);
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
                    controller
                },
                |controller| {
                    let _ = controller.explain_candidates_for(seat);
                },
                BatchSize::SmallInput,
            )
        });
    }

    group.finish();
}

criterion_group!(benches, bench_hard_explain);
criterion_main!(benches);
