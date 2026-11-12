//! riehub-store: the `Store` trait and its backends.

#![forbid(unsafe_code)]

#[cfg(test)]
mod tests {
    #[test]
    fn workspace_version_is_wired() {
        assert_eq!(env!("CARGO_PKG_VERSION"), "0.0.1");
    }
}
