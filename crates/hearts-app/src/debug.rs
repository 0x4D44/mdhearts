//! Shared debug utilities for MDHearts
//!
//! This module consolidates debug logging functionality that was previously
//! duplicated across multiple modules.

use std::sync::OnceLock;

/// Returns true if debug logging is enabled via MDH_DEBUG_LOGS environment variable.
///
/// Accepts "1", "true", or "on" (case-insensitive) as truthy values.
/// The result is cached after first check for performance.
pub fn debug_enabled() -> bool {
    static ON: OnceLock<bool> = OnceLock::new();
    *ON.get_or_init(|| {
        std::env::var("MDH_DEBUG_LOGS")
            .map(|v| v == "1" || v.eq_ignore_ascii_case("true") || v.eq_ignore_ascii_case("on"))
            .unwrap_or(false)
    })
}

#[cfg(test)]
mod tests {
    // Note: We don't test debug_enabled() directly because it reads env vars
    // and uses OnceLock caching which makes it difficult to test in isolation.
    // The function is simple enough that visual inspection suffices.
}
