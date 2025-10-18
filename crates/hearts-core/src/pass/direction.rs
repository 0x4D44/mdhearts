use crate::model::passing::PassingDirection;

/// Identifies the weighting profile for the current pass direction.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum DirectionWeightKind {
    LeftAttack,
    RightProtect,
    AcrossBalance,
    Hold,
}

/// Aggregated multipliers applied to different scoring components when
/// evaluating a pass candidate.
#[derive(Debug, Clone, Copy)]
pub struct DirectionProfile {
    /// Bonus applied to offloading liabilities (e.g., Q♠) when sending to this seat.
    pub liability_factor: f32,
    /// Bonus applied to void creation for this direction.
    pub void_factor: f32,
    /// Situational moon bonus multiplier.
    pub moon_factor: f32,
    /// Identifier for debugging/telemetry.
    pub kind: DirectionWeightKind,
}

impl DirectionProfile {
    pub fn from_direction(direction: PassingDirection) -> Self {
        match direction {
            PassingDirection::Left => Self {
                liability_factor: 1.2,
                void_factor: 1.1,
                moon_factor: 1.0,
                kind: DirectionWeightKind::LeftAttack,
            },
            PassingDirection::Right => Self {
                liability_factor: 0.9,
                void_factor: 1.4,
                moon_factor: 0.9,
                kind: DirectionWeightKind::RightProtect,
            },
            PassingDirection::Across => Self {
                liability_factor: 1.0,
                void_factor: 1.0,
                moon_factor: 1.1,
                kind: DirectionWeightKind::AcrossBalance,
            },
            PassingDirection::Hold => Self {
                liability_factor: 1.0,
                void_factor: 1.0,
                moon_factor: 1.0,
                kind: DirectionWeightKind::Hold,
            },
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::passing::PassingDirection;

    #[test]
    fn profiles_vary_by_direction() {
        let left = DirectionProfile::from_direction(PassingDirection::Left);
        let right = DirectionProfile::from_direction(PassingDirection::Right);
        assert!(left.liability_factor > right.liability_factor);
        assert!(right.void_factor > left.void_factor);
        assert_ne!(left.kind as u8, right.kind as u8);
    }
}
