/// Tunable bot parameters for heuristic scoring.
///
/// These values control the behavior of the PassPlanner and PlayPlanner.
/// Extracted from hardcoded magic numbers to enable systematic tuning.
#[allow(dead_code)]
#[derive(Debug, Clone, Copy)]
pub struct BotParams {
    // === Passing Strategy Parameters ===
    /// Bonus for passing Queen of Spades (default: 18000)
    pub pass_queen_spades: i32,

    /// Bonus for passing Ace of Spades (default: 5000)
    pub pass_ace_spades: i32,

    /// Bonus for passing King of Spades (default: 7000)
    pub pass_king_spades: i32,

    /// Bonus for passing Jack of Spades (default: 2500)
    pub pass_jack_spades: i32,

    /// Base bonus for passing hearts (default: 6000)
    pub pass_hearts_base: i32,

    /// Per-rank bonus for passing hearts (default: 120)
    pub pass_hearts_rank_mult: i32,

    /// Bonus for passing high non-heart cards (default: 2200)
    pub pass_high_cards_base: i32,

    /// Per-rank multiplier for high cards (default: 80)
    pub pass_high_cards_rank_mult: i32,

    /// Bonus for creating voids (suit_len <= 2) (default: 4000)
    pub pass_void_creation_base: i32,

    /// Per-card penalty for short suits (default: 800)
    pub pass_void_creation_mult: i32,

    /// Penalty for long suits (suit_len >= 5) per extra card (default: 400)
    pub pass_long_suit_penalty: i32,

    /// Multiplier when passing to trailing player (default: 1400)
    pub pass_to_trailing_mult: i32,

    /// Multiplier when passing to leader (default: -1200)
    pub pass_to_leader_mult: i32,

    /// Multiplier when my score >= 75 (desperate mode) (default: 1600)
    pub pass_desperate_mult: i32,

    /// Bonus for unseen cards (default: 90)
    pub pass_unseen_bonus: i32,

    /// Penalty for keeping 2 of Clubs (default: -4000)
    pub pass_two_clubs_penalty: i32,

    /// Moon shot: penalty for passing hearts (default: -9000)
    pub pass_moon_keep_hearts: i32,

    /// Moon shot: penalty for passing Queen of Spades (default: -12000)
    pub pass_moon_keep_queen: i32,

    /// Moon shot: penalty for passing high spades (default: -9000)
    pub pass_moon_keep_spades: i32,

    /// Moon shot: bonus for creating voids in non-hearts (default: 2500)
    pub pass_moon_void_bonus: i32,

    /// Hunt leader: bonus per penalty point (default: 900)
    pub pass_hunt_penalty_mult: i32,

    /// Hunt leader: extra bonus when passing to trailing (default: 600)
    pub pass_hunt_trailing_mult: i32,

    /// Late-round urgency multiplier (default: 12)
    pub pass_cards_played_mult: i32,

    /// Leader advantage: rank multiplier when far ahead (default: 40)
    pub pass_leader_rank_mult: i32,

    // === Play Strategy Parameters ===
    /// Penalty for taking a trick (default: -4800)
    pub play_take_trick_penalty: i32,

    /// Penalty per point when taking trick (default: -700)
    pub play_take_points_mult: i32,

    /// Reward for not taking trick (default: 600)
    pub play_avoid_trick_reward: i32,

    /// Reward per point when dumping (default: 500)
    pub play_dump_points_mult: i32,

    /// Penalty for taking clean trick (default: -18 per rank)
    pub play_clean_trick_rank_mult: i32,

    /// Bonus for creating void (default: 750)
    pub play_void_creation_bonus: i32,

    /// When following: rank penalty (dump high) (default: -24)
    pub play_follow_rank_mult: i32,

    /// When sloughing: penalty bonus (default: 500)
    pub play_slough_penalty_mult: i32,

    /// When leading: rank penalty (lead low) (default: -10)
    pub play_lead_rank_mult: i32,

    /// Penalty for breaking hearts (default: -1100)
    pub play_break_hearts_penalty: i32,

    /// Hunt leader: bonus for leading penalties (default: 10000)
    pub play_hunt_lead_penalty_base: i32,

