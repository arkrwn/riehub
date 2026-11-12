//! riehub-core: ids, errors, config types, `Secret<T>`, clock.

#![forbid(unsafe_code)]

pub const VERSION: &str = env!("CARGO_PKG_VERSION");

#[cfg(test)]
mod tests {
    #[test]
    fn version_matches_workspace_version() {
        assert_eq!(super::VERSION, "0.0.1");
    }
}
