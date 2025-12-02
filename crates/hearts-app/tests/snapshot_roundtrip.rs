use hearts_core::game::serialization::MatchSnapshot;
use hearts_core::model::player::PlayerPosition;

#[test]
fn full_snapshot_fixture_restores_round_state() {
    // Use bundled fixture to ensure full snapshots round-trip correctly.
    let data = include_str!("fixtures/full_snapshot_example.json");

    let snapshot: MatchSnapshot = MatchSnapshot::from_json(&data).expect("valid snapshot json");
    let state = snapshot.clone().restore_full();

    assert_eq!(state.seed(), snapshot.seed);
    assert_eq!(state.round_number(), snapshot.round_number);
    assert_eq!(
        state.passing_direction().as_str(),
        snapshot.passing_direction
    );
    assert_eq!(
        state.passing_direction(),
        hearts_core::model::passing::PassingDirection::Left
    );
    assert_eq!(state.round().starting_player(), PlayerPosition::North);

    // Hands should match fixture (3 cards each, sorted).
    for seat in PlayerPosition::LOOP.iter().copied() {
        assert_eq!(state.round().hand(seat).len(), 3);
    }

    // Phase should still be Passing with no submissions.
    match state.round().phase() {
        hearts_core::model::round::RoundPhase::Passing(p) => {
            assert!(p.submissions().iter().all(|s| s.is_none()));
        }
        other => panic!("expected Passing phase, got {other:?}"),
    }
    assert!(!state.round().hearts_broken());
}
