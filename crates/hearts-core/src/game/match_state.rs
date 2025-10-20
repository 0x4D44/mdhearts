use crate::model::deck::Deck;
use crate::model::passing::PassingDirection;
use crate::model::player::PlayerPosition;
use crate::model::round::{RoundPhase, RoundState};
use crate::model::score::ScoreBoard;
use rand::SeedableRng;
use rand::rngs::StdRng;

#[derive(Debug, Clone)]
pub struct MatchState {
    scores: ScoreBoard,
    passing_index: usize,
    round_number: u32,
    current_round: RoundState,
    rng: StdRng,
    seed: u64,
}

impl MatchState {
    pub fn new(starting_player: PlayerPosition) -> Self {
        let seed: u64 = rand::random();
        Self::with_seed_round_direction(seed, 1, PassingDirection::CYCLE[0], starting_player)
    }

    pub fn with_seed(starting_player: PlayerPosition, seed: u64) -> Self {
        Self::with_seed_round_direction(seed, 1, PassingDirection::CYCLE[0], starting_player)
    }

    pub fn with_seed_round_direction(
        seed: u64,
        round_number: u32,
        direction: PassingDirection,
        starting_player: PlayerPosition,
    ) -> Self {
        let normalized_round = round_number.max(1);
        let mut rng = StdRng::seed_from_u64(seed);

        for _ in 1..normalized_round {
            let _ = Deck::shuffled(&mut rng);
        }

        let deck = Deck::shuffled(&mut rng);
        let passing_index = Self::passing_sequence()
            .iter()
            .position(|d| *d == direction)
            .unwrap_or(0);

        let current_round = RoundState::deal(&deck, starting_player, direction);

        Self {
            scores: ScoreBoard::new(),
            passing_index,
            round_number: normalized_round,
            current_round,
            rng,
            seed,
        }
    }

    pub fn from_snapshot(snapshot: &crate::game::serialization::MatchSnapshot) -> Self {
        let direction = snapshot
            .passing_direction
            .parse::<PassingDirection>()
            .unwrap_or(PassingDirection::Left);
        let mut state = MatchState::with_seed_round_direction(
            snapshot.seed,
            snapshot.round_number,
            direction,
            snapshot.round_starting_player,
        );
        state.scores_mut().set_totals(snapshot.scores);
        state
    }

    pub fn seed(&self) -> u64 {
        self.seed
    }

    pub fn scores(&self) -> &ScoreBoard {
        &self.scores
    }

    pub fn scores_mut(&mut self) -> &mut ScoreBoard {
        &mut self.scores
    }

    pub fn round(&self) -> &RoundState {
        &self.current_round
    }

    pub fn round_mut(&mut self) -> &mut RoundState {
        &mut self.current_round
    }

    pub fn round_number(&self) -> u32 {
        self.round_number
    }

    pub fn passing_direction(&self) -> PassingDirection {
        Self::passing_sequence()[self.passing_index % Self::passing_sequence().len()]
    }

    pub fn round_penalties(&self) -> [u8; 4] {
        self.current_round.penalty_totals()
    }

    pub fn finish_round_and_start_next(&mut self) {
        let penalties = self.current_round.penalty_totals();
        self.scores.apply_hand(penalties);

        self.round_number += 1;
        self.passing_index = (self.passing_index + 1) % Self::passing_sequence().len();

        let next_passing = self.passing_direction();
        let next_starting_player = self.current_round.starting_player().next();

        let deck = Deck::shuffled(&mut self.rng);
        self.current_round = RoundState::deal(&deck, next_starting_player, next_passing);
    }

    pub fn is_round_ready_for_scoring(&self) -> bool {
        matches!(self.current_round.phase(), RoundPhase::Playing)
            && self.current_round.tricks_completed() == 13
    }

    const fn passing_sequence() -> &'static [PassingDirection; 4] {
        &PassingDirection::CYCLE
    }
}

#[cfg(test)]
mod tests {
    use super::MatchState;
    use crate::model::card::Card;
    use crate::model::passing::PassingDirection;
    use crate::model::player::PlayerPosition;
    use crate::model::rank::Rank;
    use crate::model::suit::Suit;

    #[test]
    fn new_match_starts_with_left_pass() {
        let match_state = MatchState::with_seed(PlayerPosition::North, 0);
        assert_eq!(match_state.round_number(), 1);
        assert_eq!(match_state.passing_direction(), PassingDirection::Left);
    }

    #[test]
    fn finish_round_rotates_passing_direction() {
        let mut match_state = MatchState::with_seed(PlayerPosition::North, 0);

        match_state.finish_round_and_start_next();
        assert_eq!(match_state.round_number(), 2);
        assert_eq!(match_state.passing_direction(), PassingDirection::Right);

        match_state.finish_round_and_start_next();
        assert_eq!(match_state.round_number(), 3);
        assert_eq!(match_state.passing_direction(), PassingDirection::Across);
    }

    #[test]
    fn finishing_round_keeps_scores_when_no_penalties() {
        let mut match_state = MatchState::with_seed(PlayerPosition::North, 0);
        let before = *match_state.scores().standings();

        match_state.finish_round_and_start_next();

        assert_eq!(before, *match_state.scores().standings());
    }

    #[test]
    fn match_seed_is_exposed() {
        let match_state = MatchState::with_seed(PlayerPosition::North, 1234);
        assert_eq!(match_state.seed(), 1234);
    }

    #[test]
    fn next_round_leader_follows_two_of_clubs() {
        let mut match_state = MatchState::with_seed(PlayerPosition::North, 42);

        match_state.finish_round_and_start_next();

        let round = match_state.round();
        let two_of_clubs = Card::new(Rank::Two, Suit::Clubs);
        let expected = PlayerPosition::LOOP
            .iter()
            .copied()
            .find(|seat| round.hand(*seat).contains(two_of_clubs))
            .expect("two of clubs is dealt");

        assert_eq!(round.starting_player(), expected);
        assert_eq!(round.current_trick().leader(), expected);
    }
}
