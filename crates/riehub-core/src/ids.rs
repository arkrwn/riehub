//! Typed ids: `<prefix>_<ULID>` (storage spec, section 2).

use crate::error::{Error, Result};
use std::fmt;

macro_rules! typed_id {
    ($(#[$doc:meta])* $name:ident, $prefix:literal) => {
        $(#[$doc])*
        #[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord)]
        pub struct $name(String);

        impl $name {
            pub const PREFIX: &'static str = $prefix;

            pub fn new() -> Self {
                Self(format!("{}_{}", $prefix, ulid::Ulid::generate()))
            }

            pub fn parse(s: &str) -> Result<Self> {
                let ulid_part = s.strip_prefix(concat!($prefix, "_")).ok_or_else(|| {
                    Error::Invalid(format!("id must start with '{}_'", $prefix))
                })?;
                let parsed = ulid::Ulid::from_string(ulid_part)
                    .map_err(|_| Error::Invalid(format!("not a valid ULID: '{}'", s)))?;
                if parsed.to_string() != ulid_part {
                    return Err(Error::Invalid(format!("id is not canonical: '{}'", s)));
                }
                Ok(Self(s.to_owned()))
            }

            pub fn as_str(&self) -> &str {
                &self.0
            }
        }

        impl Default for $name {
            fn default() -> Self {
                Self::new()
            }
        }

        impl fmt::Display for $name {
            fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                f.write_str(&self.0)
            }
        }
    };
}

typed_id!(
    /// A user of the hub.
    UserId,
    "user"
);
typed_id!(
    /// A browser session.
    SessionId,
    "sess"
);
typed_id!(
    /// An API key a client presents to the hub.
    HubKeyId,
    "hkey"
);
typed_id!(
    /// A stored provider credential.
    ConnectionId,
    "conn"
);
typed_id!(
    /// One canonical AI request.
    RequestId,
    "req"
);

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used, clippy::expect_used)]

    use super::*;

    #[test]
    fn id_round_trips() {
        let id = UserId::new();
        let parsed = UserId::parse(id.as_str()).unwrap();
        assert_eq!(parsed, id);
        assert!(id.as_str().starts_with("user_"));
        assert_eq!(id.as_str().len(), "user_".len() + 26);
    }

    #[test]
    fn id_rejects_wrong_prefix() {
        let id = SessionId::new();
        assert!(UserId::parse(id.as_str()).is_err());
    }

    #[test]
    fn id_parse_rejects_bad_ulid() {
        assert!(UserId::parse("user_not-a-ulid").is_err());
        assert!(UserId::parse("user_").is_err());
        assert!(UserId::parse("").is_err());
        assert!(UserId::parse("user").is_err());
        // lowercase ULIDs are not canonical
        let canonical = UserId::new().as_str().to_owned();
        let lowercase = format!("user_{}", canonical["user_".len()..].to_lowercase());
        assert!(UserId::parse(&lowercase).is_err());
    }
}
