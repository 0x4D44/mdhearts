use core::fmt;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[repr(u8)]
pub enum Suit {
    Clubs = 0,
    Diamonds = 1,
    Spades = 2,
    Hearts = 3,
}

impl Suit {
    pub const ALL: [Suit; 4] = [Suit::Clubs, Suit::Diamonds, Suit::Spades, Suit::Hearts];

    pub const fn from_index(index: usize) -> Option<Self> {
        match index {
            0 => Some(Suit::Clubs),
            1 => Some(Suit::Diamonds),
            2 => Some(Suit::Spades),
            3 => Some(Suit::Hearts),
            _ => None,
        }
    }

    pub const fn is_heart(self) -> bool {
        matches!(self, Suit::Hearts)
    }

    pub const fn is_black(self) -> bool {
        matches!(self, Suit::Clubs | Suit::Spades)
    }
}

impl fmt::Display for Suit {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let symbol = match self {
            Suit::Clubs => "C",
            Suit::Diamonds => "D",
            Suit::Spades => "S",
            Suit::Hearts => "H",
        };
        f.write_str(symbol)
    }
}

#[cfg(test)]
mod tests {
    use super::Suit;

    #[test]
    fn display_returns_ascii_symbols() {
        assert_eq!(Suit::Clubs.to_string(), "C");
        assert_eq!(Suit::Hearts.to_string(), "H");
    }

    #[test]
    fn from_index_maps_valid_values() {
        assert_eq!(Suit::from_index(2), Some(Suit::Spades));
        assert_eq!(Suit::from_index(4), None);
    }
}
