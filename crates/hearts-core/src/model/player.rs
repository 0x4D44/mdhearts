use core::fmt;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[repr(u8)]
pub enum PlayerPosition {
    North = 0,
    East = 1,
    South = 2,
    West = 3,
}

impl PlayerPosition {
    pub const LOOP: [PlayerPosition; 4] = [
        PlayerPosition::North,
        PlayerPosition::East,
        PlayerPosition::South,
        PlayerPosition::West,
    ];

    pub const fn from_index(index: usize) -> Option<Self> {
        match index {
            0 => Some(PlayerPosition::North),
            1 => Some(PlayerPosition::East),
            2 => Some(PlayerPosition::South),
            3 => Some(PlayerPosition::West),
            _ => None,
        }
    }

    pub const fn index(self) -> usize {
        self as usize
    }

    pub const fn next(self) -> PlayerPosition {
        match self {
            PlayerPosition::North => PlayerPosition::East,
            PlayerPosition::East => PlayerPosition::South,
            PlayerPosition::South => PlayerPosition::West,
            PlayerPosition::West => PlayerPosition::North,
        }
    }

    pub const fn previous(self) -> PlayerPosition {
        match self {
            PlayerPosition::North => PlayerPosition::West,
            PlayerPosition::East => PlayerPosition::North,
            PlayerPosition::South => PlayerPosition::East,
            PlayerPosition::West => PlayerPosition::South,
        }
    }

    pub const fn opposite(self) -> PlayerPosition {
        match self {
            PlayerPosition::North => PlayerPosition::South,
            PlayerPosition::East => PlayerPosition::West,
            PlayerPosition::South => PlayerPosition::North,
            PlayerPosition::West => PlayerPosition::East,
        }
    }
}

impl fmt::Display for PlayerPosition {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let label = match self {
            PlayerPosition::North => "North",
            PlayerPosition::East => "East",
            PlayerPosition::South => "South",
            PlayerPosition::West => "West",
        };
        f.write_str(label)
    }
}

#[cfg(test)]
mod tests {
    use super::PlayerPosition;

    #[test]
    fn next_wraps_around() {
        assert_eq!(PlayerPosition::West.next(), PlayerPosition::North);
    }

    #[test]
    fn previous_wraps_around() {
        assert_eq!(PlayerPosition::North.previous(), PlayerPosition::West);
    }

    #[test]
    fn opposite_is_expected() {
        assert_eq!(PlayerPosition::North.opposite(), PlayerPosition::South);
    }

    #[test]
    fn index_roundtrip() {
        for (i, seat) in PlayerPosition::LOOP.iter().enumerate() {
            assert_eq!(PlayerPosition::from_index(i), Some(*seat));
            assert_eq!(seat.index(), i);
        }
    }
}