    /// Hunt leader: per-point bonus when leading (default: 400)
    pub play_hunt_lead_penalty_mult: i32,

    /// Moon shot: bonus for leading hearts (default: 1300)
    pub play_moon_lead_hearts: i32,

    /// Late-round urgency when at 90+ points (default: -1200)
    pub play_desperate_take_mult: i32,

    /// Late-round dump bonus when at 90+ points (default: 300)
    pub play_desperate_dump_mult: i32,

    /// Per-card-played pacing multiplier (default: 8)
    pub play_cards_played_mult: i32,

    /// Bonus for unseen cards (default: 20)
    pub play_unseen_bonus: i32,

    /// Moon shot: bonus for taking tricks (default: 5500)
    pub play_moon_take_trick: i32,

    /// Moon shot: bonus per penalty point taken (default: 900)
    pub play_moon_take_points_mult: i32,

    /// Moon shot: penalty per penalty point not taken (default: -800)
    pub play_moon_avoid_points_mult: i32,

    /// Hunt leader: general avoid trick penalty (default: -1000)
    pub play_hunt_avoid_trick: i32,

    /// Hunt leader: bonus for feeding leader (default: 700)
    pub play_hunt_feed_leader_mult: i32,
}

impl Default for BotParams {
    fn default() -> Self {
        Self {
            // Passing parameters
            pass_queen_spades: 18_000,
            pass_ace_spades: 5_000,
            pass_king_spades: 7_000,
            pass_jack_spades: 2_500,
            pass_hearts_base: 6_000,
            pass_hearts_rank_mult: 120,
            pass_high_cards_base: 2_200,
            pass_high_cards_rank_mult: 80,
            pass_void_creation_base: 4_000,
            pass_void_creation_mult: 800,
            pass_long_suit_penalty: 400,
            pass_to_trailing_mult: 1_400,
            pass_to_leader_mult: -1_200,
            pass_desperate_mult: 1_600,
            pass_unseen_bonus: 90,
            pass_two_clubs_penalty: -4_000,
            pass_moon_keep_hearts: -9_000,
            pass_moon_keep_queen: -12_000,
            pass_moon_keep_spades: -9_000,
            pass_moon_void_bonus: 2_500,
            pass_hunt_penalty_mult: 900,
            pass_hunt_trailing_mult: 600,
            pass_cards_played_mult: 12,
            pass_leader_rank_mult: 40,

            // Play parameters
            play_take_trick_penalty: -4_800,
            play_take_points_mult: -700,
            play_avoid_trick_reward: 600,
            play_dump_points_mult: 500,
            play_clean_trick_rank_mult: -18,
            play_void_creation_bonus: 750,
            play_follow_rank_mult: -24,
            play_slough_penalty_mult: 500,
            play_lead_rank_mult: -10,
            play_break_hearts_penalty: -1_100,
            play_hunt_lead_penalty_base: 10_000,
            play_hunt_lead_penalty_mult: 400,
            play_moon_lead_hearts: 1_300,
            play_desperate_take_mult: -1_200,
            play_desperate_dump_mult: 300,
            play_cards_played_mult: 8,
            play_unseen_bonus: 20,
            play_moon_take_trick: 5_500,
            play_moon_take_points_mult: 900,
            play_moon_avoid_points_mult: -800,
            play_hunt_avoid_trick: -1_000,
            play_hunt_feed_leader_mult: 700,
        }
    }
}

