//! Probabilistic belief tracking for opponent card ownership.
//!
//! This module is composed of:
//! - `hard`: deterministic updates and data structures (`Belief`, `BeliefUpdateCtx`, etc.).
//! - `soft`: behavior-driven likelihood adjustments layered on top of hard constraints.
//! - `sampler`: world sampling utilities consuming the belief state.
//! - `cache`: lightweight cache keyed by coarse belief snapshots.

mod cache;
mod hard;
mod sampler;
pub mod soft;
pub mod telemetry;

pub use cache::{BeliefCacheKey, SamplerCache};
pub use hard::{Belief, BeliefUpdateCtx, SuitCounts, SuitMask};
pub use sampler::{BeliefSampler, SampledWorld, SamplingError, SuitQuotas};
