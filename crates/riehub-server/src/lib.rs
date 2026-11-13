//! riehub-server: the `riehub` binary, axum routes, middleware, wiring, embedded web assets.

#![forbid(unsafe_code)]

pub mod cli;
pub mod harden;
pub mod http;
pub mod openapi;
pub mod routes;
pub mod serve;
pub mod web;

/// The line `riehub version` prints.
pub fn version_line() -> String {
    format!("riehub {}", env!("CARGO_PKG_VERSION"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn greeting_names_the_project() {
        assert!(version_line().starts_with("riehub"));
    }
}
