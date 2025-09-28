use crate::model::player::PlayerPosition;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ScoreBoard {
    totals: [u32; 4],
}

impl ScoreBoard {
    pub const fn new() -> Self {
        Self { totals: [0; 4] }
    }

    pub fn add_penalty(&mut self, seat: PlayerPosition, points: u32) {
        self.totals[seat.index()] += points;
    }

    pub fn set_score(&mut self, seat: PlayerPosition, points: u32) {
        self.totals[seat.index()] = points;
    }

    pub fn set_totals(&mut self, totals: [u32; 4]) {
        self.totals = totals;
    }

    pub fn score(&self, seat: PlayerPosition) -> u32 {
        self.totals[seat.index()]
    }

    pub fn standings(&self) -> &[u32; 4] {
        &self.totals
    }

    pub fn leading_player(&self) -> PlayerPosition {
        PlayerPosition::LOOP
            .iter()
            .copied()
            .min_by_key(|seat| self.score(*seat))
            .unwrap_or(PlayerPosition::North)
    }

    pub fn apply_hand(&mut self, penalties: [u8; 4]) {
        let total: u32 = penalties.iter().map(|&p| p as u32).sum();
        if total == 26 {
            if let Some(shooter) = PlayerPosition::LOOP
                .iter()
                .copied()
                .find(|seat| penalties[seat.index()] == 26)
            {
                for seat in PlayerPosition::LOOP.iter().copied() {
                    if seat != shooter {
                        self.add_penalty(seat, 26);
                    }
                }
                return;
            }
        }

        for seat in PlayerPosition::LOOP.iter().copied() {
            self.add_penalty(seat, penalties[seat.index()] as u32);
        }
    }
}

impl Default for ScoreBoard {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::ScoreBoard;
    use crate::model::player::PlayerPosition;

    #[test]
    fn scoreboard_tracks_penalty_points() {
        let mut board = ScoreBoard::new();
        board.add_penalty(PlayerPosition::East, 13);
        assert_eq!(board.score(PlayerPosition::East), 13);
        assert_eq!(board.score(PlayerPosition::North), 0);
    }

    #[test]
    fn leading_player_is_lowest_score() {
        let mut board = ScoreBoard::new();
        board.add_penalty(PlayerPosition::North, 26);
        board.add_penalty(PlayerPosition::West, 1);
        assert_eq!(board.leading_player(), PlayerPosition::East);
    }

    #[test]
    fn apply_hand_adds_penalties_normally() {
        let mut board = ScoreBoard::new();
        board.apply_hand([1, 5, 0, 20]);
        assert_eq!(board.score(PlayerPosition::North), 1);
        assert_eq!(board.score(PlayerPosition::East), 5);
        assert_eq!(board.score(PlayerPosition::West), 20);
    }

    #[test]
    fn shoot_the_moon_awards_opponents() {
        let mut board = ScoreBoard::new();
        board.apply_hand([26, 0, 0, 0]);
        assert_eq!(board.score(PlayerPosition::North), 0);
        assert_eq!(board.score(PlayerPosition::East), 26);
        assert_eq!(board.score(PlayerPosition::South), 26);
        assert_eq!(board.score(PlayerPosition::West), 26);
    }

    #[test]
    fn set_totals_overwrites_scores() {
        let mut board = ScoreBoard::new();
        board.set_totals([10, 20, 30, 40]);
        assert_eq!(board.score(PlayerPosition::North), 10);
        assert_eq!(board.score(PlayerPosition::West), 40);
    }
}
