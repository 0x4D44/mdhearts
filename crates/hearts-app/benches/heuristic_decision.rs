use criterion::{BatchSize, Criterion, criterion_group, criterion_main};
use hearts_app::controller::GameController;
use hearts_core::model::player::PlayerPosition;

fn bench_explain_once(c: &mut Criterion) {
    let mut group = c.benchmark_group("heuristic_decision");

    // A couple of representative seeds and seats
    let cases: &[(u64, PlayerPosition)] = &[
        (42, PlayerPosition::South),
        (12345, PlayerPosition::East),
        (8675309, PlayerPosition::North),
    ];

    for (seed, seat) in cases.iter().copied() {
        group.bench_function(
            format!("explain_candidates_seed{}_seat{:?}", seed, seat),
            |b| {
                b.iter_batched(
                    || {
                        // Fresh controller each iter to keep state stable
                        let mut controller =
                            GameController::new_with_seed(Some(seed), PlayerPosition::North);
                        // Resolve passing quickly using built-in simple passes
                        if controller.in_passing_phase() {
                            if let Some(cards) = controller.simple_pass_for(seat) {
                                let _ = controller.submit_pass(seat, cards);
                            }
                            let _ = controller.submit_auto_passes_for_others(seat);
                            let _ = controller.resolve_passes();
                        }
                        // Autoplay until it is our seat's turn
                        while !controller.in_passing_phase()
                            && controller.expected_to_play() != seat
                        {
                            if controller.autoplay_one(seat).is_none() {
                                break;
                            }
                        }
                        controller
                    },
                    |controller| {
                        // Measure generating planner candidates/scores once for this snapshot
                        let _explained = controller.explain_candidates_for(seat);
                    },
                    BatchSize::SmallInput,
                )
            },
        );
    }

    group.finish();
}

criterion_group!(benches, bench_explain_once);
criterion_main!(benches);
