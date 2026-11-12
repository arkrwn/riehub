//! riehub-core: ids, errors, config types, `Secret<T>`, clock.

#![forbid(unsafe_code)]

pub mod clock;
pub mod config;
pub mod content;
pub mod error;
pub mod ids;
pub mod owner;
pub mod secret;

pub use clock::{Clock, SystemClock, TestClock};
pub use config::{
    AuditConfig, AuditIp, Config, DataConfig, DatabaseConfig, EgressConfig, IdentityConfig,
    InstanceConfig, LogConfig, LogFormat, McpConfig, Profile, Seal, ServerConfig, Signup,
    TlsConfig, TlsMode,
};
pub use content::Content;
pub use error::{Error, Result};
pub use ids::{ConnectionId, HubKeyId, RequestId, SessionId, UserId};
pub use owner::Owner;
pub use secret::Secret;

pub const VERSION: &str = env!("CARGO_PKG_VERSION");

#[cfg(test)]
mod tests {
    #[test]
    fn version_matches_workspace_version() {
        assert_eq!(super::VERSION, "0.0.1");
    }
}
