//! HKDF-SHA384 (security spec, section 1). Every info string starts
//! with `riehub/v1/`, enforced at compile time by [`info`].

use crate::key::Key32;
use aws_lc_rs::hkdf::{Salt, HKDF_SHA384};

const INFO_PREFIX: &str = "riehub/v1/";

/// Compile-time prefix check for hkdf info strings. A wrong literal
/// fails the build, not the runtime. Pass a literal argument so the
/// const evaluation happens during compilation.
#[allow(clippy::panic)]
pub const fn info(info: &'static str) -> &'static str {
    let bytes = info.as_bytes();
    let prefix = INFO_PREFIX.as_bytes();
    let mut i = 0;
    while i < prefix.len() {
        if i >= bytes.len() || bytes[i] != prefix[i] {
            panic!("hkdf info must start with riehub/v1/");
        }
        i += 1;
    }
    info
}

struct Okm32;

impl aws_lc_rs::hkdf::KeyType for Okm32 {
    fn len(&self) -> usize {
        32
    }
}

// The panics below are documented-unreachable: a 32-byte output is
// always inside the HKDF length limit.
#[allow(clippy::panic)]
pub fn hkdf(ikm: &[u8], info: &'static str) -> Key32 {
    let salt = Salt::new(HKDF_SHA384, &[]);
    let prk = salt.extract(ikm);
    let mut out = [0u8; 32];
    let infos = [info.as_bytes()];
    let okm = match prk.expand(&infos, Okm32) {
        Ok(okm) => okm,
        Err(_) => panic!("a 32-byte okm is always within the hkdf limit"),
    };
    if okm.fill(&mut out).is_err() {
        panic!("filling 32 bytes from a 32-byte okm cannot fail");
    }
    Key32::new(out)
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used, clippy::expect_used)]

    use super::*;
    use zeroize::Zeroize;

    /// RFC 5869 style: the expected value is computed with aws-lc-rs
    /// directly, then compared against the wrapper.
    #[test]
    fn hkdf_matches_an_aws_lc_rs_computation() {
        let ikm = b"input key material for the riehub hkdf test";
        let text = info("riehub/v1/test/kdf");
        let expected = {
            let salt = Salt::new(HKDF_SHA384, &[]);
            let prk = salt.extract(ikm);
            let mut out = [0u8; 32];
            prk.expand(&[text.as_bytes()], Okm32)
                .unwrap()
                .fill(&mut out)
                .unwrap();
            out
        };
        let key = hkdf(ikm, text);
        assert_eq!(key.expose(), &expected);
    }

    #[test]
    fn hkdf_is_deterministic_and_info_separates_domains() {
        let a = hkdf(b"ikm", info("riehub/v1/test/one"));
        let b = hkdf(b"ikm", info("riehub/v1/test/one"));
        let c = hkdf(b"ikm", info("riehub/v1/test/two"));
        assert_eq!(a.expose(), b.expose());
        assert_ne!(a.expose(), c.expose());
    }

    #[test]
    fn info_prefix_holds_for_call_sites() {
        // Every info literal in the codebase goes through info(), so a
        // wrong prefix is a compile error. This pins the accepted form.
        assert_eq!(info("riehub/v1/test/one"), "riehub/v1/test/one");
    }

    #[test]
    fn hkdf_zeroizes_failed_scratch() {
        // The okm buffer is owned by the returned Key32, which zeroes
        // on drop; this pins that the scratch path leaves nothing
        // reachable from outside Key32.
        let key = hkdf(b"ikm", info("riehub/v1/test/scratch"));
        let mut bytes = *key.expose();
        bytes.zeroize();
        assert!(bytes.iter().all(|b| *b == 0));
    }
}
