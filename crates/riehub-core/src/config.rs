//! Configuration (api spec, section 3). One TOML file, defaults, then
//! `RIEHUB_*` environment variables with `__` between levels.

use crate::error::{Error, Result};
use serde::{Deserialize, Serialize};
use std::path::Path;

const ENV_PREFIX: &str = "RIEHUB_";

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(deny_unknown_fields, default)]
pub struct Config {
    pub profile: Profile,
    pub server: ServerConfig,
    pub data: DataConfig,
    pub database: DatabaseConfig,
    pub instance: InstanceConfig,
    pub identity: IdentityConfig,
    pub audit: AuditConfig,
    pub egress: EgressConfig,
    pub mcp: McpConfig,
    pub log: LogConfig,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum Profile {
    #[default]
    Dev,
    Production,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields, default)]
pub struct ServerConfig {
    pub bind: String,
    pub public_url: String,
    pub trusted_proxies: Vec<String>,
    pub client_ip_header: String,
    pub metrics_bind: String,
    pub max_body_bytes: u64,
    pub tls: TlsConfig,
}

impl Default for ServerConfig {
    fn default() -> Self {
        Self {
            bind: "127.0.0.1:8080".to_owned(),
            public_url: "http://127.0.0.1:8080".to_owned(),
            trusted_proxies: Vec::new(),
            client_ip_header: String::new(),
            metrics_bind: String::new(),
            max_body_bytes: 33_554_432,
            tls: TlsConfig::default(),
        }
    }
}

