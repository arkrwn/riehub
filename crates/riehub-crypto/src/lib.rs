//! riehub-crypto: AEAD, KDFs, hashing, random, constant-time compare.

#![forbid(unsafe_code)]

pub mod aead;
pub mod ct;
pub mod hash;
pub mod hkdf;
pub mod key;
pub mod password;
pub mod random;

pub use aead::{aead_open, aead_seal};
pub use ct::ct_eq;
pub use hash::{hmac, sha384};
pub use hkdf::{hkdf, info};
pub use key::Key32;
pub use password::{argon2id, Argon2Params};
pub use random::{base64url, base64url_decode, random_bytes};
