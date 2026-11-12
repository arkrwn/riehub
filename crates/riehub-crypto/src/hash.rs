//! HMAC-SHA384 and SHA-384 (security spec, section 1).

use crate::key::Key32;
use aws_lc_rs::{digest, hmac};

pub fn hmac(key: &Key32, data: &[u8]) -> [u8; 48] {
    let k = hmac::Key::new(hmac::HMAC_SHA384, key.expose());
    let tag = hmac::sign(&k, data);
    let mut out = [0u8; 48];
    out.copy_from_slice(tag.as_ref());
    out
}

pub fn sha384(data: &[u8]) -> [u8; 48] {
    let d = digest::digest(&digest::SHA384, data);
    let mut out = [0u8; 48];
    out.copy_from_slice(d.as_ref());
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    /// RFC 4231 test case 1 for HMAC-SHA-384: key is 0x0b repeated 20
    /// times, data is "Hi There".
    #[test]
    fn hmac_matches_rfc4231_case_1() {
        let key = Key32::new({
            let mut k = [0u8; 32];
            k[..20].fill(0x0b);
            k
        });
        let tag = hmac(&key, b"Hi There");
        let expected: [u8; 48] = [
            0xaf, 0xd0, 0x39, 0x44, 0xd8, 0x48, 0x95, 0x62, 0x6b, 0x08, 0x25, 0xf4, 0xab, 0x46,
            0x90, 0x7f, 0x15, 0xf9, 0xda, 0xdb, 0xe4, 0x10, 0x1e, 0xc6, 0x82, 0xaa, 0x03, 0x4c,
            0x7c, 0xeb, 0xc5, 0x9c, 0xfa, 0xea, 0x9e, 0xa9, 0x07, 0x6e, 0xde, 0x7f, 0x4a, 0xf1,
            0x52, 0xe8, 0xb2, 0xfa, 0x9c, 0xb6,
        ];
        assert_eq!(tag, expected);
    }

    #[test]
    fn hmac_is_deterministic() {
        let key = Key32::new([9u8; 32]);
        assert_eq!(hmac(&key, b"one"), hmac(&key, b"one"));
        assert_ne!(hmac(&key, b"one"), hmac(&key, b"two"));
    }

    /// SHA-384 of "abc" (NIST CAVP known answer).
    #[test]
    fn sha384_matches_the_abc_vector() {
        let digest = sha384(b"abc");
        let expected: [u8; 48] = [
            0xcb, 0x00, 0x75, 0x3f, 0x45, 0xa3, 0x5e, 0x8b, 0xb5, 0xa0, 0x3d, 0x69, 0x9a, 0xc6,
            0x50, 0x07, 0x27, 0x2c, 0x32, 0xab, 0x0e, 0xde, 0xd1, 0x63, 0x1a, 0x8b, 0x60, 0x5a,
            0x43, 0xff, 0x5b, 0xed, 0x80, 0x86, 0x07, 0x2b, 0xa1, 0xe7, 0xcc, 0x23, 0x58, 0xba,
            0xec, 0xa1, 0x34, 0xc8, 0x25, 0xa7,
        ];
        assert_eq!(digest, expected);
    }
}