impl Profile {
    pub fn as_str(&self) -> &'static str {
        match self {
            Profile::Dev => "dev",
            Profile::Production => "production",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum TlsMode {
    #[default]
    Off,
    Files,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(deny_unknown_fields, default)]
pub struct TlsConfig {
    pub mode: TlsMode,
    pub cert: String,
    pub key: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields, default)]
pub struct DataConfig {
    pub dir: String,
}

impl Default for DataConfig {
    fn default() -> Self {
        Self {
            dir: "./data".to_owned(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields, default)]
pub struct DatabaseConfig {
    pub url: String,
}

impl Default for DatabaseConfig {
    fn default() -> Self {
        Self {
            url: "sqlite://./data/riehub.db".to_owned(),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum Seal {
    #[default]
    Systemd,
    Keychain,
    File,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields, default)]
pub struct InstanceConfig {
    pub seal: Seal,
    pub key_file: String,
}

impl Default for InstanceConfig {
    fn default() -> Self {
        Self {
            seal: Seal::Systemd,
            key_file: String::new(),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum Signup {
    #[default]
    Invite,
    Open,
    Closed,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(deny_unknown_fields, default)]
pub struct IdentityConfig {
    pub signup: Signup,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum AuditIp {
    #[default]
    Truncated,
    Full,
    Off,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields, default)]
pub struct AuditConfig {
    pub ip: AuditIp,
    pub retention_days: u64,
}

impl Default for AuditConfig {
    fn default() -> Self {
        Self {
            ip: AuditIp::Truncated,
            retention_days: 400,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(deny_unknown_fields, default)]
pub struct EgressConfig {
    pub allow: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(deny_unknown_fields, default)]
pub struct McpConfig {
    pub stdio: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum LogFormat {
    #[default]
    Json,
    Text,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields, default)]
pub struct LogConfig {
    pub level: String,
    pub format: LogFormat,
}

impl Default for LogConfig {
    fn default() -> Self {
        Self {
            level: "info".to_owned(),
            format: LogFormat::Json,
        }
    }
}

impl Config {
    pub const DEFAULT_PATH: &'static str = "./riehub.toml";

    /// Defaults, then the file (default `./riehub.toml`, then
    /// `/etc/riehub/riehub.toml`), then `RIEHUB_*` environment variables.
    pub fn load(path: Option<&Path>) -> Result<Self> {
        let file_table = match path {
            Some(p) => Some(read_table(p)?),
            None => {
                if Path::new(Self::DEFAULT_PATH).is_file() {
                    Some(read_table(Path::new(Self::DEFAULT_PATH))?)
                } else if Path::new("/etc/riehub/riehub.toml").is_file() {
                    Some(read_table(Path::new("/etc/riehub/riehub.toml"))?)
                } else {
                    None
                }
            }
        };

        let mut table = to_default_table()?;
        if let Some(file) = file_table {
            merge(&mut table, file);
        }
        apply_env(&mut table);

        let config = Config::deserialize(toml::Value::Table(table))
            .map_err(|e| Error::Invalid(format!("invalid config: {e}")))?;
        Ok(config)
    }

    /// Profile rules from the spec. Errors name the key, never a value.
    pub fn validate(&self) -> Result<()> {
        match self.profile {
            Profile::Dev => self.validate_dev(),
            Profile::Production => self.validate_production(),
        }
    }

    fn validate_dev(&self) -> Result<()> {
        if !is_loopback_host(&self.server.bind) {
            return Err(Error::Invalid(
                "server.bind: the dev profile binds to loopback only".to_owned(),
            ));
        }
        Ok(())
    }

    fn validate_production(&self) -> Result<()> {
        if self.server.tls.mode == TlsMode::Off && self.server.trusted_proxies.is_empty() {
            return Err(Error::Invalid(
                "server.tls.mode: production needs tls files or a trusted proxy in server.trusted_proxies"
                    .to_owned(),
            ));
        }
        if !self.server.public_url.starts_with("https://") {
            return Err(Error::Invalid(
                "server.public_url: production requires an https url".to_owned(),
            ));
        }
        if dir_world_readable(&self.data.dir) {
            return Err(Error::Invalid(
                "data.dir: the data directory is readable by other users".to_owned(),
            ));
        }
        Ok(())
    }
}

fn read_table(path: &Path) -> Result<toml::Table> {
    let text = std::fs::read_to_string(path)
        .map_err(|_| Error::NotFound(format!("config file not readable: {}", path.display())))?;
    toml::from_str(&text).map_err(|e| Error::Invalid(format!("invalid config file: {e}")))
}

fn to_default_table() -> Result<toml::Table> {
    let text =
        toml::to_string(&Config::default()).map_err(|e| Error::Internal(Some(Box::new(e))))?;
    toml::from_str(&text).map_err(|e| Error::Internal(Some(Box::new(e))))
}

fn merge(base: &mut toml::Table, overlay: toml::Table) {
    for (key, value) in overlay {
        match (base.get_mut(&key), value) {
            (Some(toml::Value::Table(b)), toml::Value::Table(o)) => merge(b, o),
            (_, value) => {
                base.insert(key, value);
            }
        }
    }
}

fn apply_env(table: &mut toml::Table) {
    let vars: Vec<(String, String)> = std::env::vars()
        .filter(|(key, _)| key.starts_with(ENV_PREFIX) && key.len() > ENV_PREFIX.len())
        .collect();
    for (key, raw) in vars {
        let path = key[ENV_PREFIX.len()..].to_lowercase();
        let parts: Vec<&str> = path.split("__").collect();
        if parts.iter().any(|part| part.is_empty()) {
            continue;
        }
        insert_path(table, &parts, value_from_env(&raw));
    }
}

fn insert_path(table: &mut toml::Table, path: &[&str], value: toml::Value) {
    if path.len() == 1 {
        table.insert(path[0].to_owned(), value);
        return;
    }
    let next = table
        .entry(path[0].to_owned())
        .or_insert_with(|| toml::Value::Table(toml::Table::new()));
    if let toml::Value::Table(inner) = next {
        insert_path(inner, &path[1..], value);
    }
    // A scalar blocks the deeper path, so the key is dropped. The
    // deserializer rejects or ignores it by the usual rules.
}

fn value_from_env(raw: &str) -> toml::Value {
    toml::from_str::<toml::Value>(&format!("v = {raw}"))
        .ok()
        .and_then(|t| t.get("v").cloned())
        .unwrap_or_else(|| toml::Value::String(raw.to_owned()))
}

fn is_loopback_host(bind: &str) -> bool {
    let host = bind.rsplit_once(':').map(|(h, _)| h).unwrap_or(bind);
    let host = host.trim_start_matches('[').trim_end_matches(']');
    matches!(host, "localhost" | "127.0.0.1" | "::1")
}

#[cfg(unix)]
fn dir_world_readable(path: &str) -> bool {
    use std::os::unix::fs::PermissionsExt;
    std::fs::metadata(path)
        .map(|m| m.is_dir() && m.permissions().mode() & 0o004 != 0)
        .unwrap_or(false)
}

#[cfg(not(unix))]
fn dir_world_readable(_path: &str) -> bool {
    false
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used, clippy::expect_used)]

    use super::*;

    #[test]
    fn defaults_are_dev_loopback_8080() {
        let config = Config::default();
        assert_eq!(config.profile, Profile::Dev);
        assert_eq!(config.server.bind, "127.0.0.1:8080");
        assert_eq!(config.server.public_url, "http://127.0.0.1:8080");
        assert_eq!(config.server.max_body_bytes, 33_554_432);
        assert_eq!(config.server.tls.mode, TlsMode::Off);
        assert_eq!(config.audit.retention_days, 400);
    }

    #[test]
    fn env_overrides_file() {
        let dir = std::env::temp_dir().join(format!("riehub-test-{}", ulid::Ulid::generate()));
        std::fs::create_dir_all(&dir).unwrap();
        let file = dir.join("riehub.toml");
        std::fs::write(
            &file,
            "profile = \"dev\"\n\n[server]\nbind = \"127.0.0.1:9999\"\n",
        )
        .unwrap();

        std::env::set_var("RIEHUB_SERVER__BIND", "127.0.0.1:7777");
        std::env::set_var("RIEHUB_LOG__LEVEL", "debug");

        let config = Config::load(Some(&file)).unwrap();
        assert_eq!(config.server.bind, "127.0.0.1:7777");
        assert_eq!(config.log.level, "debug");

        std::env::remove_var("RIEHUB_SERVER__BIND");
        std::env::remove_var("RIEHUB_LOG__LEVEL");
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn unknown_key_is_rejected() {
        let dir = std::env::temp_dir().join(format!("riehub-test-{}", ulid::Ulid::generate()));
        std::fs::create_dir_all(&dir).unwrap();
        let file = dir.join("riehub.toml");
        std::fs::write(&file, "[server]\nbind = \"127.0.0.1:8080\"\nsneaky = 1\n").unwrap();

        let result = Config::load(Some(&file));
        assert!(result.is_err(), "an unknown key must be rejected");

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn production_without_tls_or_trusted_proxy_is_rejected() {
        let config = Config {
            profile: Profile::Production,
            ..Default::default()
        };
        let err = config.validate().expect_err("must refuse");
        assert!(err.to_string().contains("server.tls.mode"), "was: {err}");
    }

    #[test]
    fn production_with_http_public_url_is_rejected() {
        let config = Config {
            profile: Profile::Production,
            server: ServerConfig {
                tls: TlsConfig {
                    mode: TlsMode::Files,
                    ..Default::default()
                },
                public_url: "http://hub.example.com".to_owned(),
                ..Default::default()
            },
            ..Default::default()
        };
        let err = config.validate().expect_err("must refuse");
        assert!(err.to_string().contains("server.public_url"), "was: {err}");
    }

    #[test]
    fn production_accepts_tls_or_proxy() {
        let with_tls = Config {
            profile: Profile::Production,
            server: ServerConfig {
                tls: TlsConfig {
                    mode: TlsMode::Files,
                    ..Default::default()
                },
                public_url: "https://hub.example.com".to_owned(),
                ..Default::default()
            },
            ..Default::default()
        };
        assert!(with_tls.validate().is_ok());

        let with_proxy = Config {
            profile: Profile::Production,
            server: ServerConfig {
                trusted_proxies: vec!["10.0.0.0/8".to_owned()],
                public_url: "https://hub.example.com".to_owned(),
                ..Default::default()
            },
            ..Default::default()
        };
        assert!(with_proxy.validate().is_ok());
    }

    #[test]
    fn dev_on_non_loopback_is_rejected() {
        let config = Config {
            server: ServerConfig {
                bind: "0.0.0.0:8080".to_owned(),
                ..Default::default()
            },
            ..Default::default()
        };
        let err = config.validate().expect_err("must refuse");
        assert!(err.to_string().contains("server.bind"), "was: {err}");

        assert!(is_loopback_host("127.0.0.1:8080"));
        assert!(is_loopback_host("localhost:8080"));
        assert!(is_loopback_host("[::1]:8080"));
        assert!(!is_loopback_host("10.0.0.5:8080"));
    }

    #[test]
    fn validation_error_does_not_echo_values() {
        let config = Config {
            server: ServerConfig {
                bind: "10.9.8.7:1234".to_owned(),
                ..Default::default()
            },
            ..Default::default()
        };
        let err = config.validate().expect_err("must refuse");
        let text = err.to_string();
        assert!(!text.contains("10.9.8.7"), "the value leaked: {text}");

        let config = Config {
            profile: Profile::Production,
            server: ServerConfig {
                public_url: "https://hunter2@example.com".to_owned(),
                ..Default::default()
            },
            ..Default::default()
        };
        let err = config.validate().expect_err("must refuse");
        let text = err.to_string();
        assert!(!text.contains("hunter2"), "the value leaked: {text}");
    }

    #[test]
    fn production_world_readable_data_dir_is_rejected() {
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let dir = std::env::temp_dir().join(format!("riehub-test-{}", ulid::Ulid::generate()));
            std::fs::create_dir_all(&dir).unwrap();
            std::fs::set_permissions(&dir, std::fs::Permissions::from_mode(0o755)).unwrap();

            let config = Config {
                profile: Profile::Production,
                server: ServerConfig {
                    tls: TlsConfig {
                        mode: TlsMode::Files,
                        ..Default::default()
                    },
                    public_url: "https://hub.example.com".to_owned(),
                    ..Default::default()
                },
                data: DataConfig {
                    dir: dir.display().to_string(),
                },
                ..Default::default()
            };
            let err = config.validate().expect_err("must refuse");
            assert!(err.to_string().contains("data.dir"), "was: {err}");

            let _ = std::fs::remove_dir_all(&dir);
        }
    }
}
