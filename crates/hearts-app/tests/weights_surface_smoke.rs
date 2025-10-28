#[test]
fn debug_weights_surface_new_knobs() {
    // Set env knobs to distinctive values and ensure they appear in debug strings
    unsafe {
        std::env::set_var("MDH_W_ENDGAME_FEED_CAP", "777");
        std::env::set_var("MDH_HARD_MOON_RELIEF_PERPEN", "33");
        std::env::set_var("MDH_HARD_WIDE_PERMIL_BOOST_FEED", "444");
        std::env::set_var("MDH_HARD_WIDE_PERMIL_BOOST_SELFCAP", "555");
        std::env::set_var("MDH_HARD_DET_ENABLE", "1");
        std::env::set_var("MDH_HARD_DET_SAMPLE_K", "7");
        std::env::set_var("MDH_HARD_DET_TIME_MS", "12");
    }

    let normal = hearts_app::bot::debug_weights_string();
    let hard = hearts_app::bot::search::debug_hard_weights_string();

    assert!(
        normal.contains("endgame_feed_cap_perpen=777"),
        "normal weights should show endgame_feed_cap_perpen"
    );
    assert!(
        hard.contains("moon_relief_perpen=33"),
        "hard weights should show moon_relief_perpen"
    );
    assert!(
        hard.contains("wide_boost_feed_permil=444"),
        "hard weights should show wide_boost_feed_permil"
    );
    assert!(
        hard.contains("wide_boost_self_permil=555"),
        "hard weights should show wide_boost_self_permil"
    );
    assert!(
        hard.contains("det_enable=1"),
        "hard weights should include det_enable"
    );
    assert!(
        hard.contains("det_k=7"),
        "hard weights should include det_k"
    );
    assert!(
        hard.contains("det_ms=12"),
        "hard weights should include det_ms"
    );

    // Cleanup env
    unsafe {
        std::env::remove_var("MDH_W_ENDGAME_FEED_CAP");
        std::env::remove_var("MDH_HARD_MOON_RELIEF_PERPEN");
        std::env::remove_var("MDH_HARD_WIDE_PERMIL_BOOST_FEED");
        std::env::remove_var("MDH_HARD_WIDE_PERMIL_BOOST_SELFCAP");
        std::env::remove_var("MDH_HARD_DET_ENABLE");
        std::env::remove_var("MDH_HARD_DET_SAMPLE_K");
        std::env::remove_var("MDH_HARD_DET_TIME_MS");
    }
}
