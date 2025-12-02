//! Scoring arithmetic utilities with overflow protection.
//!
//! This module provides saturating arithmetic operations for the scoring system
//! to prevent integer overflow in edge cases. While overflow is unlikely in normal
//! play (scores typically range from -100,000 to +100,000), these helpers provide
//! defense-in-depth.
//!
//! These utilities are available for use in scoring functions. The individual
//! functions may be adopted incrementally as scoring code is modified.

#![allow(dead_code)] // Functions available for incremental adoption

/// Score type alias for potential future changes (e.g., i64 or custom type)
pub type Score = i32;

/// Saturating addition for scores
#[inline]
pub const fn score_add(base: Score, delta: Score) -> Score {
    base.saturating_add(delta)
}

/// Saturating subtraction for scores
#[inline]
pub const fn score_sub(base: Score, delta: Score) -> Score {
    base.saturating_sub(delta)
}

/// Saturating multiplication for scores
#[inline]
pub const fn score_mul(a: Score, b: Score) -> Score {
    a.saturating_mul(b)
}

/// Combined operation: base + (a * b) with saturation
#[inline]
pub const fn score_add_product(base: Score, a: Score, b: Score) -> Score {
    base.saturating_add(a.saturating_mul(b))
}

/// Combined operation: base - (a * b) with saturation
#[inline]
pub const fn score_sub_product(base: Score, a: Score, b: Score) -> Score {
    base.saturating_sub(a.saturating_mul(b))
}

/// Accumulator for building up scores with automatic saturation
#[derive(Debug, Clone, Copy, Default)]
pub struct ScoreAccumulator {
    value: Score,
}

impl ScoreAccumulator {
    /// Create a new accumulator with initial value
    #[inline]
    pub const fn new(initial: Score) -> Self {
        Self { value: initial }
    }

    /// Add a value with saturation
    #[inline]
    pub fn add(&mut self, delta: Score) -> &mut Self {
        self.value = self.value.saturating_add(delta);
        self
    }

    /// Subtract a value with saturation
    #[inline]
    pub fn sub(&mut self, delta: Score) -> &mut Self {
        self.value = self.value.saturating_sub(delta);
        self
    }

    /// Add a product with saturation
    #[inline]
    pub fn add_product(&mut self, a: Score, b: Score) -> &mut Self {
        self.value = score_add_product(self.value, a, b);
        self
    }

    /// Subtract a product with saturation
    #[inline]
    pub fn sub_product(&mut self, a: Score, b: Score) -> &mut Self {
        self.value = score_sub_product(self.value, a, b);
        self
    }

    /// Get the accumulated value
    #[inline]
    pub const fn get(&self) -> Score {
        self.value
    }
}

impl From<ScoreAccumulator> for Score {
    fn from(acc: ScoreAccumulator) -> Self {
        acc.value
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn saturating_add_normal() {
        assert_eq!(score_add(100, 200), 300);
        assert_eq!(score_add(-100, 200), 100);
        assert_eq!(score_add(100, -200), -100);
    }

    #[test]
    fn saturating_add_overflow() {
        assert_eq!(score_add(i32::MAX, 1), i32::MAX);
        assert_eq!(score_add(i32::MAX, i32::MAX), i32::MAX);
        assert_eq!(score_add(i32::MIN, -1), i32::MIN);
    }

    #[test]
    fn saturating_mul_normal() {
        assert_eq!(score_mul(100, 200), 20_000);
        assert_eq!(score_mul(-100, 200), -20_000);
    }

    #[test]
    fn saturating_mul_overflow() {
        assert_eq!(score_mul(i32::MAX, 2), i32::MAX);
        assert_eq!(score_mul(i32::MIN, 2), i32::MIN);
    }

    #[test]
    fn add_product_normal() {
        assert_eq!(score_add_product(100, 10, 20), 300);
        assert_eq!(score_add_product(100, -10, 20), -100);
    }

    #[test]
    fn add_product_overflow() {
        assert_eq!(score_add_product(i32::MAX, 1000, 1000), i32::MAX);
        assert_eq!(score_add_product(0, i32::MAX, 2), i32::MAX);
    }

    #[test]
    fn accumulator_basic() {
        let mut acc = ScoreAccumulator::new(0);
        acc.add(100).add(200).sub(50);
        assert_eq!(acc.get(), 250);
    }

    #[test]
    fn accumulator_products() {
        let mut acc = ScoreAccumulator::new(1000);
        acc.add_product(10, 20); // +200
        acc.sub_product(5, 10); // -50
        assert_eq!(acc.get(), 1150);
    }

    #[test]
    fn accumulator_overflow_protection() {
        let mut acc = ScoreAccumulator::new(i32::MAX - 100);
        acc.add(1000);
        assert_eq!(acc.get(), i32::MAX);
    }

    /// Test realistic scoring scenario with extreme values
    #[test]
    fn realistic_extreme_scoring() {
        // Simulate worst-case scoring scenario:
        // - Maximum penalties (26 points)
        // - Large multipliers (up to 1500)
        // - Multiple bonuses/penalties stacking
        let mut acc = ScoreAccumulator::new(0);

        // QS priority
        acc.add(18_000);

        // Multiple penalty calculations
        for _ in 0..13 {
            acc.add_product(26, 1_500); // 39,000 per iteration
        }

        // This would overflow without saturation
        // 18_000 + 13 * 39_000 = 525,000 (still within i32)
        assert!(acc.get() > 0);
        assert!(acc.get() < i32::MAX);
    }
}
