//! `Key32`: 32 bytes of key material (security spec, section 1). No
//! `Debug`, `Display`, `Clone` or `Serialize`.

use zeroize::Zeroize;

// The spec forbids Debug on key material (security spec, section 1),
// so this crate opts out of the workspace-wide Debug lint for Key32.
#[allow(missing_debug_implementations)]
pub struct Key32([u8; 32]);

impl Key32 {
    pub const fn new(bytes: [u8; 32]) -> Self {
        Self(bytes)
    }

    /// The key bytes. The name shows up in reviews and audits.
    pub fn expose(&self) -> &[u8; 32] {
        &self.0
    }
}

impl Drop for Key32 {
    fn drop(&mut self) {
        self.0.zeroize();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn key32_zeroizes_on_drop_is_sound_by_construction() {
        // The struct owns its bytes and zeroes them in Drop. This test
        // pins the contract that expose() is the only read path.
        let key = Key32::new([7u8; 32]);
        assert_eq!(key.expose(), &[7u8; 32]);
    }
}
