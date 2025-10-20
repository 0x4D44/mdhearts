use hearts_app::controller::GameController;
use hearts_core::model::card::Card;
use hearts_core::model::passing::PassingDirection;
use hearts_core::model::player::PlayerPosition;
use hearts_core::model::rank::Rank;
use hearts_core::model::suit::Suit;

#[test]
fn golden_first_ai_play_after_passes_is_two_of_clubs() {
    // Ensure heuristic planners are used
    unsafe {
        std::env::set_var("MDH_BOT_DIFFICULTY", "normal");
    }

    for seed in 0u64..128 {
        let mut controller = GameController::new_with_seed(Some(seed), PlayerPosition::North);
        if controller.passing_direction() == PassingDirection::Hold {
            continue;
        }
        let south_pass = controller
            .simple_pass_for(PlayerPosition::South)
            .expect("south can pass");
        controller
            .submit_pass(PlayerPosition::South, south_pass)
            .expect("south pass ok");
        controller
            .submit_auto_passes_for_others(PlayerPosition::South)
            .expect("other passes ok");
        controller.resolve_passes().expect("resolve passes ok");

        let two = Card::new(Rank::Two, Suit::Clubs);
        let holder = PlayerPosition::LOOP
            .iter()
            .copied()
            .find(|seat| controller.hand(*seat).contains(&two))
            .expect("two of clubs dealt");

        // First AI play should be 2C from the holder if it's their turn
        let mut first_play: Option<(PlayerPosition, Card)> = None;
        loop {
            if controller.in_passing_phase() {
                break;
            }
            let seat = controller.expected_to_play();
            if seat == PlayerPosition::South {
                break;
            }
            match controller.autoplay_one(PlayerPosition::South) {
                Some(play) => {
                    first_play.get_or_insert(play);
                }
                None => break,
            }
        }

        if holder == PlayerPosition::South {
            let legal = controller.legal_moves(PlayerPosition::South);
            assert_eq!(legal.len(), 1, "seed {} south legal count", seed);
            assert_eq!(
                legal[0],
                Card::new(Rank::Two, Suit::Clubs),
                "seed {} south must hold 2C",
                seed
            );
        } else if let Some((_, card)) = first_play {
            assert_eq!(card, two, "seed {} should lead with 2C", seed);
        }
    }
}
