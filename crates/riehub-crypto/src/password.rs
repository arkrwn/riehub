//! Argon2id (security spec, section 1): 64 MiB, 3 passes, 1 lane,
//! 16-byte salt, 32-byte output.

use crate::key::Key32;
use argon2::{Algorithm, Argon2, Params, Version};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Argon2Params {
    pub m_kib: u32,
    pub t: u32,
    pub p: u32,
}

impl Argon2Params {
    /// 64 MiB, 3 passes, 1 lane (security spec, section 1).
    pub const DEFAULT: Self = Self {
        m_kib: 65_536,
        t: 3,
        p: 1,
    };

    /// 8 MiB, 1 pass, 1 lane. For tests only.
    #[cfg(any(test, feature = "test-util"))]
    pub const TEST: Self = Self {
        m_kib: 8_192,
        t: 1,
        p: 1,
    };
}

// The panics below are documented-unreachable: the spec parameters are
// always valid argon2 parameters and the output is 32 bytes.
#[allow(clippy::panic)]
pub fn argon2id(password: &[u8], salt: &[u8; 16], params: Argon2Params) -> Key32 {
    let built = match Params::new(params.m_kib, params.t, params.p, Some(32)) {
        Ok(p) => p,
        Err(_) => panic!("the spec argon2 parameters are always valid"),
    };
    let argon = Argon2::new(Algorithm::Argon2id, Version::V0x13, built);
    let mut out = [0u8; 32];
    if argon.hash_password_into(password, salt, &mut out).is_err() {
        panic!("hash_password_into with a 32-byte output cannot fail");
    }
    Key32::new(out)
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used, clippy::expect_used)]

    use super::*;

    /// Known-answer test against the argon2 crate's own reference
    /// computation with the same parameters.
    #[test]
    fn argon2id_matches_the_reference_computation() {
        let password = b"correct horse battery staple";
        let salt = [0x11u8; 16];
        let expected = {
            let params = Params::new(
                Argon2Params::TEST.m_kib,
                Argon2Params::TEST.t,
                Argon2Params::TEST.p,
                Some(32),
            )
            .unwrap();
            let argon = Argon2::new(Algorithm::Argon2id, Version::V0x13, params);
            let mut out = [0u8; 32];
            argon.hash_password_into(password, &salt, &mut out).unwrap();
            out
        };
        let key = argon2id(password, &salt, Argon2Params::TEST);
        assert_eq!(key.expose(), &expected);
    }

    #[test]
    fn argon2id_is_deterministic_and_salt_separates() {
        let a = argon2id(b"pw", &[1u8; 16], Argon2Params::TEST);
        let b = argon2id(b"pw", &[1u8; 16], Argon2Params::TEST);
        let c = argon2id(b"pw", &[2u8; 16], Argon2Params::TEST);
        assert_eq!(a.expose(), b.expose());
        assert_ne!(a.expose(), c.expose());
    }
}