impl BotParams {
    /// Create params with all values scaled by a factor.
    /// Useful for testing relative importance of parameters.
    #[allow(dead_code)]
    pub fn scaled(factor: f32) -> Self {
        let default = Self::default();
        Self {
            pass_queen_spades: (default.pass_queen_spades as f32 * factor) as i32,
            pass_ace_spades: (default.pass_ace_spades as f32 * factor) as i32,
            pass_king_spades: (default.pass_king_spades as f32 * factor) as i32,
            pass_jack_spades: (default.pass_jack_spades as f32 * factor) as i32,
            pass_hearts_base: (default.pass_hearts_base as f32 * factor) as i32,
            pass_hearts_rank_mult: (default.pass_hearts_rank_mult as f32 * factor) as i32,
            pass_high_cards_base: (default.pass_high_cards_base as f32 * factor) as i32,
            pass_high_cards_rank_mult: (default.pass_high_cards_rank_mult as f32 * factor) as i32,
            pass_void_creation_base: (default.pass_void_creation_base as f32 * factor) as i32,
            pass_void_creation_mult: (default.pass_void_creation_mult as f32 * factor) as i32,
            pass_long_suit_penalty: (default.pass_long_suit_penalty as f32 * factor) as i32,
            pass_to_trailing_mult: (default.pass_to_trailing_mult as f32 * factor) as i32,
            pass_to_leader_mult: (default.pass_to_leader_mult as f32 * factor) as i32,
            pass_desperate_mult: (default.pass_desperate_mult as f32 * factor) as i32,
            pass_unseen_bonus: (default.pass_unseen_bonus as f32 * factor) as i32,
            pass_two_clubs_penalty: (default.pass_two_clubs_penalty as f32 * factor) as i32,
            pass_moon_keep_hearts: (default.pass_moon_keep_hearts as f32 * factor) as i32,
            pass_moon_keep_queen: (default.pass_moon_keep_queen as f32 * factor) as i32,
            pass_moon_keep_spades: (default.pass_moon_keep_spades as f32 * factor) as i32,
            pass_moon_void_bonus: (default.pass_moon_void_bonus as f32 * factor) as i32,
            pass_hunt_penalty_mult: (default.pass_hunt_penalty_mult as f32 * factor) as i32,
            pass_hunt_trailing_mult: (default.pass_hunt_trailing_mult as f32 * factor) as i32,
            pass_cards_played_mult: (default.pass_cards_played_mult as f32 * factor) as i32,
            pass_leader_rank_mult: (default.pass_leader_rank_mult as f32 * factor) as i32,

            play_take_trick_penalty: (default.play_take_trick_penalty as f32 * factor) as i32,
            play_take_points_mult: (default.play_take_points_mult as f32 * factor) as i32,
            play_avoid_trick_reward: (default.play_avoid_trick_reward as f32 * factor) as i32,
            play_dump_points_mult: (default.play_dump_points_mult as f32 * factor) as i32,
            play_clean_trick_rank_mult: (default.play_clean_trick_rank_mult as f32 * factor) as i32,
            play_void_creation_bonus: (default.play_void_creation_bonus as f32 * factor) as i32,
            play_follow_rank_mult: (default.play_follow_rank_mult as f32 * factor) as i32,
            play_slough_penalty_mult: (default.play_slough_penalty_mult as f32 * factor) as i32,
            play_lead_rank_mult: (default.play_lead_rank_mult as f32 * factor) as i32,
            play_break_hearts_penalty: (default.play_break_hearts_penalty as f32 * factor) as i32,
            play_hunt_lead_penalty_base: (default.play_hunt_lead_penalty_base as f32 * factor)
                as i32,
            play_hunt_lead_penalty_mult: (default.play_hunt_lead_penalty_mult as f32 * factor)
                as i32,
            play_moon_lead_hearts: (default.play_moon_lead_hearts as f32 * factor) as i32,
            play_desperate_take_mult: (default.play_desperate_take_mult as f32 * factor) as i32,
            play_desperate_dump_mult: (default.play_desperate_dump_mult as f32 * factor) as i32,
            play_cards_played_mult: (default.play_cards_played_mult as f32 * factor) as i32,
            play_unseen_bonus: (default.play_unseen_bonus as f32 * factor) as i32,
            play_moon_take_trick: (default.play_moon_take_trick as f32 * factor) as i32,
            play_moon_take_points_mult: (default.play_moon_take_points_mult as f32 * factor) as i32,
            play_moon_avoid_points_mult: (default.play_moon_avoid_points_mult as f32 * factor)
                as i32,
            play_hunt_avoid_trick: (default.play_hunt_avoid_trick as f32 * factor) as i32,
            play_hunt_feed_leader_mult: (default.play_hunt_feed_leader_mult as f32 * factor) as i32,
        }
    }
}
