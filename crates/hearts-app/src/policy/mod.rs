mod embedded;

pub use embedded::EmbeddedPolicy;
pub use hearts_bot::policy::{HeuristicPolicy, Policy, PolicyContext};

/// Policy kind enumeration for factory construction
#[allow(dead_code)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PolicyKind {
    EasyLegacy,
    NormalHeuristic,
    FutureHard,
    EmbeddedML,
}

#[allow(dead_code)]
impl PolicyKind {
    pub fn create(&self) -> Box<dyn Policy> {
        match self {
            Self::EasyLegacy => Box::new(HeuristicPolicy::easy()),
            Self::NormalHeuristic => Box::new(HeuristicPolicy::normal()),
            Self::FutureHard => Box::new(HeuristicPolicy::hard()),
            Self::EmbeddedML => Box::new(EmbeddedPolicy::new()),
        }
    }
}
