//! AES-256-GCM-SIV AEAD (security spec, section 1). The blob format is
//! `0x01 || nonce(12) || ciphertext+tag`.

use crate::key::Key32;
use aws_lc_rs::aead::{Aad, LessSafeKey, Nonce, UnboundKey, AES_256_GCM_SIV};
use riehub_core::error::{Error, Result};
use zeroize::{Zeroize, Zeroizing};

const VERSION: u8 = 0x01;
const NONCE_LEN: usize = 12;
const TAG_LEN: usize = 16;

// The panics below are documented-unreachable: a 32-byte key is always
// valid for AES-256-GCM-SIV and the nonce is structurally valid, so the
// only failure is the OS generator, which is not recoverable.
#[allow(clippy::panic)]
pub fn aead_seal(key: &Key32, aad: &[u8], plaintext: &[u8]) -> Vec<u8> {
    let mut nonce_bytes = [0u8; NONCE_LEN];
    if aws_lc_rs::rand::fill(&mut nonce_bytes).is_err() {
        panic!("the operating system random generator failed");
    }
    let unbound = match UnboundKey::new(&AES_256_GCM_SIV, key.expose()) {
        Ok(k) => k,
        Err(_) => panic!("a 32-byte key is always valid for AES-256-GCM-SIV"),
    };
    let sealing = LessSafeKey::new(unbound);
    let nonce = Nonce::assume_unique_for_key(nonce_bytes);
    let mut in_out = plaintext.to_vec();
    if sealing
        .seal_in_place_append_tag(nonce, Aad::from(aad), &mut in_out)
        .is_err()
    {
        panic!("sealing with a valid key and nonce cannot fail");
    }
    let mut blob = Vec::with_capacity(1 + NONCE_LEN + in_out.len());
    blob.push(VERSION);
    blob.extend_from_slice(&nonce_bytes);
    blob.extend_from_slice(&in_out);
    blob
}

pub fn aead_open(key: &Key32, aad: &[u8], blob: &[u8]) -> Result<Zeroizing<Vec<u8>>> {
    if blob.first() != Some(&VERSION) {
        return Err(Error::Invalid(
            "aead blob has an unknown version byte".to_owned(),
        ));
    }
    if blob.len() < 1 + NONCE_LEN + TAG_LEN {
        return Err(Error::Invalid("aead blob is too short".to_owned()));
    }
    let unbound = match UnboundKey::new(&AES_256_GCM_SIV, key.expose()) {
        Ok(k) => k,
        Err(_) => return Err(Error::Internal(None)),
    };
    let opening = LessSafeKey::new(unbound);
    let mut nonce_bytes = [0u8; NONCE_LEN];
    nonce_bytes.copy_from_slice(&blob[1..1 + NONCE_LEN]);
    let nonce = Nonce::assume_unique_for_key(nonce_bytes);
    let mut in_out = blob[1 + NONCE_LEN..].to_vec();
    let opened = match opening.open_in_place(nonce, Aad::from(aad), &mut in_out) {
        Ok(plain) => plain,
        Err(_) => return Err(Error::Invalid("aead authentication failed".to_owned())),
    };
    let out = Zeroizing::new(opened.to_vec());
    in_out.zeroize();
    Ok(out)
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used, clippy::expect_used)]

    use super::*;

    const KEY_A: Key32 = Key32::new([1u8; 32]);
    const KEY_B: Key32 = Key32::new([2u8; 32]);

    fn blob_without_version(blob: &[u8]) -> Vec<u8> {
        blob[1..].to_vec()
    }

    #[test]
    fn aead_round_trips() {
        let plaintext = b"the vault key material";
        let blob = aead_seal(&KEY_A, b"riehub/v1/test/aad", plaintext);
        assert_eq!(blob[0], 0x01);
        assert_eq!(blob.len(), 1 + NONCE_LEN + plaintext.len() + TAG_LEN);
        let opened = aead_open(&KEY_A, b"riehub/v1/test/aad", &blob).unwrap();
        assert_eq!(opened.as_slice(), plaintext.as_slice());
    }

    #[test]
    fn aead_rejects_wrong_key() {
        let blob = aead_seal(&KEY_A, b"", b"secret");
        assert!(aead_open(&KEY_B, b"", &blob).is_err());
    }

    #[test]
    fn aead_rejects_wrong_aad() {
        let blob = aead_seal(&KEY_A, b"riehub/v1/test/one", b"secret");
        assert!(aead_open(&KEY_A, b"riehub/v1/test/two", &blob).is_err());
    }

    #[test]
    fn aead_rejects_flipped_bit() {
        let blob = aead_seal(&KEY_A, b"", b"secret");
        let mut tampered = blob.clone();
        let last = tampered.len() - 1;
        tampered[last] ^= 0x01;
        assert!(aead_open(&KEY_A, b"", &tampered).is_err());
    }

    #[test]
    fn aead_rejects_truncated() {
        let blob = aead_seal(&KEY_A, b"", b"secret");
        let truncated = &blob[..blob.len() - 1];
        assert!(aead_open(&KEY_A, b"", truncated).is_err());
        let bare = blob_without_version(&blob);
        assert!(aead_open(&KEY_A, b"", &bare[..bare.len() - 1]).is_err());
    }

    #[test]
    fn aead_rejects_unknown_version_byte() {
        let blob = aead_seal(&KEY_A, b"", b"secret");
        let mut tampered = blob.clone();
        tampered[0] = 0x02;
        assert!(aead_open(&KEY_A, b"", &tampered).is_err());
    }

    #[test]
    fn two_seals_of_same_input_differ() {
        let first = aead_seal(&KEY_A, b"", b"secret");
        let second = aead_seal(&KEY_A, b"", b"secret");
        assert_ne!(first, second, "the nonce is random, blobs must differ");
        assert_ne!(first[1..13], second[1..13], "the nonces must differ");
    }
}
