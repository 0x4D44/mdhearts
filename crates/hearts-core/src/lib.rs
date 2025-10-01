#![deny(warnings)]
pub mod game;
pub mod model;

pub struct AppInfo;

impl AppInfo {
    pub const fn name() -> &'static str {
        "mdhearts"
    }

    pub const fn codename() -> &'static str {
        "Rust Remaster"
    }

    pub const fn version() -> &'static str {
        env!("CARGO_PKG_VERSION")
    }
}

#[cfg(test)]
mod tests {
    use super::AppInfo;

    #[test]
    fn exposes_static_metadata() {
        assert_eq!(AppInfo::name(), "mdhearts");
        assert_eq!(AppInfo::codename(), "Rust Remaster");
        assert!(!AppInfo::version().is_empty());
    }
}
