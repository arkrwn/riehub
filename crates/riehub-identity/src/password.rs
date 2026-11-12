//! Password policy (identity spec, section 2): 12 to 1024 bytes, no
//! composition rules, the 10 000 most common passwords rejected from
//! the embedded list.

use riehub_core::error::{Error, Result};
use std::collections::HashSet;
use std::sync::OnceLock;

const MIN_BYTES: usize = 12;
const MAX_BYTES: usize = 1024;
const COMMON_LIST: &str = include_str!("../data/common_passwords.txt");

static COMMON: OnceLock<HashSet<&'static str>> = OnceLock::new();

fn common() -> &'static HashSet<&'static str> {
    COMMON.get_or_init(|| COMMON_LIST.lines().collect())
}

#[derive(Debug)]
pub struct PasswordBytes<'a> {
    bytes: &'a [u8],
}

impl<'a> From<&'a [u8]> for PasswordBytes<'a> {
    fn from(bytes: &'a [u8]) -> Self {
        Self { bytes }
    }
}

/// Validates the length and the common-password rule. The value never
/// appears in an error.
pub fn validate(bytes: &PasswordBytes<'_>) -> Result<()> {
    if bytes.bytes.len() < MIN_BYTES {
        return Err(Error::Invalid(
            "password: use at least 12 characters".to_owned(),
        ));
    }
    if bytes.bytes.len() > MAX_BYTES {
        return Err(Error::Invalid(
            "password: at most 1024 characters".to_owned(),
        ));
    }
    let text = std::str::from_utf8(bytes.bytes)
        .map_err(|_| Error::Invalid("password: use characters".to_owned()))?;
    if common().contains(text) {
        return Err(Error::Invalid(
            "password: this one is too common".to_owned(),
        ));
    }
    Ok(())
}

/// Whether the password is on the embedded common list.
pub fn is_common(text: &str) -> bool {
    common().contains(text)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_embedded_list_holds_10000_unique_entries() {
        assert_eq!(common().len(), 10_000);
        assert!(is_common("password123"));
        assert!(!is_common("x7!kQ2vLm@pR"));
    }

    #[test]
    fn length_bounds_are_enforced() {
        assert!(validate(&b"short".as_slice().into()).is_err());
        let ok = b"a-really-long-passphrase-here";
        assert!(validate(&ok.as_slice().into()).is_ok());
        let too_long = vec![b'x'; MAX_BYTES + 1];
        assert!(validate(&too_long.as_slice().into()).is_err());
    }
}
